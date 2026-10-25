//! Profile-owned editor presentation preferences shared by every project and empty window.

use gpui_kit::{App, BorrowAppContext as _, Global};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

/// One mutually exclusive location for open-document navigation; document ownership stays unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TabPlacement {
    /// A horizontal strip above the document, matching existing sessions.
    #[default]
    Top,
    /// A horizontal strip below the document.
    Bottom,
    /// A vertical list inside the document region's left edge.
    Left,
    /// A vertical list inside the document region's right edge.
    Right,
    /// The existing dock system presents the list as the Open Files tool window.
    ToolWindow,
}

/// The disk record retains unrelated and future editor settings when changing one known field.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct StoredPreferences {
    /// Missing settings retain the current top-strip presentation.
    tab_placement: TabPlacement,
    /// Unknown keys belong to other preferences and must survive this writer unchanged.
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

impl StoredPreferences {
    /// Load `profile/editor.json`; only absence supplies defaults, while malformed values fail.
    fn load(profile: &Path) -> anyhow::Result<Self> {
        match std::fs::read(profile.join("editor.json")) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }

    /// Atomically save inside `profile`; replacement failures leave the existing destination intact.
    fn save(&self, profile: &Path) -> anyhow::Result<()> {
        use std::io::Write as _;
        std::fs::create_dir_all(profile)?;
        let mut temporary = tempfile::NamedTempFile::new_in(profile)?;
        temporary.write_all(&serde_json::to_vec_pretty(self)?)?;
        temporary.as_file().sync_all()?;
        temporary
            .persist(profile.join("editor.json"))
            .map_err(|error| error.error)?;
        Ok(())
    }
}

/// A failed read/write preserves the last usable presentation and exposes its error to settings.
#[derive(Default)]
struct ProfileState {
    /// Last successfully loaded or saved presentation; malformed first loads retain Top.
    placement: TabPlacement,
    /// The latest storage failure is visible until a successful retry.
    error: Option<String>,
}

/// Shared profile state broadcasts successful changes without writing workspace session preferences.
#[derive(Default)]
pub(crate) struct EditorPreferences {
    /// Explicit host-selected profiles stay isolated, including native application test fixtures.
    profiles: BTreeMap<PathBuf, ProfileState>,
}
impl Global for EditorPreferences {}

/// Read `profile` once into `cx`; failed loads remain visible and never rewrite the damaged file.
pub(crate) fn ensure_profile(profile: &Path, cx: &mut App) -> anyhow::Result<()> {
    if !cx.has_global::<EditorPreferences>() {
        cx.set_global(EditorPreferences::default());
    }
    if let Some(state) = cx.global::<EditorPreferences>().profiles.get(profile) {
        return match &state.error {
            Some(error) => Err(anyhow::anyhow!(error.clone())),
            None => Ok(()),
        };
    }
    let loaded = StoredPreferences::load(profile);
    let state = match &loaded {
        Ok(value) => ProfileState {
            placement: value.tab_placement,
            error: None,
        },
        Err(error) => ProfileState {
            error: Some(error.to_string()),
            ..Default::default()
        },
    };
    cx.update_global::<EditorPreferences, _>(|preferences, _| {
        preferences.profiles.insert(profile.to_owned(), state);
    });
    loaded.map(|_| ())
}

/// Return `profile`'s published placement in `cx`; an uninitialized or invalid first load uses Top.
pub(crate) fn placement(profile: &Path, cx: &App) -> TabPlacement {
    cx.try_global::<EditorPreferences>()
        .and_then(|preferences| preferences.profiles.get(profile))
        .map_or(TabPlacement::Top, |state| state.placement)
}

/// Return the latest storage error for `profile` in `cx`, without initiating filesystem work.
pub(crate) fn error(profile: &Path, cx: &App) -> Option<String> {
    cx.try_global::<EditorPreferences>()
        .and_then(|preferences| preferences.profiles.get(profile))
        .and_then(|state| state.error.clone())
}

