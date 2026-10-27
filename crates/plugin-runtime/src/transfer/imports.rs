//! User-dropped files borrow one offer's readonly lifetime; paths never enter guest JSON or file roots.
use super::*;
use crate::native_path::{invalid_path, io_failure, is_reparse, pin_path};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    time::SystemTime,
};

/// A native readonly handle pins the admitted regular file until the offer closes or expires.
pub(super) struct NativeImport {
    file: Mutex<File>,
    name: String,
    length: u64,
    modified: Option<SystemTime>,
}
impl NativeImport {
    /// Keep ancestor pins until the final non-reparse handle has been opened with no write sharing.
    fn open(path: &std::path::Path) -> Result<Self, Failure> {
        let _pins = pin_path(path, false)?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(1).custom_flags(0x00200000);
        }
        let file = options.open(path).map_err(io_failure)?;
        let metadata = file.metadata().map_err(io_failure)?;
        if !metadata.is_file() || is_reparse(&metadata) {
            return Err(invalid_path());
        }
        if metadata.len() > transfer::MAX_FILE_BYTES {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Dropped file size quota exceeded",
            ));
        }
        Ok(Self {
            file: Mutex::new(file),
            name: path
                .file_name()
                .and_then(|value| value.to_str())
                .filter(|name| name.len() <= 1024)
                .ok_or_else(invalid_path)?
                .into(),
            length: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }
    /// Detect changes on platforms where a writer can retain an already-open handle.
    fn unchanged(&self, file: &File) -> Result<(), Failure> {
        let metadata = file.metadata().map_err(io_failure)?;
        if metadata.len() != self.length || metadata.modified().ok() != self.modified {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Imported file changed",
            ));
        }
        Ok(())
    }
}

impl TransferOfferResource {
    /// Materialize the selected representation, translating native paths into readonly import slots.
    /// Whole-batch validation precedes publication; failed batches grant no partial file authority.
    pub fn read_selected(&self, preferred: &[String]) -> Result<transfer::Value, Failure> {
        if self.select(preferred)? != transfer::FILES_MIME {
            return match self.read(preferred)? {
                HostTransferData::Bytes(item) => Ok(transfer::Value::Data(item)),
                HostTransferData::Files(_) => unreachable!("selected representation was checked"),
            };
        }
        // External rendering and file IO never hold the registry lock needed by close/admission.
        if self.imports.lock().unwrap().is_none() {
            let HostTransferData::Files(paths) = self.read(preferred)? else {
                return Err(Failure::new(
                    ErrorCode::InvalidRequest,
                    "Native file list required",
                ));
            };
            if paths.is_empty() || paths.len() > transfer::MAX_FILES {
                return Err(Failure::new(
                    ErrorCode::LimitExceeded,
                    "Dropped file count quota exceeded",
                ));
            }
            let files = paths
                .iter()
                .map(|path| NativeImport::open(path))
                .collect::<Result<Vec<_>, _>>()?;
            if files.iter().map(|file| file.length).sum::<u64>() > transfer::MAX_FILE_BYTES {
                return Err(Failure::new(
                    ErrorCode::LimitExceeded,
                    "Dropped batch size quota exceeded",
                ));
            }
            if !self.is_live() {
                return Err(invalid());
            }
            let mut imports = self.imports.lock().unwrap();
            if !self.is_live() {
                return Err(invalid());
            }
            // A concurrent materialization cannot replace already issued entry identities.
            imports.get_or_insert_with(|| Arc::new(files));
        }
        let imports = self.imports.lock().unwrap().clone().ok_or_else(invalid)?;
        Ok(transfer::Value::Files(
            imports
                .iter()
                .enumerate()
                .map(|(entry, file)| transfer::ImportedFile {
                    handle: transfer::ImportHandle {
                        offer: self.handle.clone(),
                        entry: entry as u32,
                    },
                    name: file.name.clone(),
                    byte_len: file.length,
                })
                .collect(),
        ))
    }

    /// Validate a guest's entry and chunk budget without touching disk or extending its parent lease.
    pub(crate) fn check_file(
        &self,
        handle: &transfer::ImportHandle,
        offset: u64,
        max_bytes: u32,
    ) -> Result<(), Failure> {
        if !self.is_live() || handle.offer != self.handle {
            return Err(invalid());
        }
        if max_bytes == 0 || max_bytes as usize > transfer::MAX_BYTES {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Import chunk quota exceeded",
            ));
        }
        let imports = self.imports.lock().unwrap();
        let file = imports
            .as_ref()
            .and_then(|files| files.get(handle.entry as usize))
            .ok_or_else(invalid)?;
        if offset > file.length {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Import offset exceeds file size",
            ));
        }
        Ok(())
    }

    /// Read only a bounded chunk of the admitted object; no caller-supplied path or write mode exists.
    pub fn read_file(
        &self,
        handle: &transfer::ImportHandle,
        offset: u64,
        max_bytes: u32,
    ) -> Result<transfer::Value, Failure> {
        self.check_file(handle, offset, max_bytes)?;
        let imports = self.imports.lock().unwrap().clone().ok_or_else(invalid)?;
        let file = imports.get(handle.entry as usize).ok_or_else(invalid)?;
        let mut native_file = file.file.lock().unwrap();
        file.unchanged(&native_file)?;
        native_file
            .seek(SeekFrom::Start(offset))
            .map_err(io_failure)?;
        let mut bytes = Vec::new();
        native_file
            .by_ref()
            .take(u64::from(max_bytes))
            .read_to_end(&mut bytes)
            .map_err(io_failure)?;
        file.unchanged(&native_file)?;
        if !self.is_live() {
            return Err(invalid());
        }
        let eof = offset + bytes.len() as u64 >= file.length;
        Ok(transfer::Value::FileBytes { bytes, eof })
    }
}
