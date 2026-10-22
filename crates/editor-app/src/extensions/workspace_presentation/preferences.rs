//! User-owned presentation settings preserve provider choices and stable entry identities.
use super::*;
use serde::{Deserialize, Serialize};
use std::io::Write as _;

/// Colors and title arrangement are user-wide; the one filename provider is selected per workspace.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Preferences {
    /// Master filename-color switch does not alter the source's data or unsaved-edit markers.
    pub enabled: bool,
    /// Exact publisher IDs; unavailable choices remain saved instead of silently falling back.
    pub providers: BTreeMap<String, String>,
    /// Explicit entry order precedes unconfigured defaults; retired IDs remain remembered.
    pub order: Vec<String>,
    /// User-hidden full plugin/entry identities, independent of any panel visibility.
    pub hidden: BTreeSet<String>,
    /// Optional RGB overrides in the positive/informative/negative semantic order.
    pub light: [Option<u32>; 3],
    /// Separate dark overrides preserve readability through appearance switches.
    pub dark: [Option<u32>; 3],
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            enabled: true,
            providers: Default::default(),
            order: Vec::new(),
            hidden: Default::default(),
            light: [None; 3],
            dark: [None; 3],
        }
    }
}
impl Preferences {
    /// Load only this host-selected profile, retaining damaged files instead of overwriting defaults.
    pub(super) fn load(profile: &Path) -> anyhow::Result<Self> {
        match std::fs::read(profile.join("workspace-presentation.json")) {
            Ok(bytes) => {
                let value: Self = serde_json::from_slice(&bytes)?;
                anyhow::ensure!(
                    value
                        .light
                        .iter()
                        .chain(&value.dark)
                        .flatten()
                        .all(|color| *color <= 0xffffff),
                    "Presentation colors require RGB values"
                );
                Ok(value)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }
    /// Save atomically inside the user profile; project configuration cannot change these controls.
    pub(super) fn save(&self, profile: &Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(profile)?;
        let mut temporary = tempfile::NamedTempFile::new_in(profile)?;
        temporary.write_all(&serde_json::to_vec_pretty(self)?)?;
        temporary.as_file().sync_all()?;
        temporary
            .persist(profile.join("workspace-presentation.json"))
            .map_err(|error| error.error)?;
        Ok(())
    }
    /// Resolve semantic meaning through the current theme unless a user RGB override exists.
    pub(super) fn color(&self, tone: tree::Tone, cx: &App) -> gpui_kit::Hsla {
        let index = match tone {
            tree::Tone::Positive => 0,
            tree::Tone::Informative => 1,
            tree::Tone::Negative => 2,
        };
        let overrides = if cx.theme().is_dark() {
            &self.dark
        } else {
            &self.light
        };
        if let Some(color) = overrides[index] {
            return gpui_kit::rgb(color).into();
        }
        match tone {
            tree::Tone::Positive => cx.theme().success,
            tree::Tone::Negative => cx.theme().danger,
            tree::Tone::Informative => gpui_kit::rgb(if cx.theme().is_dark() {
                0x60a5fa
            } else {
                0x1d4ed8
            })
            .into(),
        }
    }
}
