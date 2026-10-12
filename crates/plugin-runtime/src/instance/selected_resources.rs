//! Native picker grants bind one instance to an exact file/save target or an explicitly selected directory.
//! Legacy picks stay readonly; files.selection 1.1 preserves the user's explicit write consent.
use super::*;
use crate::native_path::{invalid_path, io_failure, is_reparse, pin_path};
use api::{EditorValue, ErrorCode, Failure, ResourceHandle, Value};
use plugin_protocol::interaction::{SelectedResource, SelectionAccess, SelectionMode};
use std::{fs::OpenOptions, io::Read, sync::atomic::Ordering};

/// Immutable authority is never reconstructed from a guest's display name or resource number.
pub(super) struct Selection {
    pub(super) handle: ResourceHandle,
    pub(super) path: PathBuf,
    pub(super) mode: SelectionMode,
    /// A new explicit picker may allow writes; Save remains write-only even when this is true.
    pub(super) write: bool,
    /// Only immutable identities survive selection; even shared native handles can obstruct replace.
    pub(super) observed: Vec<(u64, u64, u64)>,
}

impl State {
    /// Native UI and direct host commands may use their own grants; peers and hooks cannot.
    pub(super) fn check_selection_authority(&self) -> Result<(), Failure> {
        if !self.active
            || self.roots.retired
            || self.migrating
            || self.language_hook
            || self.roots.application
            || self
                .plugin_services
                .context
                .as_ref()
                .is_some_and(|context| {
                    !context.direct_host_selection(&self.plugin_services.principal.instance)
                })
            || !self.plugin_services.alive.load(Ordering::Acquire)
        {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Selection requires the active owning instance, without delegation",
            ));
        }
        if !self.api.capabilities.contains_key("files.selection") {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "files.selection was not negotiated",
            ));
        }
        if !self.permissions.contains("files.select") {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "files.select permission required",
            ));
        }
        Ok(())
    }

    /// Validate the whole native result before allocating anything; failures leave no partial grants.
    pub(super) fn grant_selection(
        &mut self,
        paths: Vec<PathBuf>,
        mode: SelectionMode,
        multiple: bool,
        access: SelectionAccess,
    ) -> Result<EditorValue, Failure> {
        self.check_selection_authority()?;
        if paths.is_empty()
            || paths.len() > 32
            || (!multiple && paths.len() != 1)
            || (mode == SelectionMode::Save && paths.len() != 1)
        {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Invalid native selection count",
            ));
        }
        let checked = paths
            .into_iter()
            .map(|path| {
                let pins = pin_path(&path, mode == SelectionMode::Save)?;
                let path = if path.exists() {
                    path.canonicalize().map_err(io_failure)?
                } else {
                    path.parent()
                        .ok_or_else(invalid_path)?
                        .canonicalize()
                        .map_err(io_failure)?
                        .join(path.file_name().ok_or_else(invalid_path)?)
                };
                if (mode == SelectionMode::Directory && !path.is_dir())
                    || (mode == SelectionMode::File && !path.is_file())
                    || (mode == SelectionMode::Save && path.exists() && !path.is_file())
                {
                    return Err(Failure::new(
                        ErrorCode::InvalidPath,
                        "Selection type does not match native target",
                    ));
                }
                Ok((path, pins))
            })
            .collect::<Result<Vec<_>, Failure>>()?;
        let mut issued = Vec::new();
        for (path, pins) in checked {
            let handle = match self.roots.open(resource_roots::RootKind::Selected) {
                Ok(Value::Resource(handle)) => handle,
                Err(error) => {
                    for resource in &issued {
                        let resource: &SelectedResource = resource;
                        self.roots.remove(&resource.handle);
                        self.selected_resources.remove(&resource.handle.resource);
                    }
                    return Err(error);
                }
                _ => unreachable!(),
            };
            let name = path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned();
            self.selected_resources.insert(
                handle.resource,
                Selection {
                    handle: handle.clone(),
                    path,
                    mode,
                    write: access == SelectionAccess::ReadWrite,
                    observed: pins.identities,
                },
            );
            issued.push(SelectedResource {
                handle,
                name,
                kind: mode,
            });
        }
        Ok(EditorValue::Interaction(
            plugin_protocol::interaction::Value::Selected(issued),
        ))
    }

    /// File handles use an empty relative name; directory handles permit only validated descendants.
    /// Save intent deliberately has no read/write path until host document transactions consume it.
    pub(super) fn read_selection(
        &self,
        handle: &ResourceHandle,
        relative: &str,
    ) -> Result<Vec<u8>, Failure> {
        self.check_selection_authority()?;
        self.roots.resolve(handle)?;
        let selection = self
            .selected_resources
            .get(&handle.resource)
            .ok_or_else(|| {
                Failure::new(ErrorCode::InvalidHandle, "Selected resource was released")
            })?;
        let path = match selection.mode {
            SelectionMode::File if relative.is_empty() => selection.path.clone(),
            SelectionMode::Directory => {
                if relative.is_empty()
                    || relative.len() > 4096
                    || relative.contains(['\\', ':', '\0'])
                    || relative
                        .split('/')
                        .any(|part| part.is_empty() || matches!(part, "." | ".."))
                {
                    return Err(invalid_path());
                }
                selection.path.join(relative)
            }
            SelectionMode::Save => {
                return Err(Failure::new(
                    ErrorCode::PermissionDenied,
                    "Save selection grants no read authority",
                ));
            }
            _ => return Err(invalid_path()),
        };
        let pins = pin_path(&path, false)?;
        if !pins.identities.starts_with(&selection.observed) {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Selected target or ancestor was replaced",
            ));
        }
        let resolved = path.canonicalize().map_err(io_failure)?;
        if (selection.mode == SelectionMode::File && resolved != selection.path)
            || (selection.mode == SelectionMode::Directory
                && !resolved.starts_with(&selection.path))
        {
            return Err(invalid_path());
        }
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // OPEN_REPARSE_POINT prevents a final-component junction/symlink race.
            options.share_mode(3).custom_flags(0x00200000);
        }
        let file = options.open(&resolved).map_err(io_failure)?;
        let metadata = file.metadata().map_err(io_failure)?;
        if !metadata.is_file() || is_reparse(&metadata) {
            return Err(invalid_path());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            // Unix has no Windows sharing pin: compare the actually opened inode before reading bytes.
            if pins.identities.last() != Some(&(metadata.dev(), metadata.ino(), 0)) {
                return Err(Failure::new(
                    ErrorCode::InvalidHandle,
                    "Selected file changed before open",
                ));
            }
        }
        let mut bytes = Vec::new();
        file.take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(io_failure)?;
        if bytes.len() > 1024 * 1024 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Selected file exceeds read quota",
            ));
        }
        Ok(bytes)
    }

    /// Quiesce revokes selected authority before a version switch or trust revocation.
    pub(super) fn clear_selections(&mut self) {
        for selection in self.selected_resources.values() {
            self.roots.remove(&selection.handle);
        }
        self.selected_resources.clear();
    }
}
