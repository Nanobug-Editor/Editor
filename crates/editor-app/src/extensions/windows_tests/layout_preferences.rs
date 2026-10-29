//! A real independent Git package restores opaque preferences after its Manager is dropped/reopened.
use super::*;
use plugin_runtime::plugin_protocol::{api, ui};

/// Drive the public UI event boundary against the currently admitted native document.
fn release(manager: &mut Manager, node: &str, action: ui::Action) {
    let revision = manager.live["git"].views["history"].revision;
    manager
        .event(
            "git",
            Some("history".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: node.into(),
                action,
            }),
        )
        .unwrap();
}

/// Cold restart carries no Snapshot or old WASM memory; only the private workspace record survives.
#[gpui::test]
#[ignore = "requires Git 0.1.49 candidate ZIP in NANOBUG_GIT_CANDIDATE"]
fn git_layout_preferences_survive_a_cold_manager_restart(cx: &mut TestAppContext) {
    if !run_isolated_test(
        "extensions::windows_tests::layout_preferences::git_layout_preferences_survive_a_cold_manager_restart",
    ) {
        return;
    }
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let root = path.parent().unwrap();
        for args in [
            vec!["init", "-b", "main"],
            vec!["config", "user.name", "Layout fixture"],
            vec!["config", "user.email", "layout@example.invalid"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["add", "."],
            vec!["commit", "-m", "Layout baseline"],
        ] {
            assert!(git(root, &args));
        }
        let package = Package::read(Path::new(
            &std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate ZIP"),
        ))
        .unwrap();
        let profile = root.join(".runtime-plugin-test");
        let env = visual.update(|_, cx| environment(root, cx));
        let mut manager = Manager::open(profile.clone(), env).unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        package_ui_test_support::demand(
            &mut manager,
            "git",
            plugin_runtime::DemandReason::View("history".into()),
        )
        .unwrap();
        flush(&mut manager, &app, visual);
        for (node, weights) in [
            ("history-body", vec![0.2, 0.5, 0.3]),
            ("history-preview", vec![0.6, 0.4]),
        ] {
            release(&mut manager, node, ui::Action::SplitResized(weights));
            flush(&mut manager, &app, visual);
        }
        let canvas = match &manager.live["git"].views["history"]
            .active_node("log")
            .unwrap()
            .kind
        {
            ui::Kind::Canvas(canvas) => canvas,
            _ => panic!("history canvas"),
        };
        let epoch = canvas
            .column_resize
            .as_ref()
            .expect("loaded history columns")
            .epoch;
        release(
            &mut manager,
            "log",
            ui::Action::Canvas(ui::CanvasEvent::ColumnsResized {
                epoch,
                widths: vec![420., 210., 90., 150.],
            }),
        );
        flush(&mut manager, &app, visual);
        manager.disable("git").unwrap();
        flush(&mut manager, &app, visual);
        drop(manager);

        let env = visual.update(|_, cx| environment(root, cx));
        let mut manager = Manager::open(profile, env).unwrap();
        manager.enable("git").unwrap();
        package_ui_test_support::demand(
            &mut manager,
            "git",
            plugin_runtime::DemandReason::View("history".into()),
        )
        .unwrap();
        flush(&mut manager, &app, visual);
        let document = &manager.live["git"].views["history"];
        assert_eq!(
            document
                .active_node("history-body")
                .unwrap()
                .layout
                .split_proportions,
            Some(vec![0.2, 0.5, 0.3])
        );
        assert_eq!(
            document
                .active_node("history-preview")
                .unwrap()
                .layout
                .split_proportions,
            Some(vec![0.6, 0.4])
        );
        let ui::Kind::Canvas(canvas) = &document.active_node("log").unwrap().kind else {
            panic!("history canvas");
        };
        assert_eq!(
            &canvas.column_resize.as_ref().unwrap().widths[1..],
            &[210., 90., 150.]
        );
        manager.disable("git").unwrap();
        flush(&mut manager, &app, visual);
    });
}
