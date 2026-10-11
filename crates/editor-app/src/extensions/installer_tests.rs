//! Real GPUI consent interactions drive the same install token consumed by the native manager.
use super::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};

/// Build a data-only package with a native fixture: no application-private installer test entry exists.
fn package(marker: &std::path::Path) -> Package {
    let bytes = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/debug/examples/installer_fixture.exe"),
    )
    .unwrap();
    let manifest = json!({
        "id":"installer-ui","name":"Installer UI","version":"1.0.0","protocol":7,
        "api":{"base":"^1","required":{"language.lsp":"^1","process":"^1","dependencies":"^1"}},
        "contributions":"plugin.toml","storage_limit":1024,
        "permissions":["dependencies.prepare","dependencies.install","process.service.analysis"],
        "language_servers":[{"id":"analysis","language":"arbitrary","service":"analysis"}],
        "services":{"analysis":{"program":"unused","installation":{
            "executable":"server/installed/tool.exe","artifacts":[{
                "id":"server","version":"1","platform":format!("{}-{}",std::env::consts::OS,std::env::consts::ARCH),
                "sha256":format!("{:x}",Sha256::digest(&bytes)),"source":{"kind":"package","path":"installer.exe"},
                "format":{"kind":"file","path":"installer.exe"},"installer":{
                    "program":"installer.exe","args":["${target}",marker,"success"],"target":"installed",
                    "purpose":format!("Prepare private test tool{}", "\nReviewed private SDK details".repeat(24)),"kind":"project_sdk"
                }
            }]
        }}}
    });
    super::language_tests::packages::repack(BTreeMap::from([
        ("manifest.json".into(),serde_json::to_vec(&manifest).unwrap()),
        ("installer.exe".into(),bytes),
        ("plugin.toml".into(),b"[plugin]\nid='installer-ui'\nname='Installer UI'\nversion='1.0.0'\nhost_version='^0.1'\n".to_vec()),
    ])).unwrap()
}

