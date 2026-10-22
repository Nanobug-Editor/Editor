//! Native legacy operations keep verified ancestors and the exact source fixed until completion.

use crate::attachments::{pin_directories, resolved_handle};
use editor_history::{LegacyFingerprint, LegacyIo};
use std::{
    fs::File,
    io,
    path::{Component, Path},
    time::UNIX_EPOCH,
};

/// Windows-first safe access to retired history; unsupported native deletion fails conservatively.
pub struct NativeLegacyIo;

impl LegacyIo for NativeLegacyIo {
    fn open(&self, root: &Path, relative: &Path, expected: LegacyFingerprint) -> io::Result<File> {
        let (_pins, path) = pin_source(root, relative)?;
        let file = open_source(&path, false)?;
        validate(&file, &path, expected)?;
        Ok(file)
    }

    fn remove(&self, root: &Path, relative: &Path, expected: LegacyFingerprint) -> io::Result<()> {
        let (pins, path) = match pin_source(root, relative) {
            Ok(value) => value,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        let file = match open_source(&path, true) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        validate(&file, &path, expected)?;
        remove_opened(&file, &pins, &path)
    }
}

/// Freeze every ancestor before opening the final component; never accept an alternate data stream.
fn pin_source(root: &Path, relative: &Path) -> io::Result<(Vec<File>, std::path::PathBuf)> {
    if relative.components().count() != 2
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        || relative.to_string_lossy().contains(':')
        || root.canonicalize()? != root
    {
        return Err(denied());
    }
    let path = root.join(relative);
    let parent = path.parent().ok_or_else(denied)?;
    let pins = pin_directories(parent)?;
    if root.canonicalize()? != root || parent.canonicalize()? != parent {
        return Err(denied());
    }
    Ok((pins, path))
}

/// File sharing rejects replacement and in-place writes throughout the checked read/delete.
#[cfg(windows)]
fn open_source(path: &Path, deleting: bool) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .access_mode(0x80000000 | if deleting { 0x00010000 } else { 0 })
        .share_mode(1)
        .custom_flags(0x00200000)
        .open(path)
}

#[cfg(not(windows))]
fn open_source(path: &Path, _: bool) -> io::Result<File> {
    File::open(path)
}

/// Validate the opened object rather than an earlier pathname's metadata.
fn validate(file: &File, path: &Path, expected: LegacyFingerprint) -> io::Result<()> {
    let metadata = file.metadata()?;
    let modified = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map_err(|_| denied())?;
    if !metadata.is_file()
        || metadata.len() != expected.size
        || i64::try_from(modified.as_nanos()).ok() != Some(expected.modified_ns)
        || resolved_handle(file)? != path
    {
        return Err(denied());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(denied());
        }
    }
    Ok(())
}

/// Mark the opened Windows file for deletion; no second path lookup can select another file.
#[cfg(windows)]
fn remove_opened(file: &File, _: &[File], _: &Path) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{
            FILE_DISPOSITION_INFO, FileDispositionInfo, SetFileInformationByHandle,
        },
    };
    let info = FILE_DISPOSITION_INFO {
        DeleteFile: true.into(),
    };
    // The owned handle was opened with DELETE access and the exact structure remains live for this call.
    unsafe {
        SetFileInformationByHandle(
            HANDLE(file.as_raw_handle()),
            FileDispositionInfo,
            (&info as *const FILE_DISPOSITION_INFO).cast(),
            std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    }
    .map_err(io::Error::other)
}

/// A pinned parent alone cannot protect the final directory entry from replacement on Unix.
#[cfg(not(windows))]
fn remove_opened(_: &File, _: &[File], _: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Native history deletion is unavailable",
    ))
}

fn denied() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "Legacy history source changed or left its private root",
    )
}
