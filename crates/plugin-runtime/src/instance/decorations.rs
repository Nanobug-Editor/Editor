//! Admission of owned marker collections, before any native text or UI entity is touched.
use super::*;
use api::{EditorOperation, ErrorCode, Failure, Value};
use plugin_protocol::decorations::Operation;
use resource_roots::RootKind;
use std::{collections::BTreeMap, sync::Arc};

/// Immutable artwork is carried only by the admitted native request; failed replacement cannot change old icons.
pub(super) type Prepared = (Arc<crate::DecorationResource>, BTreeMap<String, Arc<[u8]>>);
impl State {
    pub(super) fn prepare_decorations(
        &mut self,
        operation: &EditorOperation,
    ) -> Result<Option<Prepared>, Failure> {
        let EditorOperation::Decorations { operation } = operation else {
            return Ok(None);
        };
        operation.validate()?;
        let markers = match operation {
            Operation::Create { markers, .. } | Operation::Replace { markers, .. } => {
                markers.as_slice()
            }
            Operation::Read { .. } => &[],
        };
        let mut icons = BTreeMap::new();
        for marker in markers {
            if let Some(icon) = &marker.icon {
                if !self.permissions.contains("assets.read") {
                    return Err(Failure::new(
                        ErrorCode::PermissionDenied,
                        "Marker icons require assets.read",
                    ));
                }
                if !self.api.capabilities.contains_key("ui.icons") {
                    return Err(Failure::new(
                        ErrorCode::CapabilityUnavailable,
                        "Marker icons require ui.icons",
                    ));
                }
                for path in [&icon.light, &icon.dark] {
                    if icons.contains_key(path) {
                        continue;
                    }
                    let resolved = safe_path(&self.assets, path, true).map_err(|_| {
                        Failure::new(ErrorCode::InvalidPath, "Marker icon is not an owned asset")
                    })?;
                    if !std::fs::metadata(&resolved)
                        .is_ok_and(|metadata| metadata.is_file() && metadata.len() <= 65536)
                    {
                        return Err(Failure::new(
                            ErrorCode::LimitExceeded,
                            "Marker icon exceeds 64 KiB",
                        ));
                    }
                    let bytes = std::fs::read(resolved).map_err(|_| {
                        Failure::new(ErrorCode::NotFound, "Marker icon unavailable")
                    })?;
                    crate::package::icons::svg(&bytes).map_err(|error| {
                        Failure::new(ErrorCode::InvalidRequest, error.to_string())
                    })?;
                    icons.insert(path.clone(), Arc::<[u8]>::from(bytes));
                }
            }
        }
        if icons.values().map(|bytes| bytes.len()).sum::<usize>() > 2 * 1024 * 1024 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Marker artwork budget exceeded",
            ));
        }
        let resource = match operation {
            Operation::Create { .. } => {
                if self.decorations.len() >= 16 {
                    return Err(Failure::new(
                        ErrorCode::LimitExceeded,
                        "Decoration collection quota exceeded",
                    ));
                }
                let Value::Resource(handle) = self.roots.open(RootKind::Decoration)? else {
                    unreachable!()
                };
                let authority = crate::EditorAuthority::new(
                    self.plugin_services.alive.clone(),
                    self.plugin_services.context.as_ref(),
                );
                let resource = Arc::new(crate::DecorationResource::new(handle.clone(), authority));
                // Persistent resource authority follows the same delegated lifetimes as the native request.
                if let Some(context) = &self.plugin_services.context {
                    self.plugin_services
                        .resources
                        .insert(handle.resource, (handle.clone(), context.clone()));
                }
                self.decorations.insert(handle.resource, resource.clone());
                resource
            }
            Operation::Replace { handle, .. } | Operation::Read { handle } => {
                if !matches!(self.roots.resolve(handle)?, RootKind::Decoration) {
                    return Err(Failure::new(
                        ErrorCode::InvalidHandle,
                        "Not a decoration collection",
                    ));
                }
                self.decorations
                    .get(&handle.resource)
                    .filter(|resource| resource.is_live())
                    .cloned()
                    .ok_or_else(|| {
                        Failure::new(ErrorCode::InvalidHandle, "Decoration owner was revoked")
                    })?
            }
        };
        Ok(Some((resource, icons)))
    }
}
