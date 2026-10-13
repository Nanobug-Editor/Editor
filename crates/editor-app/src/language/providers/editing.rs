//! Persist host user policy and optional per-language overrides without plugin configuration authority.
#[cfg(test)]
use super::legacy;
use super::{Providers, preferences::EditingPreferences};

/// Missing language overrides inherit the user-wide values; first use requires no settings file.
impl Providers {
    pub(crate) fn editing_preferences(&self, language: &str) -> EditingPreferences {
        let registry = self.read();
        let mut effective = EditingPreferences::default();
        for override_value in registry
            .saved
            .editing
            .get("*")
            .into_iter()
            .chain(registry.saved.editing.get(language))
        {
            if let Some(value) = override_value.format_on_save {
                effective.format_on_save = value;
            }
            if let Some(value) = override_value.linked_editing {
                effective.linked_editing = value;
            }
        }
        effective
    }
}

#[cfg(test)]
pub(crate) fn editing_preferences(language: &str) -> EditingPreferences {
    legacy().editing_preferences(language)
}

/// Settings can show which language values currently override the user's global preferences.
impl Providers {
    pub(crate) fn has_editing_override(&self, language: &str) -> bool {
        self.read().saved.editing.contains_key(language)
    }
}

#[cfg(test)]
pub(crate) fn has_editing_override(language: &str) -> bool {
    legacy().has_editing_override(language)
}

/// Change one native preference after persistence succeeds; `None` removes a language override.
/// `format_on_save` selects which setting changes. This grants no process or workspace authority.
impl Providers {
    pub(crate) fn set_editing_preference(
        &self,
        language: &str,
        format_on_save: bool,
        enabled: Option<bool>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            !language.is_empty() && language.len() <= 128,
            "Invalid preference language"
        );
        let mut registry = self.write();
        anyhow::ensure!(
            registry.error.is_none(),
            "Language preferences are unavailable"
        );
        let mut saved = registry.saved.clone();
        if let Some(enabled) = enabled {
            let value = saved.editing.entry(language.into()).or_default();
            if format_on_save {
                value.format_on_save = Some(enabled);
            } else {
                value.linked_editing = Some(enabled);
            }
        } else {
            saved.editing.remove(language);
        }
        anyhow::ensure!(
            saved.editing.len() <= 512,
            "Too many language editing overrides"
        );
        let mut next = (*registry).clone();
        next.saved = saved;
        registry.commit(next)?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) fn set_editing_preference(
    language: &str,
    format_on_save: bool,
    enabled: Option<bool>,
) -> anyhow::Result<()> {
    legacy().set_editing_preference(language, format_on_save, enabled)
}
