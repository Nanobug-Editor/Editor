//! Readonly, owned tool-distribution observations are separate from native execution and file grants.
use super::*;
use api::{ErrorCode, Failure, Value};
use std::{collections::BTreeMap, io::Read};

impl State {
    /// Inspect a local `source` without probes; `root_levels` selects its distribution directory.
    /// Metadata `files` stay under that canonical directory, with no process or filesystem handle issued.
    pub(super) fn inspect_tool_metadata(
        &self,
        source: &plugin_protocol::process::ToolSource,
        root_levels: u8,
        files: &[String],
    ) -> Result<Value, Failure> {
        if (!self.active && !self.language_hook) || self.roots.retired {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Tool observation requires a live instance or readonly preparation hook",
            ));
        }
        if !self.api.capabilities.contains_key("tools.inspect") {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "tools.inspect was not negotiated",
            ));
        }
        if !self.permissions.contains("process.exec") {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Tool observation requires execution consent",
            ));
        }
        if root_levels > 1 || files.len() > 16 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Tool metadata request exceeds its bounds",
            ));
        }
        let mut declaration = match source {
            plugin_protocol::process::ToolSource::Service { service } => {
                if !self
                    .permissions
                    .contains(&format!("process.service.{service}"))
                {
                    return Err(Failure::new(
                        ErrorCode::PermissionDenied,
                        "Declared tool service consent is required",
                    ));
                }
                self.services
                    .get(service)
                    .cloned()
                    .ok_or_else(|| Failure::new(ErrorCode::NotFound, "Undeclared tool service"))?
            }
            plugin_protocol::process::ToolSource::Program { program } => {
                plugin_protocol::process::Service {
                    on_demand: false,
                    platforms: vec![],
                    program: program.clone(),
                    args: vec![],
                    search_paths: vec![],
                    check_args: vec![],
                    installation: None,
                }
            }
        };
        if !declaration.valid() {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Invalid tool declaration",
            ));
        }
        // Readonly discovery must not run even a declaration's approved version-check arguments.
        declaration.check_args.clear();
        let managed_root =
            self.assets.ancestors().nth(3).ok_or_else(|| {
                Failure::new(ErrorCode::InvalidState, "Missing package storage root")
            })?;
        let prepared = declaration
            .installation
            .as_ref()
            .map(|plan| {
                if !self.permissions.contains("dependencies.prepare") {
                    return Err(Failure::new(
                        ErrorCode::PermissionDenied,
                        "Managed tool observation requires dependency consent",
                    ));
                }
                crate::dependencies::cached(managed_root, plan)
                    .map_err(|error| Failure::new(ErrorCode::NotFound, error.to_string()))
            })
            .transpose()?;
        let program = if let Some(prepared) = &prepared {
            prepared.program.clone()
        } else {
            crate::toolchains::resolve_service(&declaration)
                .map_err(|error| Failure::new(ErrorCode::NotFound, error.to_string()))?
        };
        let directory = program
            .parent()
            .and_then(|directory| directory.ancestors().nth(root_levels as usize))
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidPath,
                    "Missing tool distribution directory",
                )
            })?
            .canonicalize()
            .map_err(|error| Failure::new(ErrorCode::InvalidPath, error.to_string()))?;
        if directory.parent().is_none() {
            return Err(Failure::new(
                ErrorCode::InvalidPath,
                "A volume root is not a tool distribution",
            ));
        }
        let inside_managed_storage = directory.starts_with(
            managed_root
                .canonicalize()
                .map_err(|error| Failure::new(ErrorCode::InvalidPath, error.to_string()))?,
        );
        let inside_own_package = directory.starts_with(
            self.assets
                .canonicalize()
                .map_err(|error| Failure::new(ErrorCode::InvalidPath, error.to_string()))?,
        );
        let inside_own_data = directory.starts_with(
            self.data
                .canonicalize()
                .map_err(|error| Failure::new(ErrorCode::InvalidPath, error.to_string()))?,
        );
        let inside_prepared = prepared.as_ref().is_some_and(|prepared| {
            prepared.roots.values().any(|root| {
                root.canonicalize()
                    .is_ok_and(|root| directory.starts_with(root))
            })
        });
        if inside_managed_storage && !(inside_own_package || inside_own_data || inside_prepared) {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Tool metadata cannot read another plugin's managed files",
            ));
        }
        let mut metadata = BTreeMap::new();
        let mut total = 0usize;
        for relative in files {
            let path = safe_path(&directory, relative, true)
                .map_err(|error| Failure::new(ErrorCode::InvalidPath, error.to_string()))?;
            let file = std::fs::File::open(path)
                .map_err(|error| Failure::new(ErrorCode::NotFound, error.to_string()))?;
            let mut bytes = Vec::new();
            file.take(16 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|error| Failure::new(ErrorCode::OperationFailed, error.to_string()))?;
            total += bytes.len();
            if bytes.len() > 16 * 1024 || total > 64 * 1024 {
                return Err(Failure::new(
                    ErrorCode::LimitExceeded,
                    "Tool metadata exceeds its byte budget",
                ));
            }
            let text = String::from_utf8(bytes).map_err(|_| {
                Failure::new(ErrorCode::InvalidRequest, "Tool metadata must be UTF-8")
            })?;
            metadata.insert(relative.clone(), text);
        }
        Ok(Value::ToolMetadata(
            plugin_protocol::process::ToolMetadata {
                program: program.display().to_string(),
                root: directory.display().to_string(),
                files: metadata,
            },
        ))
    }
}
