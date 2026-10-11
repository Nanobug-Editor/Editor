//! Native schema controls submit actual new-SDK HotApply values and own secret key references.
use super::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};

/// Refresh effective values and the original native correlation receipt without inventing settings.
fn publish_result(
    owner: &Entity<ExtensionPanel>,
    manager: &mut plugin_runtime::Manager,
    result: Option<(u64, Result<(), String>)>,
    visual: &mut VisualTestContext,
) {
    visual.update(|_, cx| {
        owner.update(cx, |owner, cx| {
            let mut state = owner.worker.state.lock().unwrap();
            state.publish_manager(manager);
            state.configurations.insert(
                "cloud-translation".into(),
                manager
                    .effective_settings("cloud-translation")
                    .map_err(|error| error.to_string()),
            );
            if let Some(result) = result {
                state.configuration_result = Some(result);
            }
            state.configuration_revision += 1;
            drop(state);
            owner.poll(cx);
        })
    });
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
}

/// Apply travels through the real Base input/button and the worker's unchanged public payload.
fn apply(
    visual: &mut VisualTestContext,
    owner: &Entity<ExtensionPanel>,
    manager: &mut plugin_runtime::Manager,
    key: &str,
    value: &str,
) -> Result<(), String> {
    // GPUI's bounds lookup accepts static control IDs. Keep these three actual form targets
    // explicit rather than leaking a dynamically allocated string into the test process.
    let (input_id, apply_id) = match key {
        "timeout" => (
            "setting-cloud-translation-timeout-value",
            "setting-cloud-translation-timeout-apply",
        ),
        "poll_interval" => (
            "setting-cloud-translation-poll_interval-value",
            "setting-cloud-translation-poll_interval-apply",
        ),
        "keyRef" => (
            "setting-cloud-translation-keyRef-value",
            "setting-cloud-translation-keyRef-apply",
        ),
        _ => panic!("unexpected native settings test field: {key}"),
    };
    let input = visual.debug_bounds(input_id).unwrap();
    visual.simulate_click(input.center(), Default::default());
    // simulate_input only types characters: an empty string does not delete the selection.
    // Native Backspace clears even a rejected draft before typing or submitting an empty key.
    visual.simulate_keystrokes("ctrl-a backspace");
    if !value.is_empty() {
        visual.simulate_input(value);
    }
    let expected_value = match key {
        "keyRef" => serde_json::json!(value),
        _ => serde_json::json!(value.parse::<i64>().unwrap()),
    };
    let button = visual.debug_bounds(apply_id).unwrap();
    visual.simulate_click(button.center(), Default::default());
    let work = visual.update(|_, cx| {
        owner
            .read(cx)
            .worker
            .recorded
            .lock()
            .unwrap()
            .try_iter()
            .find(|work| matches!(work, Work::SetSetting { .. }))
    });
    let Some(Work::SetSetting {
        request,
        plugin,
        scope,
        key,
        value,
    }) = work
    else {
        visual.run_until_parked();
        return Err("Native declaration validation rejected the draft".into());
    };
    assert_eq!(plugin, "cloud-translation");
    assert_eq!(scope, protocol::settings::Scope::User);
    // SetSetting is produced from the real InputState draft, so assert the consumed input
    // before asking the manager to validate or persist it, including the empty reference.
    assert_eq!(
        value,
        Some(expected_value),
        "the native form must submit the text that was entered"
    );
    let result = manager
        .update_setting(&plugin, scope, &key, value)
        .map_err(|error| error.to_string());
    publish_result(owner, manager, Some((request, result.clone())), visual);
    result
}

