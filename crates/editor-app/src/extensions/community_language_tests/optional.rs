//! An actually omitted optional direct contract cannot shadow the existing selected standard LSP.
use super::language_native::{place_caret, wait_for_control};
use super::*;
use plugin_runtime::plugin_protocol::{direct, settings::Scope};
use serde_json::{Value, json};

/// Change declarations on the real packaged guest/server and reinspect through public Package;
/// an unsupported optional minor grants no pure executor or static native selection role.
#[gpui::test]
#[ignore = "opt-in frozen Package/WASM and native tools; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn native_community_language_omitted_optional_direct_keeps_selected_lsp_features(
    cx: &mut TestAppContext,
) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, original| {
        let root = original.parent().unwrap();
        let mut manager = packages(visual, &app, root, &["keyvalue-language"]);
        crate::language::providers::choose(
            Scope::User,
            "lsp:keyvalue",
            Some("keyvalue-language/analysis"),
        )
        .unwrap();
        // Change only negotiation declarations on the original digest-checked language input.
        let mut files = crate::test_packages::package("keyvalue-language-0.1.0").files;
        let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
        manifest["api"]["optional"] = json!({"language.direct":">=0.2,<0.3"});
        manifest["direct_language_providers"] = json!([{
            "id":"future-direct", "language":"keyvalue",
            "features":["references","code_actions","signature_help"]
        }]);
        files.insert(
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        );
        // A required future experimental minor is actually unavailable on this host, rather
        // than a compatible `^1` declaration masquerading as old negotiation. Admission fails
        // before replacing the already selected standard service or publishing any direct role.
        let mut future_files = files.clone();
        let mut future = manifest.clone();
        future["api"]["optional"] = json!({});
        future["api"]["required"]["language.direct"] = json!(">=0.2,<0.3");
        future_files.insert("manifest.json".into(), serde_json::to_vec(&future).unwrap());
        assert!(
            Package::from_files(future_files)
                .err()
                .expect("Required future direct API must be rejected")
                .to_string()
                .contains("language.direct")
        );
        assert!(manager.direct_language_providers().is_empty());

        // The direct declaration requires editor.read at package admission. A genuine current
        // package also cannot install with an incomplete grant set; neither path emits a lease.
        let expression = crate::test_packages::package("expression-language-0.1.0");
        let mut missing_read_files = expression.files.clone();
        let mut missing_read: Value =
            serde_json::from_slice(&missing_read_files["manifest.json"]).unwrap();
        missing_read["permissions"]
            .as_array_mut()
            .unwrap()
            .retain(|permission| permission != "editor.read");
        missing_read_files.insert(
            "manifest.json".into(),
            serde_json::to_vec(&missing_read).unwrap(),
        );
        assert!(
            Package::from_files(missing_read_files)
                .err()
                .expect("Direct declaration needs read authority")
                .to_string()
                .contains("editor.read")
        );
        let mut grants = expression.manifest.permissions.clone();
        grants.remove("editor.read");
        assert!(
            manager
                .install(&expression, grants)
                .unwrap_err()
                .to_string()
                .contains("Permission confirmation required")
        );
        assert!(!manager.installed.contains_key("expression-language"));
        assert!(manager.direct_language_providers().is_empty());

        let package = Package::from_files(files).unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        assert!(
            manager.installed["keyvalue-language"]
                .compatibility_error()
                .is_none()
        );
        assert!(!manager.installed["keyvalue-language"].supports_capability("language.direct"));
        assert!(manager.direct_language_providers().is_empty());
        publish_providers(visual, &app, &mut manager);
        assert_eq!(
            crate::language::providers::language_servers()["keyvalue"].as_deref(),
            Some("keyvalue-language/analysis"),
            "An optional extension cannot replace the valid explicit primary choice"
        );
        let path = root.join("optional.nbkv");
        std::fs::write(&path, "disk-only value").unwrap();
        let path = path.canonicalize().unwrap();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| app.open_file(path.clone(), window, cx));
        });
        draw(visual);
        let source = "port=oops\n中文=1\nother=中文\nsetting(中文, ";
        visual.simulate_keystrokes("ctrl-a");
        visual.simulate_input(source);
        draw(visual);
        pump(visual, &app, &mut manager);
        let server = visual.update(|_, cx| app.read(cx).language_servers["keyvalue"].clone());
        server.prepare_until_ready().unwrap();
        assert!(server.supports_language_features());

        // A real named reference row is the fallback signal. Metadata alone cannot prove that
        // the missing pure role left the shared native request and selection path operational.
        place_caret(visual, &app, 10);
        visual.simulate_keystrokes("shift-f12");
        wait_for_control(visual, &app, &mut manager, "language-feature-row-1");
        click(visual, "language-feature-row-1");
        assert_eq!(
            visual.update(|_, cx| app.read(cx).editor.read(cx).cursor()),
            source.match_indices("中文").nth(1).unwrap().0
        );
        assert_eq!(text(visual, &app), source);

        // The other two feature roles must also fall back to the same selected controlled
        // server, without creating a pure executor or silently treating a ghost as ambiguity.
        place_caret(visual, &app, 0);
        visual.simulate_keystrokes("ctrl-.");
        wait_for_control(visual, &app, &mut manager, "language-feature-row-0");
        assert!(visual.update(|window, _| {
            gpui_base::test_support::snapshots(window)
                .iter()
                .any(|node| {
                    node.role() == Some(gpui_kit::Role::Button)
                        && node.visible()
                        && node
                            .label()
                            .is_some_and(|label| label.contains("Use default port"))
                })
        }));
        visual.simulate_keystrokes("escape");
        draw(visual);
        place_caret(visual, &app, source.len());
        visual.simulate_keystrokes("ctrl-shift-space");
        wait_for_control(visual, &app, &mut manager, "language-signature-card-0");
        assert!(visual.update(|window, _| {
            let label = rust_i18n::t!("language_features.parameter", parameter = "value");
            gpui_base::test_support::snapshots(window)
                .iter()
                .any(|node| node.visible() && node.label() == Some(label.as_ref()))
        }));
        for feature in [
            direct::Feature::References,
            direct::Feature::CodeActions,
            direct::Feature::SignatureHelp,
        ] {
            assert!(!crate::language::providers::has_language_feature(
                "keyvalue", feature
            ));
            assert!(
                crate::language::providers::language_feature_provider("keyvalue", feature)
                    .is_none()
            );
        }
        assert_eq!(text(visual, &app), source);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "disk-only value");
        assert_eq!(
            manager.language_services()["keyvalue-language/analysis"]
                .as_ref()
                .unwrap()
                .process_count(),
            1
        );
        manager.shutdown();
    });
}
