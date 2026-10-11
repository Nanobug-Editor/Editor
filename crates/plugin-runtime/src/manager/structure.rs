//! Publish independent structure callbacks without requiring an LSP service or process permission.
use super::*;
use crate::StructureProvider;
use std::sync::Arc;

pub(super) struct Prepared {
    pub(super) fingerprint: language::Fingerprint,
    pub(super) provider: Result<Arc<StructureProvider>, String>,
}

impl Drop for Prepared {
    fn drop(&mut self) {
        if let Ok(provider) = &self.provider {
            provider.retire();
        }
    }
}

impl Manager {
    /// Publish existing demanded executors without preparing all enabled structure declarations.
    pub fn structure_providers(
        &mut self,
    ) -> BTreeMap<String, Result<Arc<StructureProvider>, String>> {
        let expected = self.role_fingerprints();
        self.structure_providers
            .retain(|key, prepared| expected.get(key) == Some(&prepared.fingerprint));
        self.structure_providers
            .iter()
            .map(|(key, prepared)| (key.clone(), prepared.provider.clone()))
            .collect()
    }

    /// Revocation covers disable/uninstall, successful cutover, rollback and configuration replacement.
    pub(super) fn retire_structure_providers(&mut self, id: &str) {
        self.structure_providers
            .retain(|key, _| !key.starts_with(&format!("{id}/")));
    }

    /// Invalidate UI-held Arcs explicitly before forgetting the selected workspace's executors.
    pub(super) fn clear_structure_providers(&mut self) {
        for prepared in self.structure_providers.values() {
            if let Ok(provider) = &prepared.provider {
                provider.retire();
            }
        }
        self.structure_providers.clear();
    }

    /// Each active pure executor owns one bounded memory resource, independent of native process counts.
    pub(super) fn structure_resource_count(&self) -> usize {
        self.structure_providers
            .values()
            .filter_map(|item| item.provider.as_ref().ok())
            .filter(|provider| provider.is_active())
            .count()
    }
}
