//! Host capability inventory; package identity never changes interface availability.
use plugin_protocol::{Manifest, api};
use semver::Version;

/// Production admission and instance preparation enforce the same current baseline.
pub(crate) fn require_current(manifest: &Manifest) -> anyhow::Result<()> {
    anyhow::ensure!(
        manifest.protocol == 7,
        "插件不兼容：协议 {} 已停止支持，请使用新版 SDK 更新插件（需要协议 7）。",
        manifest.protocol
    );
    // Registry restoration uses the same release floor as ZIP/development admission. Neither
    // capability negotiation nor an already saved grant makes a future host contract available.
    if let Some(minimum) = &manifest.minimum_host {
        anyhow::ensure!(minimum.len() <= 128, "Invalid minimum host version length");
        let minimum = Version::parse(minimum)
            .map_err(|error| anyhow::anyhow!("Invalid minimum host version: {error}"))?;
        let host = Version::parse(env!("CARGO_PKG_VERSION"))?;
        anyhow::ensure!(
            !host.cmp_precedence(&minimum).is_lt(),
            "插件不兼容：minimum host {minimum}, current host {host}; please update the host."
        );
    }
    anyhow::ensure!(manifest.activation.valid(), "Invalid background purpose");
    if manifest.activation.background.is_some() {
        anyhow::ensure!(
            manifest.component.is_some()
                && manifest
                    .api
                    .as_ref()
                    .is_some_and(|requirements| requirements
                        .required
                        .get("lifecycle.activation")
                        .is_some_and(|range| range.matches(&Version::new(1, 0, 0)))),
            "Background activation requires WASM and lifecycle.activation 1.0"
        );
    }
    // Registry records and new archives agree on which added schema semantics an old callback
    // may receive. Today's inventory matching ^1 does not prove that the author excluded 1.0.
    if manifest
        .settings
        .values()
        .any(|definition| definition.requires_configuration_1_1())
    {
        anyhow::ensure!(
            manifest
                .api
                .as_ref()
                .and_then(|requirements| requirements.required.get("configuration"))
                .is_some_and(|range| range.matches(&Version::new(1, 1, 0))
                    && !range.matches(&Version::new(1, 0, 0))),
            "Hot settings and secret references require configuration >=1.1"
        );
    }
    if manifest
        .settings
        .values()
        .any(|definition| definition.apply == plugin_protocol::settings::Apply::HotApply)
    {
        anyhow::ensure!(
            manifest.component.is_some() && manifest.settings_hook,
            "Hot settings require a WASM configuration hook"
        );
    }
    if manifest.settings.values().any(|definition| {
        matches!(
            definition.value_type,
            plugin_protocol::settings::SettingType::SecretReference
        )
    }) {
        anyhow::ensure!(
            manifest.permissions.contains("secrets")
                && manifest
                    .api
                    .as_ref()
                    .is_some_and(|requirements| requirements
                        .required
                        .contains_key("storage.secrets")),
            "Secret references require storage.secrets and secrets permission"
        );
    }
    negotiate(manifest)
        .map(|_| ())
        .map_err(|error| anyhow::anyhow!("插件 API 不兼容，请更新插件：{error:#}"))
}

