//! A formal resource-only language package consumes native features and transactions without a fake guest.
use super::language_native::{place_caret, wait_for_control};
use super::*;
use plugin_runtime::plugin_protocol::{diagnostics::Fix, direct, language::SourceSnapshot};
use plugin_runtime::{LanguageFixOrigin, RequestCancellation};
use serde_json::{Value, json};

/// Reinspect the actual independent archive after replacing its optional hook with declarative data.
/// The real packaged executable, dependency digest, service permission and language resources remain.
fn declarative_files() -> BTreeMap<String, Vec<u8>> {
    // Keep the original grammar, executable and permissions while removing only its optional hook.
    let package = crate::test_packages::package("keyvalue-language-0.1.0");
    let mut files = package.files;
    let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    let component = manifest
        .as_object_mut()
        .unwrap()
        .remove("component")
        .expect("The formal package has an optional lifecycle component");
    files.remove(component.as_str().unwrap());
    manifest["language_servers"][0]["hook"] = json!(false);
    manifest["language_servers"][0]["initialization_options"] =
        json!({"keyValue":{"defaultPort":8080}});
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    files
}

/// A user keystroke selects a painted proposal and reaches the Manager's ordinary native request queue.
fn action_preview(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
) -> EditorRequest {
    place_caret(visual, app, 0);
    visual.simulate_keystrokes("ctrl-.");
    wait_for_control(visual, app, manager, "language-feature-row-0");
    assert!(visual.update(|window, _| {
        gpui_base::test_support::snapshots(window)
            .iter()
            .any(|node| {
                node.role() == Some(gpui_kit::Role::Button)
                    && node.visible()
                    && node
                        .label()
                        .is_some_and(|name| name.contains("Use default port"))
            })
    }));
    click(visual, "language-feature-row-0");
    let request = pump(visual, app, manager)
        .into_iter()
        .find(|request| {
            matches!(
                request.operation(),
                api::EditorOperation::Workspace {
                    operation: workspace::Operation::ApplyPlan { .. }
                }
            )
        })
        .expect("A resource-only code action must reach the public native plan dispatcher");
    assert!(visual.debug_bounds("workspace-plan-confirm").is_some());
    assert!(
        manager.live.is_empty(),
        "Preview cannot allocate an empty WASM owner"
    );
    request
}

/// Obtain a real proposal for the current unsaved source; admission tests retain its service seal.
fn current_action(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
) -> (LanguageFixOrigin, api::DocumentVersion, Fix) {
    let (server, source, lease, workspace) = visual.update(|_, cx| {
        let app = app.read(cx);
        let index = app.active_text_tab_index().unwrap();
        let server = app.language_servers["keyvalue"].clone();
        let uri = crate::language::navigation::file_uri(app.tabs[index].path()).unwrap();
        let source = SourceSnapshot {
            document: app.plugin_document_version(index).unwrap(),
            text: app.editor.read(cx).text().to_string(),
        };
        let lease = server
            .document(&uri)
            .expect("The actual native source has a shared lease");
        (server, source, lease, app.workspace.root().to_path_buf())
    });
    let result = server
        .feature_for(
            lease,
            source.clone(),
            0,
            direct::Feature::CodeActions,
            &RequestCancellation::default(),
            &workspace,
        )
        .unwrap();
    let direct::Result::CodeActions { mut actions } = result else {
        panic!("Expected the real stdio service's current-source proposal")
    };
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].plan.origin.as_ref(), Some(&source.document));
    (server.fix_origin(), source.document, actions.remove(0))
}

