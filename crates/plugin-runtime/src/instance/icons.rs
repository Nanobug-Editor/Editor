//! Native tree and marker artwork shares owned SVG admission; each consumer retains its own total budget.
use super::*;
use api::{ErrorCode, Failure};
use std::{collections::BTreeMap, io::Read, sync::Arc};

impl State {
    /// Admit both theme variants into a deduplicated immutable artwork set, after capability and grant
    /// checks. Resolve only owned assets and bound each file read before SVG validation; callers keep
    /// their resource ownership and aggregate artwork quotas at the publication boundary.
    pub(super) fn prepare_icon(
        &self,
        icon: &ui::ToolIcon,
        icons: &mut BTreeMap<String, Arc<[u8]>>,
    ) -> Result<(), Failure> {
        if !self.permissions.contains("assets.read") {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Native icons require assets.read",
            ));
        }
        if !self.api.capabilities.contains_key("ui.icons") {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "Native icons require ui.icons",
            ));
        }
        for path in [&icon.light, &icon.dark] {
            if icons.contains_key(path) {
                continue;
            }
            let resolved = safe_path(&self.assets, path, true).map_err(|_| {
                Failure::new(ErrorCode::InvalidPath, "Icon is not an owned package asset")
            })?;
            let file = std::fs::File::open(resolved)
                .map_err(|_| Failure::new(ErrorCode::NotFound, "Icon asset unavailable"))?;
            if !file
                .metadata()
                .is_ok_and(|metadata| metadata.is_file() && metadata.len() <= 65536)
            {
                return Err(Failure::new(
                    ErrorCode::LimitExceeded,
                    "Icon exceeds 64 KiB",
                ));
            }
            // The read is bounded independently of metadata, so an external asset race cannot allocate
            // beyond the per-image limit. Nothing is published until the complete page/layer is valid.
            let mut bytes = Vec::new();
            file.take(65537)
                .read_to_end(&mut bytes)
                .map_err(|_| Failure::new(ErrorCode::NotFound, "Icon asset unavailable"))?;
            if bytes.len() > 65536 {
                return Err(Failure::new(
                    ErrorCode::LimitExceeded,
                    "Icon exceeds 64 KiB",
                ));
            }
            crate::package::icons::svg(&bytes)
                .map_err(|error| Failure::new(ErrorCode::InvalidRequest, error.to_string()))?;
            icons.insert(path.clone(), Arc::<[u8]>::from(bytes));
        }
        Ok(())
    }
}
