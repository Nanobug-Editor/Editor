//! Project-dropdown interaction covers native search, history removal and explicit cloning.
use super::*;

/// Cancelling a native close keeps cloning alive; successful completion opens a peer project window.
#[gpui::test]
fn project_clone_from_active_editor_survives_cancelled_close(cx: &mut TestAppContext) {
    let git = plugin_runtime::native_processes::resolve_program("git").unwrap();
    let source = tempfile::tempdir().unwrap();
    assert!(
        std::process::Command::new(git)
            .args(["init", "--quiet"])
            .arg(source.path())
            .status()
            .unwrap()
            .success()
    );
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("a.txt"), "A original").unwrap();
    let destination = tempfile::tempdir().unwrap();
    let target = destination.path().join("clone-from-editor");
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, host| {
        pick_restricted_project(visual, project.path());
        visual.simulate_input("A retained 中文");
        let original = visual.update(|_, cx| host.read(cx).editor.clone().unwrap());
        let text = visual.update(|_, cx| original.read(cx).editor.read(cx).value().to_string());
        click(visual, "project-clone");
        visual.simulate_keystrokes("tab");
        visual.simulate_input(&source.path().display().to_string());
        click(visual, "project-clone-path");
        visual.simulate_input(&target.display().to_string());
        click(visual, "project-clone-start");
        request_close_project(visual);
        assert!(visual.debug_bounds("project-decision-cancel").is_some());
        assert!(visual.update(|_, cx| host.read(cx).has_running_clone()));
        click(visual, "project-decision-cancel");
        assert!(visual.update(|_, cx| host.read(cx).clone_project.is_some()));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while window_count(visual) == 1 {
            std::thread::sleep(std::time::Duration::from_millis(20));
            visual
                .executor()
                .advance_clock(std::time::Duration::from_millis(50));
            visual.run_until_parked();
            assert!(
                std::time::Instant::now() < deadline,
                "cloning did not produce a new project window"
            );
        }
        assert!(target.join(".git").is_dir());
        assert_eq!(
            visual.update(|_, cx| host.read(cx).editor.clone()),
            Some(original.clone())
        );
        assert_eq!(
            visual.update(|_, cx| original.read(cx).editor.read(cx).value().to_string()),
            text
        );
        let peer = visual.update(|window, cx| {
            cx.windows()
                .into_iter()
                .find(|handle| handle.window_id() != window.window_handle().window_id())
                .unwrap()
        });
        let mut peer = VisualTestContext::from_window(peer, &visual.cx);
        peer.run_until_parked();
        assert!(peer.debug_bounds("project-trust-restricted").is_some());
        click(&mut peer, "project-decision-cancel");
    });
}

/// Searching by a Chinese name or path retains the original record's removal identity.
#[gpui::test]
fn project_home_search_filters_and_removes_the_matching_record(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    let projects = tempfile::tempdir().unwrap();
    let a = projects.path().join("Alpha");
    let b = projects.path().join("中文项目");
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    with_home(cx, profile.path(), |visual, host| {
        visual.update(|_, cx| {
            let recent = host.read(cx).recent.clone();
            recent.borrow_mut().opened(&a).unwrap();
            recent.borrow_mut().opened(&b).unwrap();
            cx.refresh_windows();
        });
        visual.run_until_parked();
        click(visual, "project-switcher-trigger");
        visual.simulate_input("alpha");
        visual.run_until_parked();
        assert!(recent_bounds(visual, 0).is_none());
        assert!(recent_bounds(visual, 1).is_some());
        click(visual, "project-remove-1");
        assert!(a.is_dir());
        assert_eq!(
            visual.update(|_, cx| host.read(cx).recent.borrow().entries[0].path.clone()),
            b
        );
        assert!(recent_bounds(visual, 0).is_none());
        click(visual, "project-search");
        visual.simulate_keystrokes("ctrl-a");
        visual.simulate_input("中文");
        visual.run_until_parked();
        assert!(recent_bounds(visual, 0).is_some());
        visual.simulate_keystrokes("escape");
        visual.run_until_parked();
        assert!(visual.debug_bounds("native-popup-menu").is_none());
    });
}

