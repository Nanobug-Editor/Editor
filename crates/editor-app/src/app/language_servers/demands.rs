//! Only an actually open native document demands its selected primary language server.
use super::*;
use crate::extensions::DemandWait;
use plugin_runtime::Availability;
use plugin_runtime::plugin_protocol as protocol;
use std::collections::BTreeMap;

/// Closing the last original session drops its waiter; no text or alternate document model is kept.
#[derive(Default)]
pub(crate) struct DemandState {
    /// Readonly result authority expires on configuration or disk graph changes, not UI tab switches.
    pub(super) snapshot_epoch: super::snapshots::SnapshotEpoch,
    records: BTreeMap<String, Record>,
    /// Native adapters are unique per immutable runtime plan, including plugin control consumers.
    pub(super) transports: BTreeMap<usize, Arc<language_navigation::LanguageServer>>,
    pub(super) contexts: BTreeMap<PathBuf, ContextRecord>,
    /// Formatting consumers may retain a source while its primary analyzer is a different provider.
    pub(super) auxiliary: BTreeMap<
        (String, protocol::language::DocumentContext),
        (PathBuf, Arc<crate::extensions::LanguageDocumentLease>),
    >,
    /// Successful control operations may return opaque state needed by later bounded calls.
    /// The source and independently live callers own that state, never an individual request wait.
    pub(super) control_sources:
        BTreeMap<(String, protocol::language::DocumentContext), ControlSourceLease>,
    /// Weak indexing shares one final-release guard across UI, formatting and concurrent controls.
    pub(super) document_leases: BTreeMap<
        (String, String, protocol::language::DocumentContext),
        std::sync::Weak<crate::extensions::LanguageDocumentLease>,
    >,
}

/// One open source retains a shared transport while at least one sealed call chain remains live.
/// Store only identity and lifetime metadata; EditorState continues to own all editable text.
pub(super) struct ControlSourceLease {
    pub(super) incarnation: String,
    pub(super) callers: Vec<plugin_runtime::EditorAuthority>,
    pub(super) _lease: Arc<crate::extensions::LanguageDocumentLease>,
}

/// Per-source routing retains metadata and a plan lease, never text or another mutable document model.
pub(super) struct ContextRecord {
    pub(super) _lease: Arc<crate::extensions::LanguageDocumentLease>,
    pub(super) provider: String,
    pub(super) document: protocol::language::DocumentContext,
    pub(super) incarnation: String,
    pub(super) source_revision: u64,
    pub(super) availability: Availability,
    pub(super) pending: Option<DemandWait>,
    pub(super) error: Option<String>,
    pub(super) server: Option<Arc<language_navigation::LanguageServer>>,
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
                if self.extensions.read(cx).language_provider_has_contexts(key) {
                    continue;
                }
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
