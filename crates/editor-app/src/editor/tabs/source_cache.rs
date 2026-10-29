//! Retired readonly native sources may be reused without retaining any document capability.
use super::TextTab;
use crate::*;

/// Small per-window pool; only hidden preview replacement enters it, never a user's editable file.
#[derive(Default)]
pub(crate) struct SourceCache {
    entries: std::collections::VecDeque<Entry>,
    /// Native timing probes distinguish text-cache hits from actual editor/layout reuse.
    #[cfg(test)]
    hits: usize,
}
/// A resource-free native source remains bound to its original live issuer and grammar generation.
struct Entry {
    owner: plugin_runtime::EditorAuthority,
    epoch: u64,
    language: String,
    inserted: Instant,
    bytes: usize,
    text: TextTab,
}
impl SourceCache {
    /// Read-only observation; never influences admission, source identity or reuse decisions.
    #[cfg(test)]
    pub(crate) fn hits(&self) -> usize {
        self.hits
    }
    /// Retire stale issuers/grammars and expire idle entries; no old handle can be resurrected.
    pub(crate) fn prune(&mut self, epoch: u64) {
        self.entries.retain(|entry| {
            entry.owner.is_live()
                && entry.epoch == epoch
                && entry.inserted.elapsed() < Duration::from_secs(60)
        });
    }
    /// Reuse only identical immutable contents from the same issuer and selected language.
    pub(crate) fn take(
        &mut self,
        owner: &plugin_runtime::EditorAuthority,
        epoch: u64,
        language: &str,
        contents: &str,
        cx: &App,
    ) -> Option<TextTab> {
        self.prune(epoch);
        let index = self.entries.iter().position(|entry| {
            entry.owner.same_owner(owner)
                && entry.language == language
                && entry.bytes == contents.len()
                && entry
                    .text
                    .editor
                    .read(cx)
                    .text()
                    .chars()
                    .eq(contents.chars())
        })?;
        let entry = self.entries.remove(index)?;
        #[cfg(test)]
        {
            self.hits += 1;
        }
        Some(entry.text)
    }
    /// Keep at most eight native sources and 8 MiB of original text, dropping oldest state first.
    pub(crate) fn remember(
        &mut self,
        owner: plugin_runtime::EditorAuthority,
        epoch: u64,
        language: String,
        text: TextTab,
        cx: &App,
    ) {
        self.prune(epoch);
        let bytes = text.editor.read(cx).text().len();
        if !owner.is_live() || bytes > 1024 * 1024 {
            return;
        }
        self.entries.push_back(Entry {
            owner,
            epoch,
            language,
            inserted: Instant::now(),
            bytes,
            text,
        });
        while self.entries.len() > 8
            || self.entries.iter().map(|entry| entry.bytes).sum::<usize>() > 8 * 1024 * 1024
        {
            self.entries.pop_front();
        }
    }
}
impl EditorApp {
    /// Preview replacement releases the old resource normally but can keep its readonly native layout.
    pub(crate) fn recycle_comparison_source(&mut self, index: usize, cx: &mut App) {
        let tab = &mut self.tabs[index];
        let Some(source) = &tab.virtual_document else {
            return;
        };
        if !source.hidden
            || !source.resource.is_live()
            || tab.text.as_ref().is_none_or(|text| {
                !text.session.is_readonly()
                    || text.session.is_dirty()
                    || text.highlighter.has_structure_folds()
                    || !text
                        .editor
                        .read(cx)
                        .display_map()
                        .folded_ranges()
                        .is_empty()
            })
        {
            return;
        }
        let mut text = tab.text.take().unwrap();
        // Reusing a native entity must not rebind an old document's decorations or delayed results.
        // Revoke before synchronize so the native paint collections are disposed as well.
        let markers = text.highlighter.markers();
        markers.borrow().revoke();
        markers.borrow_mut().synchronize(None, cx);
        text.diagnostics = Default::default();
        text.definition_highlight_generation = text.definition_highlight_generation.wrapping_add(1);
        text.definition_highlight.clear(cx);
        text.editor.update(cx, |editor, cx| {
            editor.set_search_query("", false, cx);
            editor.close_search(cx);
            if let Some(diagnostics) = editor.diagnostics_mut() {
                diagnostics.clear();
            }
            editor.clear_diagnostic_popover(cx);
        });
        self.comparison_source_cache.remember(
            source.owner.clone(),
            self.providers.feature_generation(),
            source.language.clone(),
            text,
            cx,
        );
    }
}
