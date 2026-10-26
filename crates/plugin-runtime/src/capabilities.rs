//! Host capability inventory; package identity never changes interface availability.
//! Technical rejection details are English; native UI localizes the operation and recovery context.
use plugin_protocol::{Manifest, api};
use semver::Version;

/// One inventory value also governs language declaration floors as additive minors evolve.
pub(crate) const LANGUAGE_LSP_VERSION: Version = plugin_protocol::language::CAPABILITY_VERSION;
/// File refactors add explicit workspace authority to the earlier native-source edit contract.
pub(crate) const LANGUAGE_EDITING_VERSION: Version = Version::new(1, 3, 0);

/// Production admission and instance preparation enforce the same current baseline.
pub(crate) fn require_current(manifest: &Manifest) -> anyhow::Result<()> {
    if manifest.service_preparation_hook {
        anyhow::ensure!(
            manifest.component.is_some()
                && manifest
                    .api
                    .as_ref()
                    .and_then(|api| api.required.get("dependencies"))
                    .is_some_and(|range| range.matches(&Version::new(1, 4, 0))
                        && !range.matches(&Version::new(1, 3, 0))),
            crate::validation::Diagnostic::floor("dependencies", "1.4")
        );
    }
    // Composable contracts are new semantics, not an automatic expansion of old service grants.
    if manifest
        .plugin_services
        .provides
        .values()
        .any(|contract| contract.collection)
        || manifest
            .plugin_services
            .requires
            .values()
            .any(|dependency| dependency.collection)
    {
        anyhow::ensure!(
            manifest
                .api
                .as_ref()
                .and_then(|api| api.required.get("plugin.services"))
                .is_some_and(|range| range.matches(&Version::new(1, 2, 0))
                    && !range.matches(&Version::new(1, 1, 0))
                    && !range.matches(&Version::new(1, 0, 0))),
            crate::validation::Diagnostic::floor("plugin.services", "1.2")
        );
    }
    if manifest.protocol != 7 {
        return Err(anyhow::Error::from(crate::validation::Diagnostic::new(
            crate::validation::Code::Protocol,
            Some("/protocol"),
        ))
        .context(format!(
            "Incompatible plugin protocol {}; update the plugin with the current SDK (protocol 7 required).",
            manifest.protocol
        )));
    }
    // Registry restoration uses the same release floor as ZIP/development admission. Neither
    // capability negotiation nor an already saved grant makes a future host contract available.
    if let Some(minimum) = &manifest.minimum_host {
        anyhow::ensure!(
            minimum.len() <= 128,
            crate::validation::Diagnostic::new(
                crate::validation::Code::Manifest,
                Some("/minimum_host")
            )
        );
        let minimum = Version::parse(minimum).map_err(|_| {
            crate::validation::Diagnostic::new(
                crate::validation::Code::HostVersion,
                Some("/minimum_host"),
            )
        })?;
        let host = Version::parse(env!("CARGO_PKG_VERSION"))?;
        let mut host_error = crate::validation::Diagnostic::new(
            crate::validation::Code::HostVersion,
            Some("/minimum_host"),
        );
        host_error.required = Some(minimum.to_string());
        host_error.available = Some(host.to_string());
        anyhow::ensure!(!host.cmp_precedence(&minimum).is_lt(), host_error);
    }
    anyhow::ensure!(manifest.activation.valid(), "Invalid background purpose");
    if manifest.panels.iter().any(|panel| panel.exclusive) {
        anyhow::ensure!(
            manifest
                .api
                .as_ref()
                .and_then(|requirements| requirements.required.get("ui.panels"))
                .is_some_and(|range| range.matches(&Version::new(1, 2, 0))
                    && !range.matches(&Version::new(1, 1, 0))
                    && !range.matches(&Version::new(1, 0, 0))),
            crate::validation::Diagnostic::floor("ui.panels", "1.2")
        );
        anyhow::ensure!(
            manifest.panels.iter().all(|panel| !panel.exclusive
                || matches!(panel.position.as_str(), "left" | "right" | "bottom")),
            "Exclusive panels require a side or bottom dock position"
        );
    }
    // Hiding an entry must be explicit on hosts that understand it; accepting ^1 alone would
    // let an older toolbar expose a contextual window the author deliberately omitted.
    if manifest
        .panels
        .iter()
        .any(|panel| !panel.show_in_window_controls)
    {
        anyhow::ensure!(
            manifest
                .api
                .as_ref()
                .and_then(|requirements| requirements.required.get("ui.panels"))
                .is_some_and(|range| range.matches(&Version::new(1, 2, 0))
                    && !range.matches(&Version::new(1, 0, 0))),
            crate::validation::Diagnostic::floor("ui.panels", "1.1")
        );
    }
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
            crate::validation::Diagnostic::floor("lifecycle.activation", "1.0")
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
            crate::validation::Diagnostic::floor("configuration", "1.1")
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
            crate::validation::Diagnostic::new(
                crate::validation::Code::Permission,
                Some("/permissions")
            )
        );
    }
    negotiate(manifest).map(|_| ()).map_err(|error| {
        // Native callers localize their operation label; technical guidance keeps its typed
        // cause for static checks, which formatting alone would otherwise erase.
        let message = format!("Incompatible plugin API; update the plugin: {error:#}");
        error.context(message)
    })
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
        ("ui.native".into(), Version::new(1, 2, 0)),
        ("ui.windows".into(), Version::new(1, 0, 0)),
        ("ui.visibility".into(), Version::new(1, 0, 0)),
        ("ui.code_editor".into(), Version::new(2, 0, 0)),
        ("editor.actions".into(), Version::new(1, 0, 0)),
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
        ("ui.canvas".into(), Version::new(1, 7, 0)),
        ("ui.collections".into(), Version::new(1, 0, 0)),
        ("ui.tree".into(), Version::new(1, 8, 0)),
        ("ui.workspace".into(), Version::new(1, 0, 0)),
        ("explorer.decorations".into(), Version::new(1, 0, 0)),
        ("ui.grid".into(), Version::new(1, 0, 0)),
        ("plugin.services".into(), Version::new(1, 2, 0)),
        ("workspace.files".into(), Version::new(1, 4, 0)),
        ("host.sdk".into(), Version::new(1, 0, 0)),
        ("storage.private".into(), Version::new(1, 2, 0)),
        ("storage.editor".into(), Version::new(1, 0, 0)),
        ("storage.migration".into(), Version::new(1, 0, 0)),
        ("editor.documents".into(), Version::new(1, 2, 0)),
        ("editor.virtual".into(), Version::new(1, 1, 0)),
        ("editor.diff".into(), Version::new(1, 4, 0)),
        ("editor.decorations".into(), Version::new(1, 0, 0)),
        ("editor.layout".into(), Version::new(2, 0, 0)),
        ("editor.viewport".into(), Version::new(1, 0, 0)),
        ("editor.edit".into(), Version::new(1, 1, 0)),
        ("editor.diagnostics".into(), Version::new(1, 0, 0)),
        ("editor.navigation".into(), Version::new(1, 0, 0)),
        ("editor.toolbar".into(), Version::new(1, 1, 0)),
        ("editor.images".into(), Version::new(1, 0, 0)),
        // 1.1 adds explicitly declared hot changes and namespace-only secret references.
        ("configuration".into(), Version::new(1, 1, 0)),
        // 1.1 lets packages hide contextual panel entries without changing window visibility.
        ("ui.panels".into(), Version::new(1, 2, 0)),
        ("ui.clipboard".into(), Version::new(1, 0, 0)),
        // 1.7 presents an already-owned PTY; negotiation still grants no execution or UI authority.
        ("process".into(), Version::new(1, 9, 0)),
        // Readonly tool-distribution metadata remains separate from native execution and process IO.
        ("tools.inspect".into(), Version::new(1, 0, 0)),
        // 1.2 owns native roots; 1.3 opts into immutable diagnostic URIs; 1.4 adds shared native
        // references/code actions/signature consumers, gated again by standard server advertisement.
        // 1.6 registers sealed project choices and admits only canonical direct interactive prepares.
        // 1.7 resolves explicitly requested dependency paths in initialization/configuration data.
        ("language.lsp".into(), LANGUAGE_LSP_VERSION),
        ("language.control".into(), Version::new(1, 0, 0)),
        ("language.content".into(), Version::new(1, 0, 0)),
        ("language.modules".into(), Version::new(1, 0, 0)),
        // Pure references/actions/signature help remain explicitly experimental until wider use.
        ("language.direct".into(), Version::new(0, 1, 0)),
        ("language.completion".into(), Version::new(1, 0, 0)),
        // Independent providers negotiate editing and structure without replacing primary LSP ownership.
        ("language.formatting".into(), Version::new(1, 0, 0)),
        // 1.1 permits explicitly negotiated semantic linked ranges with different initial names.
        ("language.editing".into(), LANGUAGE_EDITING_VERSION),
        ("language.structure".into(), Version::new(1, 0, 0)),
        // 1.2 prepares declared native services asynchronously at method invocation.
        // 1.3 supports bounded 256 MiB archives and 512 MiB extracted distributions.
        ("dependencies".into(), Version::new(1, 4, 0)),
    ]
    .into();
    Ok(requirements.negotiate_detailed(Version::new(1, 0, 0), &available)?)
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
                Version::new(1, 9, 0)
            );
        }
    }
}

