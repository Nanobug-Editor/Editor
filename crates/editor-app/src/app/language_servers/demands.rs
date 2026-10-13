//! Only an actually open native document demands its selected primary language server.
use super::*;
use crate::extensions::DemandWait;
use plugin_runtime::Availability;
use plugin_runtime::plugin_protocol as protocol;
use std::collections::BTreeMap;

/// Closing the last original session drops its waiter; no text or alternate document model is kept.
#[derive(Default)]
pub(crate) struct DemandState {
    records: BTreeMap<String, Record>,
}

struct Record {
    key: String,
    availability: Availability,
    documents: Vec<String>,
    pending: Option<DemandWait>,
    error: Option<String>,
}

impl EditorApp {
    /// A prepared plan cannot start this native purpose before its own original receipt completes.
    pub(super) fn language_demand_is_ready(&self, language: &str) -> bool {
        self.language_server_demands
            .records
            .get(language)
            .is_some_and(|record| record.pending.is_none() && record.error.is_none())
    }

    /// Collect actual document purposes before reading prepared leases. Installed language metadata
    /// and formatter declarations alone do not start native tools or compile their components.
    pub(super) fn demand_open_language_servers(
        &mut self,
        selected: &BTreeMap<String, Option<String>>,
        cx: &App,
    ) -> BTreeMap<String, (String, Vec<String>)> {
        let mut wanted = BTreeMap::<String, (String, Vec<String>)>::new();
        if self.session_state.workspace_trusted {
            for (index, tab) in self.tabs.iter().enumerate() {
                if tab.text.is_none() || tab.virtual_document.is_some() {
                    continue;
                }
                let language = self.language_for_path(tab.path());
                let Some(key) = selected.get(&language).and_then(Option::as_ref) else {
                    continue;
                };
                let Some(document) = self.native_language_document_version(index) else {
                    continue;
                };
                wanted
                    .entry(language)
                    .or_insert_with(|| (key.clone(), Vec::new()))
                    .1
                    .push(document.id);
            }
        }
        self.language_server_demands
            .records
            .retain(|language, record| {
                wanted.get(language).is_some_and(|(key, documents)| {
                    *key == record.key && *documents == record.documents
                })
            });
        let panel = self.extensions.read(cx);
        for (language, (key, documents)) in &wanted {
            let plugin = key
                .split_once('/')
                .map_or(key.as_str(), |(plugin, _)| plugin);
            let Some(availability) = panel.availability(plugin) else {
                self.language_server_demands.records.remove(language);
                continue;
            };
            if self
                .language_server_demands
                .records
                .get(language)
                .is_some_and(|record| record.availability != availability)
            {
                self.language_server_demands.records.remove(language);
            }
            if !self.language_server_demands.records.contains_key(language) {
                let document = (0..self.tabs.len())
                    .filter_map(|index| self.native_language_document_version(index))
                    .find(|document| documents.first() == Some(&document.id));
                let result = document
                    .ok_or_else(|| {
                        protocol::api::Failure::new(
                            protocol::api::ErrorCode::StaleRevision,
                            "Language document closed",
                        )
                    })
                    .and_then(|document| self.capture_language_document_guard(document))
                    .and_then(|guard| {
                        panel.demand_language(key, plugin_runtime::LanguageRole::Lsp, Some(guard))
                    });
                let (pending, error) = match result {
                    Ok(wait) => (Some(wait), None),
                    Err(error) => (None, Some(error.to_string())),
                };
                self.language_server_demands.records.insert(
                    language.clone(),
                    Record {
                        key: key.clone(),
                        availability,
                        documents: documents.clone(),
                        pending,
                        error,
                    },
                );
            }
            let record = self
                .language_server_demands
                .records
                .get_mut(language)
                .unwrap();
            if let Some(result) = record.pending.as_mut().and_then(DemandWait::try_take) {
                record.pending.take();
                record.error = result.err().map(|error| error.to_string());
            }
        }
        wanted
    }

    /// Report cold preparation independently from an existing adapter's LSP handshake state.
    pub(super) fn publish_language_demand_states(&mut self) {
        for (language, record) in &self.language_server_demands.records {
            if let Some(error) = &record.error {
                self.language_service_states
                    .insert(language.clone(), ServiceLoadState::Failed(error.clone()));
            } else if record.pending.is_some() {
                self.language_service_states
                    .insert(language.clone(), ServiceLoadState::Loading);
            }
        }
    }
}
