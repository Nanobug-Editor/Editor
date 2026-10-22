//! Frozen Git packages enter the real native document without replacing its text or undo owner.
use super::*;
use gpui_kit::gpui;
use plugin_runtime::{
    Package,
    plugin_protocol::{Environment, api},
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

/// Finish actual Git subprocesses; this fixture only handles non-editing native requests.
fn settle(manager: &mut Manager) {
    let deadline = Instant::now() + Duration::from_secs(45);
    let mut quiet = 0;
    loop {
        manager.poll();
        for (_, request) in manager.take_editor_requests() {
            match request.operation().clone() {
                api::EditorOperation::ListDocuments => {
                    request.finish(Ok(api::EditorValue::Documents {
                        documents: vec![],
                        active: None,
                    }))
                }
                api::EditorOperation::SetPanelVisibility { panel, visible } => {
                    request.finish(Ok(api::EditorValue::PanelVisibility { panel, visible }))
                }
                other => panic!("unexpected native request during Git installation: {other:?}"),
            }
        }
        quiet = if manager.live["git"].process_count() == 0 {
            quiet + 1
        } else {
            0
        };
        if quiet == 8 {
            break;
        }
        assert!(Instant::now() < deadline, "Git startup did not settle");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Initialize only the helper's disposable document directory, under the shared private Git config.
fn git(root: &Path, args: &[&str]) {
    super::super::git_fixture_environment::require_private_configuration().unwrap();
    let output = std::process::Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Installation, a successful cutover, failed candidate and disable all preserve native Undo/Redo.
#[gpui::test]
#[ignore = "real Git 0.1.23/0.1.24 ZIPs and native GPUI; set NANOBUG_GIT_BASELINE and NANOBUG_GIT_CANDIDATE"]
fn git_sdk_hot_update_preserves_native_document_and_undo_redo(cx: &mut TestAppContext) {
    if !super::super::git_fixture_environment::run_isolated_test(
        "extensions::developer_tooling_tests::git::git_sdk_hot_update_preserves_native_document_and_undo_redo",
    ) {
        return;
    }
    let profile = tempfile::tempdir().unwrap();
    let old_bytes =
        std::fs::read(std::env::var_os("NANOBUG_GIT_BASELINE").expect("frozen baseline ZIP"))
            .unwrap();
    use sha2::Digest as _;
    assert_eq!(
        format!("{:x}", sha2::Sha256::digest(&old_bytes)),
        "7d0ea99e484afafe343902a1cc30baaebbb7a618ae1c4ecee4144579727fe5bd"
    );
    let old = Package::from_bytes(&old_bytes).unwrap();
    let candidate = Package::read(Path::new(
        &std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("new ZIP"),
    ))
    .unwrap();
    let process = std::process::id();
    crate::tests::with_shortcut_editor(cx, false, vec![], |visual, app, path| {
        let root = path.parent().unwrap();
        git(root, &["init", "-b", "main"]);
        git(
            root,
            &[
                "-c",
                "user.name=SDK acceptance",
                "-c",
                "user.email=sdk@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--allow-empty",
                "-m",
                "baseline",
            ],
        );
        let mut manager = Manager::open(
            profile.path().join("runtime"),
            Environment {
                workspace: root.display().to_string(),
                os: "windows".into(),
                ..Default::default()
            },
        )
        .unwrap();
        visual.simulate_keystrokes("ctrl-a");
        visual.simulate_input("未保存的 Git SDK 原生验收");
        let original_editor = visual.update(|_, cx| app.read(cx).editor.clone());
        let text = visual.update(|_, cx| original_editor.read(cx).text().to_string());
        assert_eq!(text, "未保存的 Git SDK 原生验收");
        assert_ne!(std::fs::read_to_string(path).unwrap(), text);
        for phase in ["install", "update", "failed", "disable"] {
            match phase {
                "install" => {
                    manager
                        .install(&old, old.manifest.permissions.clone())
                        .unwrap();
                    super::super::demand_plugin_fixture(
                        &mut manager,
                        "git",
                        plugin_runtime::DemandReason::View("history".into()),
                    )
                    .unwrap();
                    settle(&mut manager);
                }
                "update" => {
                    manager
                        .install(&candidate, candidate.manifest.permissions.clone())
                        .unwrap();
                    settle(&mut manager);
                    assert_eq!(manager.installed["git"].manifest.version, "0.1.24");
                }
                "failed" => {
                    let identity = manager.instance_id("git").unwrap().to_owned();
                    let mut files = candidate.files.clone();
                    let mut manifest: serde_json::Value =
                        serde_json::from_slice(&files["manifest.json"]).unwrap();
                    manifest["version"] = "0.1.25".into();
                    files.insert(
                        "manifest.json".into(),
                        serde_json::to_vec(&manifest).unwrap(),
                    );
                    files.insert(
                        candidate.manifest.component.clone().unwrap(),
                        b"\0asm\x0d\0\x01\0invalid".to_vec(),
                    );
                    let failed = Package::from_files(files).unwrap();
                    assert!(
                        manager
                            .install(&failed, failed.manifest.permissions.clone())
                            .is_err()
                    );
                    assert_eq!(manager.instance_id("git").unwrap(), identity);
                }
                "disable" => {
                    manager.disable("git").unwrap();
                    assert_eq!(manager.resource_count(), 0);
                }
                _ => unreachable!(),
            }
            publish(&mut manager, &app, visual);
            if phase != "disable" {
                assert!(
                    visual.debug_bounds("plugin-ui-commit-message").is_some(),
                    "{phase}: actual Git form must render"
                );
            }
            visual.update(|window, cx| {
                assert_eq!(std::process::id(), process);
                assert_eq!(app.read(cx).editor, original_editor);
                assert_eq!(original_editor.read(cx).text().to_string(), text, "{phase}");
                original_editor.update(cx, |editor, cx| editor.focus(window, cx));
            });
            visual.simulate_keystrokes("ctrl-z");
            let undone = visual.update(|_, cx| original_editor.read(cx).text().to_string());
            assert_ne!(
                undone, text,
                "{phase}: original Undo transaction must survive"
            );
            visual.simulate_keystrokes("ctrl-y");
            assert_eq!(
                visual.update(|_, cx| original_editor.read(cx).text().to_string()),
                text,
                "{phase}: Redo must restore the native transaction"
            );
            assert_ne!(std::fs::read_to_string(path).unwrap(), text);
        }
        manager.shutdown_checked().unwrap();
    });
}
