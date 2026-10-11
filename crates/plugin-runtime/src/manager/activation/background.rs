//! Explicit continuous purposes join the same bounded demand queue once per admitted configuration.
use super::*;

impl Manager {
    /// Failure is retained until an explicit recovery/configuration transition; polling is never a retry loop.
    pub(super) fn schedule_background_demands(&mut self) {
        let eligible = self.service_availabilities();
        for (entry, token, _) in eligible {
            if entry.manifest.activation.background.is_none()
                || entry.error.is_some()
                || self.activation.background.contains_key(&token)
                || self.activation.records[&(token.scope().into(), token.plugin().into())]
                    .materialized
            {
                continue;
            }
            match self.begin_demand(&token, DemandReason::Background, 120_000) {
                Ok(request) => {
                    self.activation.background.insert(token, request);
                }
                // An unreserved purpose may wait for capacity, but an attempted failure is not replayed.
                Err(error) if error.code == api::ErrorCode::LimitExceeded => break,
                Err(error) => {
                    if let Some(entry) = self.installed.get_mut(&entry.manifest.id) {
                        entry.error = Some(error.to_string());
                    }
                }
            }
        }
    }
}
