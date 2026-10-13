//! Preserve the existing standalone fixture interface while native call sites retain explicit owners.
use super::*;
pub(crate) fn refresh(
    root: &Path,
    entries: Vec<(String, PathBuf, Vec<LanguageDefinition>, Vec<Highlighter>)>,
    services: Vec<(
        String,
        Vec<plugin_runtime::plugin_protocol::language::Provider>,
    )>,
) {
    legacy().refresh(root, entries, services)
}

pub(crate) fn refresh_with_structures(
    root: &Path,
    entries: Vec<(String, PathBuf, Vec<LanguageDefinition>, Vec<Highlighter>)>,
    services: Vec<(
        String,
        Vec<plugin_runtime::plugin_protocol::language::Provider>,
    )>,
    structures: Vec<(
        String,
        Vec<plugin_runtime::plugin_protocol::structure::Provider>,
    )>,
) {
    legacy().refresh_with_structures(root, entries, services, structures)
}

pub(crate) fn refresh_with_language_features(
    root: &Path,
    entries: Vec<(String, PathBuf, Vec<LanguageDefinition>, Vec<Highlighter>)>,
    services: Vec<(
        String,
        Vec<plugin_runtime::plugin_protocol::language::Provider>,
    )>,
    structures: Vec<(
        String,
        Vec<plugin_runtime::plugin_protocol::structure::Provider>,
    )>,
    direct: Vec<(
        String,
        Vec<plugin_runtime::plugin_protocol::direct::Provider>,
    )>,
) {
    legacy().refresh_with_language_features(root, entries, services, structures, direct)
}

pub(crate) fn configure(root: &Path, workspace: &Path) {
    owner::configure_fixture(root, workspace)
}

pub(crate) fn rows() -> Vec<ProviderRow> {
    legacy().rows()
}

pub(crate) fn error() -> Option<String> {
    legacy().error()
}

pub(crate) fn choose(scope: Scope, key: &str, provider: Option<&str>) -> anyhow::Result<()> {
    legacy().choose(scope, key, provider)
}

pub(crate) fn language_for_path(path: &Path) -> Option<String> {
    legacy().language_for_path(path)
}

pub(crate) fn grammars() -> Vec<GrammarProvider> {
    legacy().grammars()
}

pub(crate) fn language_servers() -> BTreeMap<String, Option<String>> {
    legacy().language_servers()
}

pub(crate) fn structure_provider(language: &str) -> Option<String> {
    legacy().structure_provider(language)
}

pub(crate) fn language_feature_provider(
    language: &str,
    feature: plugin_runtime::plugin_protocol::direct::Feature,
) -> Option<String> {
    legacy().language_feature_provider(language, feature)
}

pub(crate) fn has_language_feature(
    language: &str,
    feature: plugin_runtime::plugin_protocol::direct::Feature,
) -> bool {
    legacy().has_language_feature(language, feature)
}

pub(crate) fn is_current(provider: &GrammarProvider) -> bool {
    legacy().is_current(provider)
}

pub(in crate::language) fn code_provider(language: &str) -> Option<(GrammarProvider, u64)> {
    legacy().code_provider(language)
}

pub(in crate::language) fn code_epoch() -> u64 {
    legacy().code_epoch()
}

pub(crate) fn feature_generation() -> u64 {
    legacy().feature_generation()
}

pub(in crate::language) fn code_is_current(provider: &GrammarProvider, epoch: u64) -> bool {
    legacy().code_is_current(provider, epoch)
}

pub(crate) fn handles_path(path: &Path) -> bool {
    legacy().handles_path(path)
}

pub(crate) fn languages() -> std::collections::BTreeSet<String> {
    legacy().languages()
}