/// Current-package declarations exercise the complete native path and original permission/capability seal.
#[gpui::test]
#[ignore = "opt-in frozen Package/WASM and native tools; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn native_community_language_declarative_features_preview_undo_and_original_seal(
    cx: &mut TestAppContext,
) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, original| {
        let root = original.parent().unwrap();
        let files = declarative_files();
        let package = Package::from_files(files.clone()).unwrap();
        assert!(package.component().is_none());
        assert!(
            package.files.keys().all(|path| !path.ends_with(".wasm")),
            "The actual resource-only variant contains no dummy WASM bytes"
        );
        assert!(!package.manifest.language_servers[0].hook);
        let mut manager = Manager::open(
            runtime_root(root),
            protocol::Environment {
                workspace: root.display().to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        assert!(
            manager.live.is_empty(),
            "A declarative package never creates a fake main guest"
        );
        publish_providers(visual, &app, &mut manager);

        let path = root.join("declarative.nbkv");
        std::fs::write(&path, "disk-only value").unwrap();
        let path = path.canonicalize().unwrap();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| app.open_file(path.clone(), window, cx));
        });
        draw(visual);
        let source = "port=oops\nemoji=🙂 中文\nother=中文\nsetting(中文, ";
        visual.simulate_keystrokes("ctrl-a");
        visual.simulate_input(source);
        draw(visual);
        assert_eq!(text(visual, &app), source);
        assert!(visual.update(|_, cx| {
            let app = app.read(cx);
            app.tabs[app.active_text_tab_index().unwrap()].is_dirty()
        }));
        pump(visual, &app, &mut manager);
        let service = manager.language_services()["keyvalue-language/analysis"]
            .as_ref()
            .unwrap()
            .clone();
        let server = visual.update(|_, cx| app.read(cx).language_servers["keyvalue"].clone());
        server.prepare_until_ready().unwrap();
        assert!(server.supports_language_features());
        assert_eq!(service.process_count(), 1);

        // Both decisions use the real Ctrl+. input route. Cancellation cannot write a prefix,
        // and acceptance changes only the dirty native buffer until the one group Undo restores it.
        let cancelled = action_preview(visual, &app, &mut manager);
        assert_eq!(text(visual, &app), source);
        click(visual, "workspace-plan-cancel");
        pump(visual, &app, &mut manager);
        let cancelled_receipt = receipt(&cancelled);
        assert_eq!(cancelled_receipt.state, workspace::BatchState::Cancelled);
        assert!(cancelled_receipt.completed.is_empty());
        assert!(cancelled_receipt.affected.is_empty());
        assert_eq!(text(visual, &app), source);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "disk-only value");

        let applied = action_preview(visual, &app, &mut manager);
        assert_eq!(text(visual, &app), source);
        click(visual, "workspace-plan-confirm");
        pump(visual, &app, &mut manager);
        assert_eq!(receipt(&applied).state, workspace::BatchState::Complete);
        assert_eq!(
            text(visual, &app),
            source.replacen("port=oops", "port=8080", 1)
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "disk-only value");
        click(visual, "workspace-batch-undo");
        pump(visual, &app, &mut manager);
        assert_eq!(text(visual, &app), source);
        assert!(
            manager.live.is_empty(),
            "Apply and native group Undo need no WASM shim"
        );

        // References resolve UTF-16 columns after a surrogate pair against dirty native text;
        // parameter help consumes the same live connection and inert native signature card.
        place_caret(visual, &app, source.find("中文").unwrap());
        visual.simulate_keystrokes("shift-f12");
        wait_for_control(visual, &app, &mut manager, "language-feature-row-1");
        click(visual, "language-feature-row-1");
        assert_eq!(
            visual.update(|_, cx| app.read(cx).editor.read(cx).cursor()),
            source.match_indices("中文").nth(1).unwrap().0
        );
        assert_eq!(text(visual, &app), source);
        place_caret(visual, &app, source.len());
        visual.simulate_keystrokes("ctrl-shift-space");
        wait_for_control(visual, &app, &mut manager, "language-signature-card-0");
        visual.update(|window, _| {
            let parameter = rust_i18n::t!("language_features.parameter", parameter = "value");
            assert!(
                gpui_base::test_support::snapshots(window)
                    .iter()
                    .any(|node| { node.visible() && node.label() == Some(parameter.as_ref()) })
            );
        });
        visual.simulate_keystrokes("escape");
        draw(visual);
        assert_eq!(
            service.process_count(),
            1,
            "Native features share one standard service connection"
        );

        let (old_origin, document, old_fix) = current_action(visual, &app);
        manager.disable("keyvalue-language").unwrap();
        publish_providers(visual, &app, &mut manager);
        assert!(!old_origin.is_live());
        assert!(!old_origin.authority().is_live());
        assert_eq!(service.process_count(), 0);
        assert_eq!(
            manager
                .submit_language_fix(&old_origin, document.clone(), old_fix.clone())
                .unwrap_err()
                .code,
            api::ErrorCode::InvalidHandle
        );
        manager.enable("keyvalue-language").unwrap();
        publish_providers(visual, &app, &mut manager);
        pump(visual, &app, &mut manager);
        let replacement = manager.language_services()["keyvalue-language/analysis"]
            .as_ref()
            .unwrap()
            .clone();
        assert!(!Arc::ptr_eq(&service, &replacement));
        assert!(replacement.authority().is_live());
        assert_eq!(
            manager
                .submit_language_fix(&old_origin, document, old_fix)
                .unwrap_err()
                .code,
            api::ErrorCode::InvalidHandle,
            "Re-enabling the same declaration cannot revive its earlier native effect seal"
        );
        assert!(manager.live.is_empty());

        // Removing the package's write grant still permits readonly language results. The
        // original native seal cannot borrow editor.write from this application's trusted state.
        let mut restricted_files = files.clone();
        let mut manifest: Value =
            serde_json::from_slice(&restricted_files["manifest.json"]).unwrap();
        manifest["permissions"]
            .as_array_mut()
            .unwrap()
            .retain(|permission| permission != "editor.write");
        restricted_files.insert(
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        );
        let readonly = Package::from_files(restricted_files).unwrap();
        manager
            .install(&readonly, readonly.manifest.permissions.clone())
            .unwrap();
        publish_providers(visual, &app, &mut manager);
        pump(visual, &app, &mut manager);
        let (readonly_origin, document, fix) = current_action(visual, &app);
        assert_eq!(
            manager
                .submit_language_fix(&readonly_origin, document, fix)
                .unwrap_err()
                .code,
            api::ErrorCode::PermissionDenied
        );

        // This host truthfully omits an incompatible optional old file-plan contract; declaring
        // ^1.2 would negotiate 1.3 and cannot simulate a lower grant. Admission reports the
        // missing negotiated capability as CapabilityUnavailable; no version is forged here.
        let mut old_contract_files = files;
        let mut manifest: Value =
            serde_json::from_slice(&old_contract_files["manifest.json"]).unwrap();
        manifest["api"]["required"]
            .as_object_mut()
            .unwrap()
            .remove("workspace.files");
        manifest["api"]["optional"] = json!({"workspace.files":">=1,<1.3"});
        old_contract_files.insert(
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        );
        let old_contract = Package::from_files(old_contract_files).unwrap();
        manager
            .install(&old_contract, old_contract.manifest.permissions.clone())
            .unwrap();
        publish_providers(visual, &app, &mut manager);
        pump(visual, &app, &mut manager);
        let limited = manager.language_services()["keyvalue-language/analysis"]
            .as_ref()
            .unwrap()
            .clone();
        assert!(limited.capability("workspace.files").is_none());
        let (limited_origin, document, fix) = current_action(visual, &app);
        assert_eq!(
            manager
                .submit_language_fix(&limited_origin, document, fix)
                .unwrap_err()
                .code,
            api::ErrorCode::CapabilityUnavailable
        );
        assert_eq!(text(visual, &app), source);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "disk-only value");
        assert!(manager.live.is_empty());
        manager.shutdown();
        assert!(!limited_origin.is_live());
        assert_eq!(limited.process_count(), 0);
    });
}
