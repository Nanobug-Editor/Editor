//! Native authority validates every ancestor and briefly pins it without retaining obstructing file locks.

use plugin_protocol::api::{ErrorCode, Failure};
use std::{
    fs::{File, OpenOptions},
    path::{Component, Path, PathBuf},
};

/// Immutable object identities are compared only while owned native handles keep ancestors stable.
pub(crate) struct CheckedPath {
    pub(crate) identities: Vec<(u64, u64, u64)>,
    handles: Vec<File>,
    leaf_present: bool,
}

impl CheckedPath {
    /// Replacing a leaf must remain possible; ancestor sharing pins still reject link/rename redirection.
    pub(crate) fn release_leaf(&mut self) {
        if self.leaf_present {
            self.handles.pop();
        }
    }
}

/// Match existing Windows case/short-name aliases to the native document's canonical path.
/// Missing destination components retain their requested names. Ancestors stay pinned while
/// canonicalizing, so normalization cannot follow a newly substituted junction or symlink.
pub(crate) fn normalized_target(path: &Path) -> Result<PathBuf, Failure> {
    let _pins = pin_path(path, true)?;
    let mut ancestor = path.to_path_buf();
    let mut missing = Vec::new();
    loop {
        match std::fs::symlink_metadata(&ancestor) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(ancestor.file_name().ok_or_else(invalid_path)?.to_owned());
                if !ancestor.pop() {
                    return Err(invalid_path());
                }
            }
            Err(error) => return Err(io_failure(error)),
        }
    }
    let mut normalized = ancestor.canonicalize().map_err(io_failure)?;
    for name in missing.into_iter().rev() {
        normalized.push(name);
    }
    Ok(normalized)
}

/// Reject aliases, streams and redirected ancestors. A planned creation may have a missing tail.
pub(crate) fn pin_path(path: &Path, missing_tail: bool) -> Result<CheckedPath, Failure> {
    if !path.is_absolute() {
        return Err(invalid_path());
    }
    let mut current = PathBuf::new();
    let mut pins = CheckedPath {
        identities: Vec::new(),
        handles: Vec::new(),
        leaf_present: false,
    };
    let mut missing = false;
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                if !matches!(
                    prefix.kind(),
                    std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)
                ) {
                    return Err(invalid_path());
                }
                current.push(component);
                continue;
            }
            Component::RootDir => current.push(component),
            Component::Normal(name) => {
                let name = name.to_str().ok_or_else(invalid_path)?;
                if name.contains([':', '\0', '<', '>', '"', '|', '?', '*'])
                    || name.chars().any(char::is_control)
                    || name.ends_with(['.', ' '])
                    || plugin_protocol::api::is_windows_device_segment(name)
                {
                    return Err(invalid_path());
                }
                current.push(component);
            }
            _ => return Err(invalid_path()),
        }
        if missing {
            continue;
        }
        let metadata = match std::fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if missing_tail && error.kind() == std::io::ErrorKind::NotFound => {
                missing = true;
                continue;
            }
            Err(error) => return Err(io_failure(error)),
        };
        if is_reparse(&metadata) {
            return Err(invalid_path());
        }
        pins.leaf_present = current == path;
        #[cfg(windows)]
        {
            use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
            use windows_sys::Win32::Storage::FileSystem::{
                BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
            };
            // OPEN_REPARSE_POINT avoids following a changed leaf; deny delete while validating each ancestor.
            let pin = OpenOptions::new()
                .access_mode(0)
                .share_mode(3)
                .custom_flags(0x02000000 | 0x00200000)
                .open(&current)
                .map_err(io_failure)?;
            if is_reparse(&pin.metadata().map_err(io_failure)?) {
                return Err(invalid_path());
            }
            let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
            // SAFETY: the owned pin is live and the API fills the correctly sized output on success.
            if unsafe { GetFileInformationByHandle(pin.as_raw_handle(), info.as_mut_ptr()) } == 0 {
                return Err(io_failure(std::io::Error::last_os_error()));
            }
            // SAFETY: the successful native call initialized every field.
            let info = unsafe { info.assume_init() };
            pins.identities.push((
                u64::from(info.dwVolumeSerialNumber),
                (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
                (u64::from(info.ftCreationTime.dwHighDateTime) << 32)
                    | u64::from(info.ftCreationTime.dwLowDateTime),
            ));
            pins.handles.push(pin);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            pins.identities.push((metadata.dev(), metadata.ino(), 0));
        }
    }
    Ok(pins)
}

/// Reparse attributes cover symlinks and junctions on Windows, including a linked final component.
pub(crate) fn is_reparse(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

pub(crate) fn invalid_path() -> Failure {
    Failure::new(ErrorCode::InvalidPath, "Unsafe native file path")
}
pub(crate) fn io_failure(error: std::io::Error) -> Failure {
    Failure::new(
        if error.kind() == std::io::ErrorKind::NotFound {
            ErrorCode::NotFound
        } else {
            ErrorCode::OperationFailed
        },
        error.to_string(),
    )
}