/// Search, scroll and management commands remain inside the viewport at the minimum window size.
#[gpui::test]
fn project_home_caption_and_actions_fit_both_themes(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, _| {
        for dark in [false, true] {
            visual.update(|_, cx| apply_theme(builtin_theme(dark), cx));
            visual.simulate_resize(size(px(720.), px(480.)));
            visual.run_until_parked();
            assert!(visual.debug_bounds("native-popup-menu").is_none());
            click(visual, "project-switcher-trigger");
            let caption = visual.debug_bounds("custom-title-bar").unwrap();
            let menu = visual.debug_bounds("native-popup-menu").unwrap();
            let search = visual.debug_bounds("project-search").unwrap();
            assert!(
                menu.top() >= caption.bottom(),
                "menu {menu:?}, caption {caption:?}, search {search:?}"
            );
            assert!(search.size.width > px(80.));
            assert!(menu.right() <= px(712.) && menu.bottom() <= px(472.));
            assert!(
                visual
                    .debug_bounds("native-menu-project-open-folder")
                    .is_some()
            );
            assert!(visual.debug_bounds("native-menu-project-clone").is_some());
            visual.simulate_keystrokes("escape");
            visual.run_until_parked();
        }
    });
}

/// Modal input Enter cannot authorize a clone, and an existing target is preserved on validation failure.
#[gpui::test]
fn project_home_clone_requires_explicit_activation_and_preserves_existing_target(
    cx: &mut TestAppContext,
) {
    let profile = tempfile::tempdir().unwrap();
    let existing = tempfile::tempdir().unwrap();
    let file = existing.path().join("keep.txt");
    std::fs::write(&file, "keep").unwrap();
    with_home(cx, profile.path(), |visual, host| {
        click(visual, "project-clone");
        click(visual, "project-clone-url");
        visual.simulate_input("https://example.invalid/repository.git");
        visual.simulate_keystrokes("enter");
        visual.run_until_parked();
        assert!(visual.debug_bounds("project-clone-progress").is_none());
        click(visual, "project-clone-path");
        visual.simulate_input(&existing.path().display().to_string());
        visual.run_until_parked();
        click(visual, "project-clone-start");
        assert!(visual.debug_bounds("project-clone-error").is_some());
        assert!(visual.update(|_, cx| host.read(cx).clone_project.is_some()));
        assert!(visual.debug_bounds("project-clone-progress").is_none());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "keep");
        visual.simulate_keystrokes("escape");
        visual.run_until_parked();
        assert!(visual.debug_bounds("project-clone-dialog").is_none());
        assert!(visual.debug_bounds("custom-title-bar").is_some());
    });
}

/// A real local Git clone enters the ordinary trust prompt; downloading never grants workspace trust.
#[gpui::test]
fn project_home_clone_local_repository_uses_normal_trust_flow(cx: &mut TestAppContext) {
    let git = plugin_runtime::native_processes::resolve_program("git")
        .expect("local Git is required for clone acceptance");
    let source = tempfile::tempdir().unwrap();
    assert!(
        std::process::Command::new(git)
            .arg("init")
            .arg("--quiet")
            .arg(source.path())
            .status()
            .unwrap()
            .success()
    );
    let destination = tempfile::tempdir().unwrap();
    let target = destination.path().join("cloned 中文");
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, _| {
        click(visual, "project-clone");
        // Native Tab enters the first input from the modal's safe initial focus boundary.
        visual.simulate_keystrokes("tab");
        visual.simulate_input(&source.path().display().to_string());
        click(visual, "project-clone-path");
        visual.simulate_input(&target.display().to_string());
        visual.run_until_parked();
        click(visual, "project-clone-start");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            std::thread::sleep(std::time::Duration::from_millis(20));
            visual
                .executor()
                .advance_clock(std::time::Duration::from_millis(50));
            visual.run_until_parked();
            if visual.debug_bounds("project-trust-restricted").is_some() {
                break;
            }
            assert!(
                visual.debug_bounds("project-clone-error").is_none(),
                "local clone failed"
            );
            assert!(
                std::time::Instant::now() < deadline,
                "local clone did not finish"
            );
        }
        assert!(target.join(".git").is_dir());
        assert!(visual.debug_bounds("editor-shell").is_none());
        click(visual, "project-decision-cancel");
        assert!(
            target.join(".git").is_dir(),
            "cancelling open must not undo the clone"
        );
        assert!(visual.debug_bounds("custom-title-bar").is_some());
        // Cancelling a second in-flight clone invalidates its polling result and restores home.
        click(visual, "project-clone");
        visual.simulate_keystrokes("tab");
        visual.simulate_input(&source.path().display().to_string());
        click(visual, "project-clone-path");
        visual.simulate_input(
            &destination
                .path()
                .join("cancelled-clone")
                .display()
                .to_string(),
        );
        visual.run_until_parked();
        click(visual, "project-clone-start");
        assert!(visual.debug_bounds("project-clone-progress").is_some());
        click(visual, "project-clone-cancel");
        visual
            .executor()
            .advance_clock(std::time::Duration::from_millis(100));
        visual.run_until_parked();
        assert!(visual.debug_bounds("project-clone-dialog").is_none());
        assert!(visual.debug_bounds("project-trust-restricted").is_none());
        assert!(visual.debug_bounds("editor-shell").is_none());
    });
}
