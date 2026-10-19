//! Immutable selected plugin grammar declarations and isolated parser factories.
use gpui_kit::SharedString;
use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, RwLock},
};

/// Each factory creates a parser with its own WASM store; it never grants a native grammar fallback.
pub(crate) type LanguageParserFactory =
    Arc<dyn Fn() -> anyhow::Result<(tree_sitter::Parser, tree_sitter::Language)> + Send + Sync>;

/// Validated query resources and explicitly permitted injection targets for one qualified language.
#[derive(Clone)]
pub(crate) struct GrammarConfig {
    /// Native grammars remain absent; consumers may assert the plugin-only invariant.
    pub language: Option<tree_sitter::Language>,
    /// Tree-sitter highlight query supplied by the selected package.
    pub highlights: SharedString,
    /// Static injection query from the same package.
    pub injections: SharedString,
    /// Exact qualified language identities permitted by that injection query.
    pub injection_languages: Vec<SharedString>,
}
impl GrammarConfig {
    /// Create an inert language entry; publication adds validated queries and a parser factory.
    pub(crate) fn plain(_: impl Into<SharedString>) -> Self {
        Self {
            language: None,
            highlights: SharedString::default(),
            injections: SharedString::default(),
            injection_languages: Vec::new(),
        }
    }
}

/// A registry entry changes generation whenever its configuration or parser authority changes.
#[derive(Clone)]
pub(super) struct Entry {
    /// Immutable package query snapshot.
    pub config: GrammarConfig,
    /// Explicit plugin factory; absence is plain text.
    pub factory: Option<LanguageParserFactory>,
    /// Monotonic replacement marker prevents stale cached parsers surviving a hot update.
    pub generation: u64,
}

/// Process-wide immutable grammar resources, keyed by project scope and provider incarnation.
#[derive(Default)]
pub(crate) struct LanguageRegistry(RwLock<HashMap<String, Entry>>);
impl LanguageRegistry {
    /// Borrow the shared resource registry, which contains no built-in grammar registrations.
    pub(crate) fn singleton() -> &'static Self {
        static REGISTRY: LazyLock<LanguageRegistry> = LazyLock::new(LanguageRegistry::default);
        &REGISTRY
    }
    /// Publish a validated configuration for `language`, retiring the old factory until replaced.
    pub(crate) fn register(&self, language: &str, config: &GrammarConfig) {
        let mut entries = self.0.write().unwrap();
        let generation = entries
            .get(language)
            .map_or(1, |entry| entry.generation.wrapping_add(1));
        entries.insert(
            language.into(),
            Entry {
                config: config.clone(),
                factory: None,
                generation,
            },
        );
    }
    /// Attach the selected package's isolated parser factory to its current configuration.
    pub(crate) fn register_parser_factory(&self, language: &str, factory: LanguageParserFactory) {
        if let Some(entry) = self.0.write().unwrap().get_mut(language) {
            entry.factory = Some(factory);
            entry.generation = entry.generation.wrapping_add(1);
        }
    }
    /// Return immutable resources for a registered language; unknown languages remain unavailable.
    pub(crate) fn language(&self, language: &str) -> Option<GrammarConfig> {
        self.entry(language).map(|entry| entry.config)
    }
    /// Snapshot parser authority under a short lock; parser initialization never holds the registry.
    pub(super) fn entry(&self, language: &str) -> Option<Entry> {
        self.0.read().unwrap().get(language).cloned()
    }
}
