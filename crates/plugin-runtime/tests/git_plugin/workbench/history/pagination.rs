//! Native-equivalent range events load real Git history through its root without a menu action.
use super::*;
use plugin_runtime::plugin_protocol::Paint;

/// The actual package crosses several 128-row boundaries and previews the real root under the pointer.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real paged repository"]
fn git_package_scrolling_history_reaches_the_repository_root() {
    if !run_isolated_test(
        "workbench::history::pagination::git_package_scrolling_history_reaches_the_repository_root",
    ) {
        return;
    }
    let repository = repository();
    let root = repository.path();
    import_history(root, 385);
    let total: usize = git(root, &["rev-list", "--count", "--all"])
        .trim()
        .parse()
        .unwrap();
    let oldest = git(root, &["rev-list", "--max-parents=0", "--all"]);
    let oldest = oldest.trim();
    let subject = git(root, &["show", "-s", "--format=%s", oldest]);
    let (_profile, mut manager) = activate(&candidate(), root);
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
    let selected = preview_text(&manager, "history-oid");
    for _ in 0..8 {
        let scene = &manager.live["git"].views["history"];
        let ui::Kind::Canvas(canvas) = &scene.active_node("log").unwrap().kind else {
            panic!("native history canvas");
        };
        let epoch = canvas.scroll_epoch.unwrap();
        let offset = (canvas.scroll.as_ref().unwrap().content - 280.).max(0.);
        send(
            &mut manager,
            "history",
            "log",
            ui::Action::Canvas(ui::CanvasEvent::ScrollInput {
                epoch,
                change: ui::ScrollChange::To { offset },
            }),
        );
    }
    let range = log_range(&manager);
    assert_eq!(range.content, total as f32 * 28.);
    assert_eq!(range.offset + 280., range.content);
    assert_eq!(preview_text(&manager, "history-oid"), selected);
    let scene = &manager.live["git"].views["history"];
    let ui::Kind::Canvas(canvas) = &scene.active_node("log").unwrap().kind else {
        panic!("native history canvas");
    };
    assert!(
        canvas.paint.iter().any(|paint| matches!(paint,
            Paint::Text { y, text, .. }
                if text == subject.trim() && *y >= 0. && *y + 18. <= 280.
        )),
        "the real root's complete row must be inside the final viewport"
    );
    send(
        &mut manager,
        "history",
        "log",
        ui::Action::Canvas(ui::CanvasEvent::Pointer {
            phase: ui::PointerPhase::Down,
            x: 240.,
            y: 266.,
            button: 0,
            clicks: 1,
            shift: false,
        }),
    );
    assert_eq!(preview_text(&manager, "history-oid"), oldest);
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}
