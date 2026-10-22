//! Large actual Git repositories exercise bounded published scenes through the public WASM manager.
use super::*;

/// Repeated pages retain selectable rows while a wheel burst publishes only its native viewport.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real 8193-commit repository"]
fn git_package_large_history_keeps_scroll_scenes_bounded() {
    if !run_isolated_test(
        "workbench::history::performance::git_package_large_history_keeps_scroll_scenes_bounded",
    ) {
        return;
    }
    let repository = repository();
    import_history(repository.path(), 8193);
    let (_profile, mut manager) = activate(&candidate(), repository.path());
    send(
        &mut manager,
        "history",
        "log",
        ui::Action::Canvas(ui::CanvasEvent::Resize {
            width: 1000.,
            height: 280.,
            grid: None,
        }),
    );
    let started = Instant::now();
    for page in 2..=65 {
        click(&mut manager, "history", "more");
        assert_eq!(
            log_range(&manager).content,
            (page * 128).min(8193) as f32 * 28.
        );
    }
    eprintln!(
        "8193 actual Git rows admitted through 65 pages in {:?}",
        started.elapsed()
    );
    let selected = manager.live["git"].views["history"]
        .active_node("history-oid")
        .unwrap()
        .clone();
    let captured = manager.live["git"].views["history"].clone();
    let ui::Kind::Canvas(canvas) = &captured.active_node("log").unwrap().kind else {
        panic!("canvas")
    };
    let epoch = canvas.scroll_epoch.unwrap();
    let started = Instant::now();
    for _ in 0..128 {
        manager
            .event(
                "git",
                Some("history".into()),
                api::Notification::Ui(ui::UiEvent {
                    revision: captured.revision,
                    node: "log".into(),
                    action: ui::Action::Canvas(ui::CanvasEvent::ScrollInput {
                        epoch,
                        change: ui::ScrollChange::Delta { delta_y: -7. },
                    }),
                }),
            )
            .unwrap();
        let scene = &manager.live["git"].views["history"];
        let ui::Kind::Canvas(canvas) = &scene.active_node("log").unwrap().kind else {
            panic!("canvas")
        };
        assert!(
            canvas.paint.len() < 160,
            "drawing exceeded viewport plus the two native-scroll buffers"
        );
        let buffer = canvas.scroll_buffer.as_ref().expect("native scroll buffer");
        assert!(buffer.end - buffer.start <= 280. * 3. + 84.);
        assert!(
            serde_json::to_vec(scene.as_ref()).unwrap().len() < 64 * 1024,
            "offscreen data crossed the WASM scene boundary"
        );
    }
    eprintln!(
        "128 actual WASM scrolls with 8193 loaded rows in {:?}",
        started.elapsed()
    );
    assert_eq!(log_range(&manager).offset, 896.);
    assert_eq!(
        manager.live["git"].views["history"]
            .active_node("history-oid")
            .unwrap(),
        &selected
    );
    send(
        &mut manager,
        "history",
        "log",
        ui::Action::Canvas(ui::CanvasEvent::Scroll {
            offset: 8193. * 28. - 280.,
        }),
    );
    let ui::Kind::Canvas(canvas) = &manager.live["git"].views["history"]
        .active_node("log")
        .unwrap()
        .kind
    else {
        panic!("canvas")
    };
    assert!(canvas.paint.iter().any(|paint| matches!(paint, plugin_runtime::plugin_protocol::Paint::Text { text, .. } if text == "history-000")));
    first_commit(&mut manager);
    let expected = git(
        repository.path(),
        &["rev-list", "--skip=8183", "--max-count=1", "HEAD"],
    );
    assert_eq!(preview_text(&manager, "history-oid"), expected.trim());
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}
