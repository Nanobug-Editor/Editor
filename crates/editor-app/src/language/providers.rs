//! Immutable provider snapshots separate recognition from grammar choice and stale task publication.
use plugin_runtime::plugin_protocol::settings::Scope;
use plugin_schema::{Highlighter, LanguageDefinition};
mod associations;
#[cfg(test)]
mod context_tests;
mod editing;
#[cfg(test)]
mod fixtures;
mod formatting;
mod preferences;
mod selection;
#[cfg(test)]
pub(crate) use associations::associate_extension;
#[cfg(test)]
pub(crate) use editing::set_editing_preference;
#[cfg(test)]
pub(crate) use fixtures::*;
use formatting::FormatterPreferenceError;
#[cfg(test)]
pub(crate) use formatting::formatters;
use preferences::Saved;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{
        LazyLock, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

/// Package identity and content-addressed root travel with every background grammar load.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GrammarProvider {
    pub owner: String,
    pub root: PathBuf,
    pub declaration: Highlighter,
}

#[derive(Clone, Default)]
struct Registry {
    root: PathBuf,
    workspace: String,
    saved: Saved,
    error: Option<String>,
    recognizers: BTreeMap<String, Vec<(String, LanguageDefinition)>>,
    highlighters: BTreeMap<String, Vec<GrammarProvider>>,
    language_servers: BTreeMap<String, Vec<String>>,
    /// Formatting and structure remain selectable without changing analysis or grammar choices.
    formatters: BTreeMap<String, Vec<String>>,
    /// Invalid explicit formatting values retain their scope without poisoning other preference edits.
    formatter_preference_errors: BTreeMap<String, FormatterPreferenceError>,
    structure_providers: BTreeMap<String, Vec<String>>,
    /// Experimental direct feature roles share persistent choice without starting another LSP.
    language_features: BTreeMap<String, Vec<String>>,
    /// Retain a valid selected provider when another package joins the candidate set.
    selected: BTreeMap<String, String>,
    /// Never reuse an earlier selection generation, including disable/re-enable and A/B/A choices.
    highlight_epoch: u64,
}
mod owner;
pub(crate) use owner::Providers;
#[cfg(test)]
pub(crate) use owner::legacy;
static NEXT_HIGHLIGHT_EPOCH: AtomicU64 = AtomicU64::new(0);

/// Configuration rebuilds cannot reset this process-wide generation counter.
fn next_highlight_epoch() -> u64 {
    NEXT_HIGHLIGHT_EPOCH
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |epoch| {
            epoch.checked_add(1)
        })
        .expect("language provider generation exhausted")
        + 1
}

impl Registry {
    fn resolve(&mut self) {
        self.selected.clear();
        self.formatter_preference_errors.clear();
        // Verification is tied to current explicit values, never a history of installed providers.
        self.saved
            .formatter_choices
            .prune(&self.saved.user, &self.saved.projects);
        for (key, candidates) in self.candidates() {
            if !self.validate_formatter_explicit(&key, &candidates) {
                continue;
            }
            let explicit = self
                .saved
                .projects
                .get(&self.workspace)
                .and_then(|m| m.get(&key))
                .or_else(|| self.saved.user.get(&key));
            let remembered = self
                .saved
                .automatic
                .get(&self.workspace)
                .and_then(|m| m.get(&key));
            let choice = explicit
                .filter(|id| candidates.contains(id))
                .or_else(|| remembered.filter(|id| candidates.contains(id)))
                .cloned()
                .or_else(|| (candidates.len() == 1).then(|| candidates[0].clone()));
            if let Some(choice) = choice {
                self.saved
                    .automatic
                    .entry(self.workspace.clone())
                    .or_default()
                    .insert(key.clone(), choice.clone());
                self.selected.insert(key, choice);
            }
        }
    }
    /// Selection keys separate file recognition from the language's highlight provider.
    fn candidates(&self) -> BTreeMap<String, Vec<String>> {
        let mut rows = self.language_features.clone();
        for (role, declarations) in [
            ("formatter", &self.formatters),
            ("structure", &self.structure_providers),
        ] {
            for (language, providers) in declarations {
                rows.insert(format!("{role}:{language}"), providers.clone());
            }
        }
        for (language, providers) in &self.language_servers {
            rows.insert(format!("lsp:{language}"), providers.clone());
        }
        for (key, definitions) in &self.recognizers {
            rows.insert(
                format!("recognition:{key}"),
                definitions
                    .iter()
                    .map(|(owner, definition)| format!("{owner}/{}", definition.id))
                    .collect(),
            );
        }
        for (language, providers) in &self.highlighters {
            rows.insert(
                format!("highlight:{language}"),
                providers
                    .iter()
                    .map(|provider| format!("{}/{}", provider.owner, provider.declaration.id))
                    .collect(),
            );
        }
        // A loaded invalid formatter must remain visible and resettable even with no live candidates.
        for key in self
            .saved
            .user
            .keys()
            .chain(
                self.saved
                    .projects
                    .get(&self.workspace)
                    .into_iter()
                    .flat_map(|layer| layer.keys()),
            )
            .chain(
                self.saved
                    .automatic
                    .get(&self.workspace)
                    .into_iter()
                    .flat_map(|layer| layer.keys()),
            )
            .filter(|key| key.starts_with("formatter:"))
        {
            rows.entry(key.clone()).or_default();
        }
        rows
    }

