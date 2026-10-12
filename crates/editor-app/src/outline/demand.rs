//! An actual native document consumes only its selected pure structure role and frozen source.
use super::*;
use crate::extensions::DemandWait;
use plugin_runtime::{Availability, LanguageRole, plugin_protocol::language::SourceSnapshot};

/// The same source version crosses preparation and description; cancellation never resamples it.
pub(super) struct Demand {
    key: String,
    availability: Availability,
    pub(super) source: SourceSnapshot,
    pub(super) ready: bool,
    /// A terminal failure belongs to this original role/source and remains visible until it changes.
    pub(super) failed: bool,
    wait: Option<DemandWait>,
}

impl EditorApp {
    /// Changing/closing the source drops its original wait. New actual document use may reserve a
    /// new source, while unavailable installed providers remain cold and failures are not retried.
    pub(super) fn sync_outline_demand(&mut self, cx: &mut Context<Self>) {
        let wanted = self
            .active_text_tab_index()
            .filter(|_| self.session_state.workspace_trusted)
            .and_then(|index| {
                let language = language::providers::language_for_path(self.tabs[index].path())?;
                let key = language::providers::structure_provider(&language)?;
                let plugin = key
                    .split_once('/')
                    .map_or(key.as_str(), |(plugin, _)| plugin);
                let availability = self.extensions.read(cx).availability(plugin)?;
                let document = self.plugin_document_version(index).ok()?;
                Some((key, availability, document))
            });
        let Some((key, availability, document)) = wanted else {
            self.outline.demand.take();
            self.outline.error = false;
            return;
        };
        if self
            .language_edits
            .bridge
            .as_ref()
            .is_some_and(|bridge| bridge.read(cx).has_pending())
        {
            self.outline.demand.take();
            return;
        }
        let changed = self.outline.demand.as_ref().is_none_or(|demand| {
            demand.key != key
                || demand.availability != availability
                || demand.source.document != document
        });
        if changed {
            self.outline.demand.take();
            self.outline.error = false;
            let source = SourceSnapshot {
                document: document.clone(),
                text: self.editor.read(cx).text().to_string(),
            };
            let guard = match self.capture_document_intent_guard(document) {
                Ok(guard) => guard,
                Err(error) => {
                    self.outline.error = true;
                    tracing::warn!(%error, "structure source changed");
                    return;
                }
            };
            let result = self.extensions.read(cx).demand_entry(
                availability.clone(),
                plugin_runtime::DemandReason::Language {
                    provider: key.clone(),
                    role: LanguageRole::Structure,
                },
                Some(guard),
            );
            let (wait, failed) = match result {
                Ok(wait) => (Some(wait), false),
                Err(error) => {
                    tracing::warn!(%error, "structure demand rejected");
                    (None, true)
                }
            };
            self.outline.demand = Some(Demand {
                key,
                availability,
                source,
                wait,
                ready: false,
                failed,
            });
        }
        if let Some(demand) = &mut self.outline.demand
            && let Some(result) = demand.wait.as_mut().and_then(DemandWait::try_take)
        {
            demand.wait.take();
            demand.ready = result.is_ok();
            demand.failed = result.is_err();
            if let Err(error) = result {
                self.outline.error = true;
                tracing::warn!(%error, "structure activation failed");
            }
        }
    }
}
