//! Real Git conflict blocks exercise ancestor initialization, independent edits and original Undo.
use super::*;

/// Every click goes through the actual native control, its scene and source-version gates.
fn click(
    separate: &mut VisualTestContext,
    manager: &mut Manager,
    app: &Entity<EditorApp>,
    visual: &mut VisualTestContext,
    id: &'static str,
) {
    flush(manager, app, visual);
    separate.update(|window, cx| window.draw(cx).clear(cx));
    let node = id.strip_prefix("plugin-ui-").unwrap_or(id);
    assert!(
        manager.live["git"].views["conflicts"]
            .active_node(node)
            .is_some(),
        "inactive {node}: {}",
        serde_json::to_string(&manager.live["git"].views["conflicts"]).unwrap()
    );
    // The layout wrapper may stretch within a wrapped toolbar. Click Base's actual button,
    // rather than empty padding at the wrapper's center, just as a user clicks its painted face.
    let button: &'static str = Box::leak(
        id.replacen("plugin-ui-", "plugin-button-", 1)
            .into_boxed_str(),
    );
    let bounds = separate.debug_bounds(button).unwrap_or_else(|| {
        panic!(
            "missing {id}: {}",
            serde_json::to_string(&manager.live["git"].views["conflicts"]).unwrap()
        )
    });
    let published = separate.update(|_, cx| {
        app.read(cx).plugin_panels["git/conflicts"]
            .read(cx)
            .current_document()
            .unwrap()
            .revision
    });
    assert_eq!(published, manager.live["git"].views["conflicts"].revision);
    separate.simulate_click(bounds.center(), Default::default());
    flush(manager, app, visual);
    separate.update(|window, cx| window.draw(cx).clear(cx));
}

