//! Embeds shared editor icons for GPUI's asset loader; the host supplies product branding.

use gpui_kit::{AssetSource, Result, SharedString, assets::Assets};
use std::borrow::Cow;

// The default bundle omits these menu, configuration, shortcut and message actions; embed their catalog SVGs.
gpui_kit::assets::icon_assets!(
    SupplementalIcons,
    [Menu, Lock, SquarePen, Keyboard, RotateCcw, Broom]
);

macro_rules! file_icons {
    ($($path:literal => $file:literal),+ $(,)?) => {
        const FILE_ICON_PATHS: &[&str] = &[$($path),+];

        // Embed bundled system and plugin icons for synchronous asset resolution.
        /// Resolve bundled UI artwork by asset path; unknown paths return None and bytes stay borrowed.
        pub fn file_icon(path: &str) -> Option<&'static [u8]> {
            match path {
                $($path => Some(include_bytes!($file).as_slice()),)+
                _ => None,
            }
        }
    };
}

file_icons! {
    // Supplied execution outlines share one visual weight and inherit the buttons' theme colors.
    "icons/run-build.svg" => "../../assets/icons/run-build.svg",
    "icons/run-start.svg" => "../../assets/icons/run-start.svg",
    "icons/run-debug.svg" => "../../assets/icons/run-debug.svg",
    "icons/run-stop.svg" => "../../assets/icons/run-stop.svg",
    "icons/run-chevron-down.svg" => "../../assets/icons/run-chevron-down.svg",
    // Custom explorer actions share the title's theme color through monochrome SVG rendering.
    "icons/explorer-locate.svg" => "../../assets/icons/explorer-locate.svg",
    "icons/explorer-collapse-all.svg" => "../../assets/icons/explorer-collapse-all.svg",
    "icons/explorer-expand-all.svg" => "../../assets/icons/explorer-expand-all.svg",
    // The supplied outline artwork shares the window control group's theme color.
    "icons/outline-panel.svg" => "../../assets/icons/outline-panel.svg",
    "file-icons/config.svg" => "../../assets/file-icons/config.svg",
    "file-icons/config_dark.svg" => "../../assets/file-icons/config_dark.svg",
    "file-icons/file.svg" => "../../assets/file-icons/file.svg",
    "file-icons/file_dark.svg" => "../../assets/file-icons/file_dark.svg",
    "file-icons/folder.svg" => "../../assets/file-icons/folder.svg",
    "file-icons/folder_dark.svg" => "../../assets/file-icons/folder_dark.svg",
    "file-icons/json.svg" => "../../assets/file-icons/json.svg",
    "file-icons/json_dark.svg" => "../../assets/file-icons/json_dark.svg",
    "file-icons/text.svg" => "../../assets/file-icons/text.svg",
    "file-icons/text_dark.svg" => "../../assets/file-icons/text_dark.svg",
}

/// Combines local editor file icons and GPUI Kit's monochrome UI icons.
pub struct UiAssets;

impl AssetSource for UiAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(icon) = file_icon(path) {
            return Ok(Some(Cow::Borrowed(icon)));
        }
        if let Some(icon) = SupplementalIcons.load(path)? {
            return Ok(Some(icon));
        }
        Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = Assets.list(path)?;
        paths.extend(SupplementalIcons.list(path)?);
        paths.extend(
            FILE_ICON_PATHS
                .iter()
                .filter(|candidate| candidate.starts_with(path))
                .map(|candidate| SharedString::from(*candidate)),
        );
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_every_embedded_file_icon() {
        for path in FILE_ICON_PATHS {
            assert!(file_icon(path).is_some(), "missing embedded icon: {path}");
        }
    }

    /// Every header action must resolve its SVG from the application bundle in either theme.
    #[test]
    fn exposes_explorer_title_icons() {
        for path in [
            "icons/explorer-locate.svg",
            "icons/explorer-collapse-all.svg",
            "icons/explorer-expand-all.svg",
        ] {
            assert!(UiAssets.load(path).unwrap().is_some());
            assert!(
                UiAssets
                    .list("icons/")
                    .unwrap()
                    .iter()
                    .any(|entry| entry == path)
            );
        }
    }

    /// Icon-only execution controls must resolve every asset in the application bundle.
    #[test]
    fn exposes_run_title_icons() {
        for path in [
            "icons/run-build.svg",
            "icons/run-start.svg",
            "icons/run-debug.svg",
            "icons/run-stop.svg",
        ] {
            assert!(
                UiAssets.load(path).unwrap().is_some(),
                "missing run action icon: {path}"
            );
        }
    }

    /// Menu, configuration, shortcut and message controls resolve from the selected extra icon bundle.
    #[test]
    fn exposes_supplemental_action_icons() {
        for icon in [
            gpui_kit::assets::IconName::Menu,
            gpui_kit::assets::IconName::Lock,
            gpui_kit::assets::IconName::SquarePen,
            gpui_kit::assets::IconName::Keyboard,
            gpui_kit::assets::IconName::RotateCcw,
            gpui_kit::assets::IconName::Broom,
        ] {
            let path = icon.path();
            assert!(UiAssets.load(&path).unwrap().is_some());
            assert!(
                UiAssets
                    .list("icons/")
                    .unwrap()
                    .iter()
                    .any(|entry| entry == &path)
            );
        }
    }
}