/// Reload and atomically save `value` for `profile`, then publish it through `cx` to live windows.
/// Read or write failure preserves the current placement and every existing byte of damaged data.
pub(crate) fn set_placement(
    profile: &Path,
    value: TabPlacement,
    cx: &mut App,
) -> anyhow::Result<()> {
    // Retry reads even after a failed initialization, allowing an externally repaired file to recover.
    let _ = ensure_profile(profile, cx);
    let saved = (|| {
        let mut preferences = StoredPreferences::load(profile)?;
        preferences.tab_placement = value;
        preferences.save(profile)
    })();
    cx.update_global::<EditorPreferences, _>(|preferences, _| {
        let state = preferences.profiles.entry(profile.to_owned()).or_default();
        match &saved {
            Ok(()) => {
                state.placement = value;
                state.error = None;
            }
            Err(error) => state.error = Some(error.to_string()),
        }
    });
    saved
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{TestAppContext, gpui};

    /// Missing records and older empty records use Top without eagerly creating profile files.
    #[test]
    fn editor_preferences_default_without_creating_a_file() {
        let profile = tempfile::tempdir().unwrap();
        assert_eq!(
            StoredPreferences::load(profile.path())
                .unwrap()
                .tab_placement,
            TabPlacement::Top
        );
        assert_eq!(std::fs::read_dir(profile.path()).unwrap().count(), 0);
        std::fs::write(profile.path().join("editor.json"), b"{}").unwrap();
        assert_eq!(
            StoredPreferences::load(profile.path())
                .unwrap()
                .tab_placement,
            TabPlacement::Top
        );
    }

    /// Every supported choice survives restart while another preference's opaque data stays intact.
    #[gpui::test]
    fn editor_preferences_save_all_modes_and_preserve_unknown_fields(cx: &mut TestAppContext) {
        let profile = tempfile::tempdir().unwrap();
        let path = profile.path().join("editor.json");
        let extra = serde_json::json!({"font":{"family":"Example","size":16}, "future":[1,"x"]});
        std::fs::write(&path, serde_json::to_vec(&extra).unwrap()).unwrap();
        for value in [
            TabPlacement::Top,
            TabPlacement::Bottom,
            TabPlacement::Left,
            TabPlacement::Right,
            TabPlacement::ToolWindow,
        ] {
            cx.update(|cx| set_placement(profile.path(), value, cx).unwrap());
            assert_eq!(
                StoredPreferences::load(profile.path())
                    .unwrap()
                    .tab_placement,
                value
            );
            let stored: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            assert_eq!(stored["font"], extra["font"]);
            assert_eq!(stored["future"], extra["future"]);
            cx.update(|cx| {
                assert_eq!(placement(profile.path(), cx), value);
                assert!(error(profile.path(), cx).is_none());
            });
        }
    }

    /// Unknown variants and damaged JSON cannot replace the last valid choice or disk contents.
    #[gpui::test]
    fn editor_preferences_protect_damage_and_recover_after_repair(cx: &mut TestAppContext) {
        let profile = tempfile::tempdir().unwrap();
        let path = profile.path().join("editor.json");
        cx.update(|cx| set_placement(profile.path(), TabPlacement::Bottom, cx).unwrap());
        for bytes in [
            b"{broken".as_slice(),
            br#"{"tab_placement":"future_mode","font":16}"#,
            b"null",
        ] {
            std::fs::write(&path, bytes).unwrap();
            cx.update(|cx| {
                assert!(set_placement(profile.path(), TabPlacement::Left, cx).is_err());
                assert_eq!(placement(profile.path(), cx), TabPlacement::Bottom);
                assert!(error(profile.path(), cx).is_some());
            });
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
        std::fs::write(&path, b"{}").unwrap();
        cx.update(|cx| {
            set_placement(profile.path(), TabPlacement::Right, cx).unwrap();
            assert_eq!(placement(profile.path(), cx), TabPlacement::Right);
            assert!(error(profile.path(), cx).is_none());
        });
    }

    /// Independent explicit profiles cannot inherit a different project's user's selected mode.
    #[gpui::test]
    fn editor_preferences_isolate_profiles_and_failed_initial_loads(cx: &mut TestAppContext) {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let damaged = second.path().join("editor.json");
        std::fs::write(&damaged, b"{broken").unwrap();
        cx.update(|cx| {
            set_placement(first.path(), TabPlacement::ToolWindow, cx).unwrap();
            assert!(ensure_profile(second.path(), cx).is_err());
            assert_eq!(placement(first.path(), cx), TabPlacement::ToolWindow);
            assert_eq!(placement(second.path(), cx), TabPlacement::Top);
            assert!(error(second.path(), cx).is_some());
        });
        assert_eq!(std::fs::read(damaged).unwrap(), b"{broken");
    }

    /// A conflicting destination cannot be replaced or leave an abandoned temporary settings file.
    #[test]
    fn editor_preferences_failed_atomic_replace_preserves_destination() {
        let profile = tempfile::tempdir().unwrap();
        let destination = profile.path().join("editor.json");
        std::fs::create_dir(&destination).unwrap();
        let preserved = destination.join("keep.txt");
        std::fs::write(&preserved, b"existing data").unwrap();
        assert!(StoredPreferences::default().save(profile.path()).is_err());
        assert_eq!(std::fs::read(preserved).unwrap(), b"existing data");
        assert_eq!(std::fs::read_dir(profile.path()).unwrap().count(), 1);
    }
}
