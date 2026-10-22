//! Native profile locations and filesystem containment; storage belongs to editor-history.

#[cfg(windows)]
use std::path::Component;
use std::{
    io,
    path::{Path, PathBuf},
};
mod legacy;
pub use legacy::NativeLegacyIo;

/// A quick final native identity check supplements complete background content hashing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileStamp {
    /// Detect size-changing edits without rereading content on the UI thread.
    length: u64,
    /// Ordinary writes change this even when their lengths match.
    modified: std::time::SystemTime,
    /// Replacements retain a distinct creation identity where the filesystem provides one.
    created: Option<std::time::SystemTime>,
    /// A read-only transition revokes an already prepared write.
    readonly: bool,
    /// Native file identity rejects a same-path replacement with copied timestamps.
    #[cfg(windows)]
    file: [u8; 16],
    /// The same numeric file identity on another volume is unrelated.
    #[cfg(windows)]
    volume: u64,
}

impl FileStamp {
    /// Inspect a regular, unredirected native file. None represents an observed missing path.
    pub fn read(path: &Path) -> io::Result<Option<Self>> {
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        #[cfg(windows)]
        let link = {
            use std::os::windows::fs::MetadataExt as _;
            metadata.file_attributes() & 0x400 != 0
        };
        #[cfg(not(windows))]
        let link = metadata.file_type().is_symlink();
        if link || !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "History target is redirected or not a file",
            ));
        }
        #[cfg(windows)]
        let identity = file_identity(path)?;
        Ok(Some(Self {
            length: metadata.len(),
            modified: metadata.modified()?,
            created: metadata.created().ok(),
            readonly: metadata.permissions().readonly(),
            #[cfg(windows)]
            file: identity.1,
            #[cfg(windows)]
            volume: identity.0,
        }))
    }
}

/// Read the stable file identity through the already locked Windows bindings, without unstable std APIs.
#[cfg(windows)]
fn file_identity(path: &Path) -> io::Result<(u64, [u8; 16])> {
    use std::os::windows::io::AsRawHandle as _;
    use windows::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{FILE_ID_INFO, FileIdInfo, GetFileInformationByHandleEx},
    };
    let file = std::fs::File::open(path)?;
    let mut info = FILE_ID_INFO::default();
    // File owns the live handle; the zero-initialized output buffer has exactly this native type's size.
    unsafe {
        GetFileInformationByHandleEx(
            HANDLE(file.as_raw_handle()),
            FileIdInfo,
            (&mut info as *mut FILE_ID_INFO).cast(),
            std::mem::size_of::<FILE_ID_INFO>() as u32,
        )
    }
    .map_err(io::Error::other)?;
    Ok((info.VolumeSerialNumber, info.FileId.Identifier))
}

/// Resolved policy destination and established history location for one selected application profile.
#[derive(Clone, Debug)]
pub struct HistoryPaths {
    /// Normal users retain LocalAppData/MeEditor/history; isolated profiles use their own history.
    pub root: PathBuf,
    /// User-only configuration; project settings never override this file.
    pub settings: PathBuf,
}

impl HistoryPaths {
    /// Resolve future data destinations before initialization; a project cannot become its own history store.
    pub fn validate_workspace(&self, workspace: &Path) -> io::Result<()> {
        let workspace = workspace.canonicalize()?;
        for path in [&self.root, &self.settings] {
            let mut current = path.clone();
            let mut missing = Vec::new();
            while !current.try_exists()? {
                missing.push(
                    current
                        .file_name()
                        .ok_or_else(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidInput,
                                "Invalid history destination",
                            )
                        })?
                        .to_owned(),
                );
                current = current
                    .parent()
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "Missing history ancestor")
                    })?
                    .to_owned();
            }
            let mut resolved = current.canonicalize()?;
            for name in missing.into_iter().rev() {
                resolved.push(name);
            }
            if resolved.starts_with(&workspace) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "History and its user settings must be stored outside the workspace",
                ));
            }
        }
        Ok(())
    }

    /// Select paths without creating directories. A selected profile takes precedence over environment.
    pub fn for_profile(profile: Option<&Path>) -> io::Result<Self> {
        let normal = dirs::config_dir().map(|root| root.join("MeEditor"));
        let selected = profile
            .map(Path::to_path_buf)
            .or_else(|| {
                std::env::var_os("ME_EDITOR_PROFILE_HOME")
                    .filter(|value| !value.is_empty())
                    .map(PathBuf::from)
            })
            .or_else(|| normal.clone())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "Application profile directory unavailable",
                )
            })?;
        let root = if normal.as_ref() == Some(&selected) {
            dirs::data_local_dir()
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        "Local application data directory unavailable",
                    )
                })?
                .join("MeEditor")
                .join("history")
        } else {
            selected.join("history")
        };
        Ok(Self {
            root,
            settings: selected.join("history.json"),
        })
    }
}

/// Create missing destination parents only beneath an existing, unchanged workspace root.
/// Existing ancestors stay pinned during creation, so a later containment failure cannot leave
/// newly created directories outside the root. PreparedNewFile separately pins final publication.
#[cfg(windows)]
pub fn prepare_recovery_parent(root: &Path, destination: &Path) -> io::Result<()> {
    let denied = || {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Recovery path left its workspace",
        )
    };
    if root.canonicalize()? != root || !root.is_dir() {
        return Err(denied());
    }
    let parent = destination.parent().ok_or_else(denied)?;
    let relative = parent.strip_prefix(root).map_err(|_| denied())?;
    let mut current = root.to_path_buf();
    let mut pins = crate::attachments::pin_directories(root)?;
    for part in relative.components() {
        let Component::Normal(name) = part else {
            return Err(denied());
        };
        if name
            .to_string_lossy()
            .contains([':', '<', '>', '"', '|', '?', '*'])
            || name.to_string_lossy().ends_with(['.', ' '])
        {
            return Err(denied());
        }
        current.push(name);
        let creation = current.clone();
        match std::fs::create_dir(&creation) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        if !current.is_dir() || current.canonicalize()? != current {
            return Err(denied());
        }
        let mut opened = crate::attachments::pin_directories(&current)?;
        // Retaining all ancestors prevents their replacement before the next mkdir.
        pins.append(&mut opened);
    }
    if root.canonicalize()? != root {
        return Err(denied());
    }
    Ok(())
}

/// Other native targets retain the recovered buffer until an equivalent safe adapter is available.
#[cfg(not(windows))]
pub fn prepare_recovery_parent(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Safe native recovery directory creation is unavailable",
    ))
}
