//! Host asset composition adds plugin-owned files and branding to the shared local UI catalog.
use editor_ui::assets::UiAssets;
use gpui_kit::{AssetSource, Result, SharedString};
use std::borrow::Cow;

/// Resolve immutable product artwork without consulting the plugin registry.
pub(crate) fn file_icon(path: &str) -> Option<&'static [u8]> {
    match path {
        "branding/nanobug.png" => Some(include_bytes!("../../assets/branding/nanobug.png")),
        _ => editor_ui::assets::file_icon(path),
    }
}

/// The application owns dynamic plugin asset access; reusable controls use only `UiAssets`.
pub struct AppAssets;
impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = file_icon(path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        if let Some(bytes) = crate::extensions::contributions::asset(path) {
            return Ok(Some(Cow::Owned(bytes)));
        }
        UiAssets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = UiAssets.list(path)?;
        if "branding/nanobug.png".starts_with(path) {
            paths.push("branding/nanobug.png".into());
        }
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
