//! User-selected file associations change recognition without granting plugin execution.
use super::*;

/// Persist one bounded extension-to-language mapping before refreshing any open editor.
///
/// A leading dot and case are normalized. Removing a mapping uses `None`.
/// Unknown languages, unsafe extensions or unreadable preferences leave the current mapping intact.
impl Providers {
    pub(crate) fn associate_extension(
        &self,
        extension: &str,
        language: Option<&str>,
    ) -> anyhow::Result<()> {
        let extension = extension.trim();
        let extension = extension
            .strip_prefix('.')
            .unwrap_or(extension)
            .to_lowercase();
        anyhow::ensure!(valid_extension(&extension), "Invalid file extension");
        let mut registry = self.write();
        anyhow::ensure!(
            registry.error.is_none(),
            "Provider preferences are unavailable"
        );
        let mut saved = registry.saved.clone();
        if let Some(language) = language {
            let language = registry
                .code_language(language)
                .ok_or_else(|| anyhow::anyhow!("Language is unavailable"))?;
            saved.associations.insert(extension, language);
            anyhow::ensure!(
                saved.associations.len() <= 512,
                "Too many file associations"
            );
        } else {
            saved.associations.remove(&extension);
        }
        let mut next = (*registry).clone();
        next.saved = saved;
        next.highlight_epoch = next_highlight_epoch();
        registry.commit(next)?;
        let epoch = registry.highlight_epoch;
        drop(registry);
        super::super::code_highlighting::invalidate_prepared(self.scope(), epoch);
        Ok(())
    }
}

#[cfg(test)]
pub(crate) fn associate_extension(extension: &str, language: Option<&str>) -> anyhow::Result<()> {
    legacy().associate_extension(extension, language)
}

/// The native settings view displays stable user mappings even when a plugin is unavailable.
impl Providers {
    pub(crate) fn file_associations(&self) -> BTreeMap<String, String> {
        self.read().saved.associations.clone()
    }
}

#[cfg(test)]
pub(crate) fn file_associations() -> BTreeMap<String, String> {
    legacy().file_associations()
}

/// Match declarative selector constraints, including non-ASCII file extensions.
pub(super) fn valid_extension(extension: &str) -> bool {
    !extension.is_empty()
        && extension.len() <= 128
        && !extension.contains(['.', '/', '\\', '*', '?', ':', '[', ']'])
        && !extension.chars().any(char::is_control)
}
