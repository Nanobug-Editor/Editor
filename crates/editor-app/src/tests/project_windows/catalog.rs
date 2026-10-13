//! Two native project windows must keep their installed recognition and real formatter routes independent.

use super::*;
#[allow(dead_code)]
#[path = "../editing_fixture.rs"]
mod editing_fixture;

/// Opening B must leave A's native format command bound to A's declared language and provider.
#[gpui::test]
#[ignore = "uses frozen javascript Package and its approved private Node runtime for both project providers"]
fn concurrent_project_windows_keep_recognition_and_formatting_independent(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let source = "<item>中文🙂</item>\n";
    let path_a = a.path().join("source.linked");
    let path_b = b.path().join("source.linked");
    std::fs::write(&path_a, source).unwrap();
    std::fs::write(&path_b, source).unwrap();
    let package_a = editing_fixture::package(
        "project-alpha",
        "alpha",
        true,
        true,
        &a.path().join("wire.jsonl"),
        0,
        "alpha",
    );
    let package_b = editing_fixture::package(
        "project-beta",
        "beta",
        true,
        true,
        &b.path().join("wire.jsonl"),
        0,
        "beta",
    );
    let env = |path: &std::path::Path| plugin_runtime::plugin_protocol::Environment {
        workspace: path.canonicalize().unwrap().display().to_string(),
        ..Default::default()
    };
    let mut manager = plugin_runtime::Manager::open(
        crate::extensions::profile_runtime_root(profile.path()),
        env(a.path()),
    )
    .unwrap();
    manager
        .install(&package_a, package_a.manifest.permissions.clone())
        .unwrap();
    manager
        .install(&package_b, package_b.manifest.permissions.clone())
        .unwrap();
    manager.disable("project-alpha").unwrap();
    manager.disable("project-beta").unwrap();
    manager.set_project_enabled("project-alpha", true).unwrap();
    let mut first = None;
    with_home(cx, profile.path(), |visual, host| {
        visual.update(|window, cx| {
            host.update(cx, |host, cx| host.open_path(path_a.clone(), window, cx))
        });
        visual.run_until_parked();
        click(visual, "project-trust-accept");
        let editor = visual.update(|_, cx| host.read(cx).editor.clone().unwrap());
        crate::extensions::lsp_tests::publish(&editor, &mut manager, visual);
        first = Some((visual.clone(), editor));
    });
    let (mut visual_a, editor_a) = first.unwrap();
    manager.select_workspace(env(b.path()), true).unwrap();
    manager.set_project_enabled("project-beta", true).unwrap();
    let mut second = None;
    with_home(cx, profile.path(), |visual, host| {
        visual.update(|window, cx| {
            host.update(cx, |host, cx| host.open_path(path_b.clone(), window, cx))
        });
        visual.run_until_parked();
        click(visual, "project-trust-accept");
        let editor = visual.update(|_, cx| host.read(cx).editor.clone().unwrap());
        crate::extensions::lsp_tests::publish(&editor, &mut manager, visual);
        second = Some((visual.clone(), editor));
    });
    let (mut visual_b, editor_b) = second.unwrap();
    // Observe actual native commands and document text after both scopes have published, not private selection maps.
    visual_a.simulate_keystrokes("shift-alt-f");
    visual_b.simulate_keystrokes("shift-alt-f");
    let expected_a = format!("/* alpha */\n{source}");
    let expected_b = format!("/* beta */\n{source}");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        visual_a.run_until_parked();
        visual_b.run_until_parked();
        let actual_a =
            visual_a.update(|_, cx| editor_a.read(cx).editor.read(cx).value().to_string());
        let actual_b =
            visual_b.update(|_, cx| editor_b.read(cx).editor.read(cx).value().to_string());
        if actual_a == expected_a && actual_b == expected_b {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "cross-window format result A={actual_a:?}, B={actual_b:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // User editing policy uses one authoritative store even though A and B resolve different project roles.
    visual_a.update(|_, cx| {
        editor_a
            .read(cx)
            .providers
            .set_editing_preference("*", true, Some(true))
            .unwrap()
    });
    visual_b.update(|_, cx| {
        editor_b
            .read(cx)
            .providers
            .set_editing_preference("beta", false, Some(false))
            .unwrap()
    });
    assert!(visual_b.update(|_, cx| {
        editor_b
            .read(cx)
            .providers
            .editing_preferences("beta")
            .format_on_save
    }));
    assert!(!visual_a.update(|_, cx| {
        editor_a
            .read(cx)
            .providers
            .editing_preferences("beta")
            .linked_editing
    }));
    // Inherited user policy must affect B's real Save action, after B made its own independent preference edit.
    visual_b.update(|window, cx| {
        editor_b.read(cx).editor.clone().update(cx, |editor, cx| {
            editor.set_value("B policy source", window, cx);
            cx.emit(gpui_base::input::InputEvent::Change);
        })
    });
    visual_b.simulate_keystrokes("ctrl-s");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        visual_b.run_until_parked();
        if std::fs::read_to_string(&path_b).unwrap() == "/* beta */\nB policy source" {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "B did not inherit A's format-on-save policy"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    visual_a.simulate_keystrokes("ctrl-alt-t");
    visual_a.run_until_parked();
    assert!(visual_b.update(|_, cx| editor_b.read(cx).dark_theme));
    manager.shutdown_checked().unwrap();
}