/// Disabled confirm, explicit SDK selection and final execution confirmation are separate real clicks.
#[gpui::test]
#[ignore = "build installer_fixture first"]
fn installer_dialog_requires_sdk_choice_and_concrete_execution_confirmation(
    cx: &mut TestAppContext,
) {
    let previous_locale = rust_i18n::locale().to_string();
    for (locale, dark, font_size) in [("zh-CN", false, 14.), ("en", true, 24.)] {
        rust_i18n::set_locale(locale);
        cx.update(|cx| {
            gpui_kit::init(cx);
            typography::init(cx);
            apply_theme(builtin_theme(dark), cx);
        });
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("executed.txt");
        let package = package(&marker);
        let workspace = Workspace::open(directory.path()).unwrap();
        let slot = Rc::new(RefCell::new(None));
        let captured = slot.clone();
        let previous_windows = cx.update(|cx| cx.windows());
        let (_, editor_cx) = cx.add_window_view(move |window, cx| {
            let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
            *captured.borrow_mut() = Some(app.clone());
            Root::new(app, window, cx)
        });
        let app = slot.borrow_mut().take().unwrap();
        let editor_window = editor_cx.update(|window, _| window.window_handle());
        let control = editor_cx.update(|_, cx| {
            app.read(cx).extensions.clone().update(cx, |panel, _| {
                assert!(panel.queue_lifecycle(Work::Install(package.clone())));
                panel
                    .worker
                    .state
                    .lock()
                    .unwrap()
                    .install_control
                    .clone()
                    .unwrap()
            })
        });
        let runtime = directory.path().join("plugins");
        let workspace = directory.path().display().to_string();
        let token = control.clone();
        editor_cx.update(|window, cx| {
            app.read(cx)
                .extensions
                .clone()
                .update(cx, |panel, cx| panel.open_installation_progress(window, cx))
        });
        editor_cx.run_until_parked();
        let dialog = editor_cx
            .update(|_, cx| cx.windows())
            .into_iter()
            .find(|handle| *handle != editor_window && !previous_windows.contains(handle))
            .unwrap();
        let dialog_cx = VisualTestContext::from_window(dialog, cx).into_mut();
        // Open at the real default size before native preparation publishes its consent prompt.
        // A test-side resize would hide a late prompt stranded in the former 200px progress window.
        assert!(control.installer_prompt().is_none());
        dialog_cx.update(|window, cx| {
            typography::set_font_size(cx, font_size);
            theme::sync_font_sizes(cx);
            window.activate_window();
            window.draw(cx).clear(cx);
        });
        let worker = std::thread::spawn(move || {
            let mut manager = plugin_runtime::Manager::open(
                runtime,
                protocol::Environment {
                    workspace,
                    ..Default::default()
                },
            )
            .unwrap();
            manager.install_with_control(&package, package.manifest.permissions.clone(), &token)
        });
        let first = wait_prompt(&control, None);
        // Validate real parameter interpolation, not untranslated key-shaped labels with a hitbox.
        let purpose = t!("plugin_installation.purpose", value = first.purpose.clone()).to_string();
        assert!(purpose.contains(&first.purpose));
        assert!(purpose.starts_with(if locale == "en" {
            "Purpose:"
        } else {
            "用途："
        }));
        dialog_cx.update(|window, cx| window.draw(cx).clear(cx));
        let confirm = dialog_cx
            .debug_bounds("plugin-install-native-confirm-region")
            .unwrap();
        dialog_cx.simulate_click(confirm.center(), Default::default());
        assert_eq!(control.installer_prompt().unwrap().id, first.id);
        assert!(!marker.exists());
        let checkbox = dialog_cx.debug_bounds("plugin-install-sdk-region").unwrap();
        let viewport = dialog_cx.update(|window, _| window.viewport_size());
        assert!(checkbox.top() >= px(0.) && checkbox.bottom() <= viewport.height);
        assert!(confirm.top() >= px(0.) && confirm.bottom() <= viewport.height);
        let details = dialog_cx.debug_bounds("plugin-install-progress").unwrap();
        let content_before = dialog_cx
            .debug_bounds("plugin-install-prompt-details")
            .unwrap();
        dialog_cx.simulate_event(gpui_kit::ScrollWheelEvent {
            position: details.center(),
            delta: gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-300.))),
            touch_phase: gpui_kit::TouchPhase::Moved,
            modifiers: Default::default(),
        });
        dialog_cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(
            dialog_cx
                .debug_bounds("plugin-install-prompt-details")
                .unwrap()
                .top()
                < content_before.top(),
            "the long installer details must actually scroll: locale={locale}, before={content_before:?}, after={:?}, viewport={viewport:?}, details={details:?}",
            dialog_cx
                .debug_bounds("plugin-install-prompt-details")
                .unwrap()
        );
        assert_eq!(
            dialog_cx.debug_bounds("plugin-install-sdk-region").unwrap(),
            checkbox
        );
        assert_eq!(
            dialog_cx
                .debug_bounds("plugin-install-native-confirm-region")
                .unwrap(),
            confirm
        );
        dialog_cx.simulate_click(checkbox.center(), Default::default());
        dialog_cx.update(|window, cx| window.draw(cx).clear(cx));
        // Mouse focus enters the base checkbox; Space must toggle without approving the installer.
        let key = gpui_kit::Keystroke::parse("space").unwrap();
        dialog_cx.simulate_event(gpui_kit::KeyDownEvent {
            keystroke: key.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        dialog_cx.simulate_event(gpui_kit::KeyUpEvent { keystroke: key });
        dialog_cx.update(|window, cx| window.draw(cx).clear(cx));
        dialog_cx.simulate_click(confirm.center(), Default::default());
        assert_eq!(control.installer_prompt().unwrap().id, first.id);
        assert!(!marker.exists());
        dialog_cx.simulate_click(checkbox.center(), Default::default());
        dialog_cx.update(|window, cx| window.draw(cx).clear(cx));
        dialog_cx.simulate_click(confirm.center(), Default::default());
        let second = wait_prompt(&control, Some(first.id));
        assert!(!second.preparation_only);
        assert!(!marker.exists());
        dialog_cx.update(|_, cx| {
            app.read(cx)
                .extensions
                .clone()
                .update(cx, |_, cx| cx.notify())
        });
        dialog_cx.update(|window, cx| window.draw(cx).clear(cx));
        let confirm = dialog_cx
            .debug_bounds("plugin-install-native-confirm-region")
            .unwrap();
        dialog_cx.simulate_click(confirm.center(), Default::default());
        worker.join().unwrap().unwrap();
        assert!(marker.exists());
        dialog_cx.update(|window, _| window.remove_window());
    }
    rust_i18n::set_locale(&previous_locale);
}

/// Bound the test's wait across the real worker/UI handoff, including prompt replacement.
fn wait_prompt(
    control: &plugin_runtime::InstallControl,
    previous: Option<u64>,
) -> plugin_runtime::InstallerPrompt {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(prompt) = control
            .installer_prompt()
            .filter(|prompt| Some(prompt.id) != previous)
        {
            return prompt;
        }
        assert!(
            Instant::now() < deadline,
            "no new installer prompt after {previous:?}; current={:?}",
            control.installer_prompt()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