/// Optional visibility events are absent from the negotiated API of every older guest.
#[cfg(test)]
mod visibility_tests {
    use super::*;

    /// Contextual window entries use an explicit minor floor; older declarations keep their entries.
    #[test]
    fn hidden_window_controls_require_explicit_panel_capability_floor() {
        for id in ["first.contextual", "second.contextual"] {
            let mut manifest: Manifest = serde_json::from_value(serde_json::json!({
                "id":id,"name":id,"version":"1.0.0","protocol":7,
                "permissions":["ui.panels"],"storage_limit":1024,
                "api":{"base":"^1","required":{"ui.panels":"^1"}},
                "panels":[{"id":"details","title":"Details","position":"window"}]
            }))
            .unwrap();
            assert!(manifest.panels[0].show_in_window_controls);
            assert!(
                serde_json::to_value(&manifest).unwrap()["panels"][0]
                    .get("show_in_window_controls")
                    .is_none()
            );
            require_current(&manifest).unwrap();
            manifest.panels[0].show_in_window_controls = false;
            assert!(require_current(&manifest).is_err());
            manifest
                .api
                .as_mut()
                .unwrap()
                .required
                .insert("ui.panels".into(), ">=1.1,<2".parse().unwrap());
            require_current(&manifest).unwrap();
            assert_eq!(
                negotiate(&manifest).unwrap().capabilities["ui.panels"],
                Version::new(1, 2, 0)
            );
        }
    }