#[gpui::test]
#[ignore = "build Git-for-Nanobug candidate with editor.actions/ui.code_editor; set NANOBUG_GIT_CANDIDATE"]
fn git_merge_edges_keep_nonconflicting_changes_and_require_manual_block_review(
    cx: &mut TestAppContext,
) {
    // Preserve the native test body inside the same private Git configuration as its Manager.
    if !run_isolated_test(
        "extensions::windows_tests::merge::git_merge_edges_keep_nonconflicting_changes_and_require_manual_block_review",
    ) {
        return;
    }
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let root = path.parent().unwrap();
        let file = root.join("merge.txt");
        for args in [
            &["init", "-b", "main"][..],
            &["config", "core.autocrlf", "false"],
            &["config", "user.name", "fixture"],
            &["config", "user.email", "fixture@example.invalid"],
        ] {
            assert!(git(root, args));
        }
        let base = "header\nbase conflict\nkeep 1\nkeep 2\nbase left\nkeep 3\nbase right\nend\n";
        std::fs::write(&file, base).unwrap();
        assert!(git(root, &["add", "merge.txt"]));
        assert!(git(root, &["commit", "-m", "base"]));
        assert!(git(root, &["switch", "-c", "feature"]));
        std::fs::write(
            &file,
            base.replace("base conflict", "local block")
                .replace("base left", "left only"),
        )
        .unwrap();
        assert!(git(root, &["commit", "-am", "local"]));
        assert!(git(root, &["switch", "main"]));
        std::fs::write(
            &file,
            base.replace("base conflict", "incoming block")
                .replace("base right", "right only"),
        )
        .unwrap();
        assert!(git(root, &["commit", "-am", "incoming"]));
        assert!(git(root, &["switch", "feature"]));
        assert!(!git(root, &["merge", "main"]));
        let profile = tempfile::tempdir().unwrap();
        let package = Package::read(Path::new(
            &std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate required"),
        ))
        .unwrap();
        let env = visual.update(|_, cx| environment(root, cx));
        let mut manager = Manager::open(profile.path().into(), env).unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        package_ui_test_support::demand(
            &mut manager,
            "git",
            plugin_runtime::DemandReason::Command("refresh".into()),
        )
        .unwrap();
        flush(&mut manager, &app, visual);
        manager
            .invoke_command("git", "resolve-file", json!({"path":"merge.txt"}))
            .unwrap();
        flush(&mut manager, &app, visual);
        let handle = visual.update(|_, cx| app.read(cx).plugin_windows["git/conflicts"]);
        let mut separate = VisualTestContext::from_window(handle.into(), &visual.cx);
        let source = visual.update(|_, cx| {
            app.read(cx)
                .tabs
                .iter()
                .find(|tab| tab.path() == file.canonicalize().unwrap())
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .editor
                .clone()
        });
        // Edge actions address the window's exact session even when the main editor selects
        // another file. This is the same scenario as reviewing a conflict beside regular work.
        let unrelated_before = std::fs::read(path).unwrap();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.open_file(path.canonicalize().unwrap(), window, cx);
            });
        });
        flush(&mut manager, &app, visual);
        click(
            &mut separate,
            &mut manager,
            &app,
            visual,
            "plugin-ui-merge-from-base",
        );
        assert_eq!(
            separate.update(|_, cx| source.read(cx).value().to_string()),
            base,
            "{}",
            serde_json::to_string(&manager.live["git"].views["conflicts"]).unwrap()
        );
        assert_eq!(std::fs::read(path).unwrap(), unrelated_before);
        click(
            &mut separate,
            &mut manager,
            &app,
            visual,
            "plugin-ui-merge-block-0-left",
        );
        assert_eq!(
            separate.update(|_, cx| source.read(cx).value().to_string()),
            base.replace("base conflict", "local block"),
            "{}",
            serde_json::to_string(&manager.live["git"].views["conflicts"]).unwrap()
        );
        assert!(
            manager.live["git"].views["conflicts"]
                .active_node("resolved")
                .is_none(),
            "pending nonconflicting changes cannot be silently discarded"
        );
        separate.update(|window, cx| source.update(cx, |editor, cx| editor.focus(window, cx)));
        separate.simulate_keystrokes("ctrl-z");
        flush(&mut manager, &app, visual);
        assert_eq!(
            separate.update(|_, cx| source.read(cx).value().to_string()),
            base
        );
        click(
            &mut separate,
            &mut manager,
            &app,
            visual,
            "plugin-ui-merge-nonconflicting",
        );
        let independent = base
            .replace("base left", "left only")
            .replace("base right", "right only");
        assert_eq!(
            separate.update(|_, cx| source.read(cx).value().to_string()),
            independent
        );
        click(
            &mut separate,
            &mut manager,
            &app,
            visual,
            "plugin-ui-merge-block-0-left",
        );
        assert_eq!(
            separate.update(|_, cx| source.read(cx).value().to_string()),
            independent.replace("base conflict", "local block")
        );
        separate.update(|window, cx| source.update(cx, |editor, cx| editor.focus(window, cx)));
        separate.simulate_keystrokes("ctrl-z");
        flush(&mut manager, &app, visual);
        assert_eq!(
            separate.update(|_, cx| source.read(cx).value().to_string()),
            independent,
            "Undo of one block must preserve independent changes"
        );
        assert!(
            manager.live["git"].views["conflicts"]
                .active_node("resolved")
                .is_none(),
            "Undo invalidates the block review"
        );
        click(
            &mut separate,
            &mut manager,
            &app,
            visual,
            "plugin-ui-merge-block-0-ignore-left",
        );
        click(
            &mut separate,
            &mut manager,
            &app,
            visual,
            "plugin-ui-merge-block-0-ignore-right",
        );
        assert!(
            manager.live["git"].views["conflicts"]
                .active_node("resolved")
                .is_some()
        );
        // A manual native change is not a reviewed block merely because it contains no markers.
        separate.update(|window, cx| {
            source.update(cx, |editor, cx| {
                editor.set_selected_range(7..20, cx);
                editor.focus(window, cx)
            })
        });
        separate.simulate_input("手工解决😀");
        flush(&mut manager, &app, visual);
        assert!(
            manager.live["git"].views["conflicts"]
                .active_node("resolved")
                .is_none()
        );
        click(
            &mut separate,
            &mut manager,
            &app,
            visual,
            "plugin-ui-merge-block-0-confirm",
        );
        click(
            &mut separate,
            &mut manager,
            &app,
            visual,
            "plugin-ui-resolved",
        );
        assert!(git(root, &["diff", "--check"]));
        let unmerged = std::process::Command::new("git")
            .current_dir(root)
            .args(["ls-files", "-u"])
            .output()
            .unwrap();
        assert!(unmerged.stdout.is_empty());
        assert!(
            root.join(".git/MERGE_HEAD").exists(),
            "saving one result cannot continue the entire merge"
        );
        assert!(
            std::fs::read_to_string(&file)
                .unwrap()
                .contains("手工解决😀")
        );
        manager.disable("git").unwrap();
    });
}