    /// Canonical IDs win over aliases; ambiguous display names never depend on install ordering.
    fn code_language(&self, language: &str) -> Option<String> {
        if language.len() > 128 {
            return None;
        }
        let language = language.trim().to_lowercase();
        if language.is_empty() {
            return None;
        }
        let definitions = || {
            self.recognizers
                .values()
                .flatten()
                .map(|(_, definition)| definition)
        };
        if self.highlighters.contains_key(&language)
            || definitions().any(|definition| definition.id == language)
        {
            return Some(language);
        }
        let mut candidates = definitions()
            .filter(|definition| definition.name.to_lowercase() == language)
            .map(|definition| definition.id.clone())
            .collect::<BTreeSet<_>>();
        let extension = format!("ext:{}", language.strip_prefix('.').unwrap_or(&language));
        if let Some(definitions) = self.recognizers.get(&extension) {
            // Recognition preferences resolve shared extensions independently of grammar choice.
            let owner = self.selected.get(&format!("recognition:{extension}"))?;
            let (_, definition) = definitions
                .iter()
                .find(|(owner_id, definition)| format!("{owner_id}/{}", definition.id) == *owner)?;
            candidates.insert(definition.id.clone());
        }
        (candidates.len() == 1).then(|| candidates.into_iter().next().unwrap())
    }

    /// Resolve one language's selected package without consulting the upstream parser registry.
    fn grammar(&self, language: &str) -> Option<&GrammarProvider> {
        let selected = self.selected.get(&format!("highlight:{language}"))?;
        self.highlighters
            .get(language)?
            .iter()
            .find(|provider| format!("{}/{}", provider.owner, provider.declaration.id) == *selected)
    }
}

/// Workspace context is host-owned; reading repository files cannot set these preferences.

/// Windows verbatim and normal paths identify the same host store, preserving its active workspace.
fn same_root(a: &Path, b: &Path) -> bool {
    a.canonicalize().unwrap_or_else(|_| a.into()) == b.canonicalize().unwrap_or_else(|_| b.into())
}

fn configured(root: &Path, workspace: &str) -> Registry {
    let (saved, error) = match Saved::read(root) {
        Ok(saved) => (saved, None),
        Err(error) => (Saved::default(), Some(format!("{error:#}"))),
    };
    Registry {
        root: root.into(),
        workspace: workspace.into(),
        saved,
        error,
        highlight_epoch: next_highlight_epoch(),
        ..Default::default()
    }
}

/// Settings rendering uses this immutable snapshot and never reads package files while drawing.
pub(crate) struct ProviderRow {
    pub key: String,
    pub candidates: Vec<String>,
    pub selected: Option<String>,
    pub source: &'static str,
    /// Only never-validated explicit values are errors; normally withdrawn choices retain fallback rows.
    pub configuration_error: Option<FormatterPreferenceError>,
}