    #[test]
    fn exclusive_dock_panels_require_explicit_version_and_valid_region() {
        let mut manifest: Manifest = serde_json::from_value(serde_json::json!({
            "id":"independent.exclusive","name":"Exclusive","version":"1.0.0","protocol":7,
            "permissions":["ui.panels"],"storage_limit":1024,
            "api":{"base":"^1","required":{"ui.panels":"^1"}},
            "panels":[{"id":"review","title":"Review","position":"left"}]
        }))
        .unwrap();
        assert!(!manifest.panels[0].exclusive);
        assert!(
            serde_json::to_value(&manifest).unwrap()["panels"][0]
                .get("exclusive")
                .is_none()
        );
        require_current(&manifest).unwrap();
        manifest.panels[0].exclusive = true;
        assert!(require_current(&manifest).is_err());
        for range in ["^1", ">=1.1,<2", "^1.1", "^2"] {
            manifest
                .api
                .as_mut()
                .unwrap()
                .required
                .insert("ui.panels".into(), range.parse().unwrap());
            assert!(
                require_current(&manifest).is_err(),
                "{range} must not silently accept exclusivity"
            );
        }
        manifest
            .api
            .as_mut()
            .unwrap()
            .required
            .insert("ui.panels".into(), ">=1.2,<2".parse().unwrap());
        manifest.panels[0].show_in_window_controls = false;
        for position in ["left", "right", "bottom"] {
            manifest.panels[0].position = position.into();
            require_current(&manifest).unwrap();
            assert_eq!(
                negotiate(&manifest).unwrap().capabilities["ui.panels"],
                Version::new(1, 2, 0)
            );
        }
        for position in ["editor", "window"] {
            manifest.panels[0].position = position.into();
            assert!(require_current(&manifest).is_err());
        }
    }

    #[test]
    fn panel_visibility_is_opt_in_and_independent_of_plugin_identity() {
        for opt_in in [false, true] {
            let mut manifest: Manifest = serde_json::from_value(serde_json::json!({
                "id":"independent.visibility","name":"Visibility","version":"1.0.0","protocol":7,
                "permissions":[],"storage_limit":1024,"api":{"base":"^1","required":{}}
            }))
            .unwrap();
            if opt_in {
                manifest
                    .api
                    .as_mut()
                    .unwrap()
                    .optional
                    .insert("ui.visibility".into(), "^1".parse().unwrap());
            }
            let api = negotiate(&manifest).unwrap();
            assert_eq!(api.capabilities.contains_key("ui.visibility"), opt_in);
            assert!(
                manifest.permissions.is_empty(),
                "negotiation does not grant ui.panels consent"
            );
        }
    }
}