/// Validate compatibility both when inspecting a package and before restoring an instance.
pub(crate) fn negotiate(manifest: &Manifest) -> anyhow::Result<api::Negotiated> {
    anyhow::ensure!(
        manifest.protocol == 7,
        "Only capability protocol 7 is supported"
    );
    let requirements = manifest
        .api
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing API requirements"))?;
    let available = [
        ("package.assets".into(), Version::new(1, 0, 0)),
        ("ui.native".into(), Version::new(1, 1, 0)),
        ("ui.interaction".into(), Version::new(1, 0, 0)),
        ("files.selection".into(), Version::new(1, 1, 0)),
        ("plugin.commands".into(), Version::new(1, 0, 0)),
        ("network.http".into(), Version::new(1, 0, 0)),
        ("storage.secrets".into(), Version::new(1, 0, 0)),
        ("lifecycle.timers".into(), Version::new(1, 0, 0)),
        // Continuous-work declarations are consumed by the same bounded, trusted demand manager.
        ("lifecycle.activation".into(), Version::new(1, 0, 0)),
        ("ui.tools".into(), Version::new(1, 0, 0)),
        ("ui.content_colors".into(), Version::new(1, 0, 0)),
        ("ui.styles".into(), Version::new(1, 0, 0)),
        ("ui.incremental".into(), Version::new(1, 0, 0)),
        ("ui.icons".into(), Version::new(1, 0, 0)),
        ("ui.richtext".into(), Version::new(1, 0, 0)),
        ("ui.code_highlighting".into(), Version::new(1, 0, 0)),
        ("ui.links".into(), Version::new(1, 0, 0)),
        ("ui.images".into(), Version::new(1, 0, 0)),
        ("ui.file_images".into(), Version::new(1, 0, 0)),
        ("ui.viewport".into(), Version::new(1, 0, 0)),
        ("editor.files".into(), Version::new(1, 0, 0)),
        ("ui.canvas".into(), Version::new(1, 1, 0)),
        ("ui.collections".into(), Version::new(1, 0, 0)),
        ("ui.tree".into(), Version::new(1, 0, 0)),
        ("ui.grid".into(), Version::new(1, 0, 0)),
        ("plugin.services".into(), Version::new(1, 1, 0)),
        ("workspace.files".into(), Version::new(1, 3, 0)),
        ("host.sdk".into(), Version::new(1, 0, 0)),
        ("storage.private".into(), Version::new(1, 2, 0)),
        ("storage.editor".into(), Version::new(1, 0, 0)),
        ("storage.migration".into(), Version::new(1, 0, 0)),
        ("editor.documents".into(), Version::new(1, 1, 0)),
        ("editor.virtual".into(), Version::new(1, 0, 0)),
        ("editor.diff".into(), Version::new(1, 0, 0)),
        ("editor.decorations".into(), Version::new(1, 0, 0)),
        ("editor.layout".into(), Version::new(1, 0, 0)),
        ("editor.viewport".into(), Version::new(1, 0, 0)),
        ("editor.edit".into(), Version::new(1, 1, 0)),
        ("editor.diagnostics".into(), Version::new(1, 0, 0)),
        ("editor.navigation".into(), Version::new(1, 0, 0)),
        ("editor.toolbar".into(), Version::new(1, 1, 0)),
        ("editor.images".into(), Version::new(1, 0, 0)),
        // 1.1 adds explicitly declared hot changes and namespace-only secret references.
        ("configuration".into(), Version::new(1, 1, 0)),
        ("ui.panels".into(), Version::new(1, 0, 0)),
        ("ui.clipboard".into(), Version::new(1, 0, 0)),
        // 1.7 presents an already-owned PTY; negotiation still grants no execution or UI authority.
        ("process".into(), Version::new(1, 7, 0)),
        // 1.2 owns native roots; 1.3 opts into immutable diagnostic URIs; 1.4 adds shared native
        // references/code actions/signature consumers, gated again by standard server advertisement.
        ("language.lsp".into(), Version::new(1, 4, 0)),
        // Pure references/actions/signature help remain explicitly experimental until wider use.
        ("language.direct".into(), Version::new(0, 1, 0)),
        ("language.completion".into(), Version::new(1, 0, 0)),
        // Independent providers negotiate editing and structure without replacing primary LSP ownership.
        ("language.formatting".into(), Version::new(1, 0, 0)),
        // 1.1 permits explicitly negotiated semantic linked ranges with different initial names.
        ("language.editing".into(), Version::new(1, 1, 0)),
        ("language.structure".into(), Version::new(1, 0, 0)),
        // 1.2 prepares declared native services asynchronously at method invocation.
        ("dependencies".into(), Version::new(1, 2, 0)),
    ]
    .into();
    Ok(requirements
        .negotiate(Version::new(1, 0, 0), &available)
        .map_err(anyhow::Error::msg)?)
}

/// Terminal presentation is an additive, negotiated process capability available to every identity.
#[cfg(test)]
mod terminal_tests {
    use super::*;
    #[test]
    fn process_terminal_presentation_is_negotiated_without_an_identity_exception() {
        for id in ["first-debug-consumer", "independent-terminal-consumer"] {
            let manifest: Manifest = serde_json::from_value(serde_json::json!({
                "id":id,"name":id,"version":"1.0.0","protocol":7,"component":"test.wasm",
                "permissions":["process.exec","ui.panels"],"storage_limit":1024,
                "api":{"base":"^1","required":{"process":">=1.7,<2"}}
            }))
            .unwrap();
            assert_eq!(
                negotiate(&manifest).unwrap().capabilities["process"],
                Version::new(1, 7, 0)
            );
        }
    }
}