/// Settings browsing stays cold; then actual view use lets the same form hot-apply without
/// replacing its owner. UTF-8 secret references remain keys, including the empty unconfigured key.
#[gpui::test]
#[ignore = "build cloud-translation 0.1.2 through host exported SDK50 before running"]
fn native_hot_settings_apply_without_owner_restart_and_keep_only_secret_keys(
    cx: &mut TestAppContext,
) {
    // Locale is process-wide; restore it even when a native layout assertion fails.
    struct RestoreLocale(String);
    impl Drop for RestoreLocale {
        fn drop(&mut self) {
            rust_i18n::set_locale(&self.0);
        }
    }
    let _locale = RestoreLocale(rust_i18n::locale().to_string());
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let directory = tempfile::tempdir().unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let package = Package::read(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/cloud-api-test/cloud-translation-0.1.2.zip"),
    )
    .unwrap();
    assert!(matches!(
        &package.manifest.settings["keyRef"].value_type,
        protocol::settings::SettingType::SecretReference
    ));
    let mut manager = plugin_runtime::Manager::open(
        directory.path().join("runtime"),
        protocol::Environment {
            workspace: workspace.root().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, editor) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    let owner = editor.update(|_, cx| app.read(cx).extensions.clone());
    publish_result(&owner, &mut manager, None, editor);
    let editor_window = editor.update(|window, _| window.window_handle());
    let trigger = editor.debug_bounds("settings-trigger").unwrap();
    editor.simulate_click(trigger.center(), Default::default());
    let dialog = editor
        .update(|_, cx| cx.windows())
        .into_iter()
        .find(|handle| *handle != editor_window)
        .unwrap();
    let form = VisualTestContext::from_window(dialog, editor).into_mut();
    form.run_until_parked();
    let nav = form.debug_bounds("settings-nav-plugins").unwrap();
    form.simulate_click(nav.center(), Default::default());
    form.simulate_resize(size(px(1000.), px(850.)));
    form.run_until_parked();
    form.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        manager.live.is_empty(),
        "opening the real Settings dialog is not demand"
    );
    package_ui_test_support::demand(
        &mut manager,
        "cloud-translation",
        plugin_runtime::DemandReason::View("cloud".into()),
    )
    .unwrap();
    let original_owner = manager.instance_id("cloud-translation").unwrap().to_owned();
    let original_resources = manager.resource_count();
    publish_result(&owner, &mut manager, None, form);
    apply(form, &owner, &mut manager, "timeout", "100").unwrap();
    assert_eq!(
        manager.instance_id("cloud-translation"),
        Some(original_owner.as_str())
    );
    assert!(
        apply(form, &owner, &mut manager, "poll_interval", "1000").is_err(),
        "actual guest Validate must reject a poll interval exceeding its deadline"
    );
    assert!(form.debug_bounds("plugin-settings-error").is_some());
    assert_eq!(
        manager.effective_settings("cloud-translation").unwrap()["poll_interval"].value,
        serde_json::json!(25)
    );
    // One existing form covers both palettes and languages without repeating WASM preparation.
    for (dark, locale) in [(false, "en"), (true, "zh-CN")] {
        rust_i18n::set_locale(locale);
        form.update(|window, cx| {
            apply_theme(builtin_theme(dark), cx);
            owner.update(cx, |_, cx| cx.notify());
            window.draw(cx).clear(cx);
        });
        form.run_until_parked();
        assert!(
            form.debug_bounds("setting-cloud-translation-keyRef-value")
                .is_some()
        );
        assert!(
            form.debug_bounds("setting-cloud-translation-keyRef-apply")
                .is_some()
        );
    }
    apply(form, &owner, &mut manager, "keyRef", "凭据.key").unwrap();
    assert_eq!(
        manager.effective_settings("cloud-translation").unwrap()["keyRef"].value,
        serde_json::json!("凭据.key")
    );
    assert!(
        apply(form, &owner, &mut manager, "keyRef", &"界".repeat(43)).is_err(),
        "secret references are limited by UTF-8 bytes, not displayed characters"
    );
    assert!(form.debug_bounds("plugin-settings-error").is_some());
    apply(form, &owner, &mut manager, "keyRef", "").unwrap();
    assert_eq!(
        manager.effective_settings("cloud-translation").unwrap()["keyRef"].value,
        serde_json::json!("")
    );
    assert_eq!(
        manager.instance_id("cloud-translation"),
        Some(original_owner.as_str()),
        "HotApply retains the real guest and its existing resource owner"
    );
    assert_eq!(
        manager.resource_count(),
        original_resources,
        "key entry must not start credential, HTTP or timer work"
    );
    manager.shutdown();
}
