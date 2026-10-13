//! Directory leases deduplicate aliases using the filesystem's identity and preserve case-sensitive roots.
use std::{fs::File, io, path::Path, sync::Arc};

/// The open handle prevents deletion/recreation from reusing this identity while its project is alive.
#[derive(Clone, Debug)]
pub struct DirectoryIdentity {
    volume: u64,
    file: [u8; 16],
    _lease: Arc<File>,
}
impl PartialEq for DirectoryIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.volume == other.volume && self.file == other.file
    }
}
impl Eq for DirectoryIdentity {}

impl DirectoryIdentity {
    /// Resolve one existing directory, following junctions and symbolic links exactly as an ordinary open.
    /// Returns the platform error for inaccessible/disappeared roots and rejects regular files.
    pub fn open(path: &Path) -> io::Result<Self> {
        #[cfg(windows)]
        {
            use std::{
                mem::size_of,
                os::windows::{fs::OpenOptionsExt as _, io::AsRawHandle as _},
            };
            use windows::Win32::{
                Foundation::HANDLE,
                Storage::FileSystem::{
                    FILE_FLAG_BACKUP_SEMANTICS, FILE_ID_INFO, FileIdInfo,
                    GetFileInformationByHandleEx,
                },
            };
            let file = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0)
                .open(path)?;
            if !file.metadata()?.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Project path must be a directory",
                ));
            }
            let mut info = FILE_ID_INFO::default();
            // The live File owns this valid HANDLE and the initialized structure is exactly the declared buffer size.
            unsafe {
                GetFileInformationByHandleEx(
                    HANDLE(file.as_raw_handle()),
                    FileIdInfo,
                    (&mut info as *mut FILE_ID_INFO).cast(),
                    size_of::<FILE_ID_INFO>() as u32,
                )
            }
            .map_err(io::Error::other)?;
            Ok(Self {
                volume: info.VolumeSerialNumber,
                file: info.FileId.Identifier,
                _lease: Arc::new(file),
            })
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            let file = File::open(path)?;
            let metadata = file.metadata()?;
            if !metadata.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Project path must be a directory",
                ));
            }
            let mut id = [0; 16];
            id[..8].copy_from_slice(&metadata.ino().to_le_bytes());
            Ok(Self {
                volume: metadata.dev(),
                file: id,
                _lease: Arc::new(file),
            })
        }
    }
}
