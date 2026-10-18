//! Exercise title selection and project switching through their painted native controls.

use super::*;

/// Native canonical roots remain valid identities while home, title and menu labels stay readable.
#[cfg(windows)]
#[gpui::test]
fn project_paths_display_without_windows_verbatim_prefixes(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("路径 验收");
    std::fs::create_dir(&project).unwrap();
    let canonical = std::fs::canonicalize(&project).unwrap();
    assert!(canonical.to_string_lossy().starts_with(r"\\?\"));
    let expected = project.display().to_string();
    std::fs::write(
        profile.path().join("recent-projects.json"),
        serde_json::to_vec(&vec![serde_json::json!({ "path": canonical })]).unwrap(),
    )
    .unwrap();
    with_home(cx, profile.path(), |visual, host| {
        click(visual, "project-switcher-trigger");
        let labels = visual.update(|window, _| {
            gpui_base::test_support::snapshots(window)
                .into_iter()
                .filter_map(|item| item.label().map(str::to_owned))
                .collect::<Vec<_>>()
        });
        assert!(
            labels.iter().any(|label| label.ends_with(&expected)),
            "recent-project labels must show the ordinary disk path: {labels:?}"
        );
        assert!(labels.iter().all(|label| !label.contains(r"\\?\")));
        pick_restricted_project(visual, &project);
        assert_eq!(
            visual.update(|_, cx| host
                .read(cx)
                .editor
                .as_ref()
                .unwrap()
                .read(cx)
                .workspace
                .root()
                .to_owned()),
            canonical,
            "display formatting must not change the project's canonical identity"
        );
        click(visual, "project-switcher-trigger");
        let labels = visual.update(|window, _| {
            gpui_base::test_support::snapshots(window)
                .into_iter()
                .filter_map(|item| item.label().map(str::to_owned))
                .collect::<Vec<_>>()
        });
        assert!(labels.iter().any(|label| label.contains(&expected)));
        assert!(
            labels.iter().all(|label| !label.contains(r"\\?\")),
            "project title and popup labels must omit the verbatim prefix: {labels:?}"
        );
    });
}

/// Seed the external profile input used to exercise a history longer than the viewport.
fn seed_recent_projects(profile: &std::path::Path, projects: &std::path::Path) {
    let mut recent = Vec::new();
    for index in 0..24 {
        let path = projects.join(format!("project-{index}"));
        std::fs::create_dir(&path).unwrap();
        recent.push(serde_json::json!({ "path": std::fs::canonicalize(path).unwrap() }));
    }
    std::fs::write(
        profile.join("recent-projects.json"),
        serde_json::to_vec(&recent).unwrap(),
    )
    .unwrap();
}

/// Creating a fresh profile during the first open must not split later windows into another list.
#[gpui::test]
fn project_switcher_fresh_profile_keeps_shared_window_membership(cx: &mut TestAppContext) {
    let storage = tempfile::tempdir().unwrap();
    let profile = storage.path().join("new-profile");
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    with_home(cx, &profile, |visual, _| {
        pick_restricted_project(visual, a.path());
        click(visual, "project-open-another");
        let path = b.path().to_owned();
        visual.simulate_path_prompt_response(move |_| Some(vec![path]));
        visual.run_until_parked();
        let target = visual.update(|window, cx| {
            cx.windows()
                .into_iter()
                .find(|entry| entry.window_id() != window.window_handle().window_id())
                .unwrap()
        });
        let mut peer = VisualTestContext::from_window(target, &visual.cx);
        peer.run_until_parked();
        click(&mut peer, "project-trust-restricted");
        click(visual, "project-switcher-trigger");
        visual.simulate_keystrokes("home enter");
        visual.run_until_parked();
        assert_eq!(
            peer.update(|_, cx| cx.active_window().unwrap().window_id()),
            target.window_id()
        );
        assert_eq!(peer.update(|_, cx| cx.windows().len()), 2);
    });
}

/// The welcome body opens the native picker; its dedicated caption remains free of project actions.
#[gpui::test]
fn project_switcher_home_title_selects_folder_in_place(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, _| {
        assert!(visual.debug_bounds("custom-title-bar").is_some());
        assert!(visual.debug_bounds("project-select-folder-title").is_none());
        click(visual, "project-open-folder");
        assert!(visual.did_prompt_for_paths());
        visual.simulate_path_prompt_response(|_| None);
        visual.run_until_parked();
        assert!(visual.debug_bounds("custom-title-bar").is_some());
        assert!(visual.debug_bounds("editor-shell").is_none());
        click(visual, "project-open-folder");
        let path = project.path().to_owned();
        visual.simulate_path_prompt_response(move |_| Some(vec![path]));
        visual.run_until_parked();
        click(visual, "project-trust-restricted");
        assert!(visual.debug_bounds("editor-shell").is_some());
        assert_eq!(visual.update(|_, cx| cx.windows().len()), 1);
    });
}

/// Choosing a recently closed project creates a peer window; choosing an open one activates it.
#[gpui::test]
fn project_switcher_opens_recent_in_new_window_and_activates_open_project(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    std::fs::write(a.path().join("a.txt"), "A original").unwrap();
    with_home(cx, profile.path(), |visual, host| {
        pick_restricted_project(visual, a.path());
        assert!(visual.debug_bounds("project-switcher-trigger").is_some());
        let original = visual.update(|_, cx| host.read(cx).editor.clone().unwrap());
        visual.simulate_input("A unsaved 中文");
        visual.run_until_parked();
        click(visual, "project-open-another");
        let path = b.path().to_owned();
        visual.simulate_path_prompt_response(move |_| Some(vec![path]));
        visual.run_until_parked();
        let peer_handle = visual.update(|window, cx| {
            cx.windows()
                .into_iter()
                .find(|entry| entry.window_id() != window.window_handle().window_id())
                .unwrap()
        });
        let mut peer = VisualTestContext::from_window(peer_handle, &visual.cx);
        peer.run_until_parked();
        click(&mut peer, "project-trust-restricted");
        click(visual, "project-switcher-trigger");
        // A project with no document has no editor focus path; enter the actual native close gate.
        peer.update(|window, cx| {
            let root = window.root::<Root>().unwrap().unwrap();
            let host = root
                .read(cx)
                .view()
                .clone()
                .downcast::<ProjectWindow>()
                .unwrap();
            host.update(cx, |host, cx| {
                host.request_close(crate::app::projects::CloseIntent::Window, window, cx)
            });
        });
        peer.run_until_parked();
        assert_eq!(visual.update(|_, cx| cx.windows().len()), 1);
        assert!(visual.debug_bounds("native-popup-menu").is_some());
        assert!(visual.debug_bounds("native-menu-projects-opened").is_some());
        assert!(visual.debug_bounds("native-menu-projects-recent").is_some());
        // End skips group headings and chooses the sole recently closed project.
        visual.simulate_keystrokes("end up up up enter");
        visual.run_until_parked();
        assert_eq!(visual.update(|_, cx| cx.windows().len()), 2);
        let reopened_handle = visual.update(|window, cx| {
            cx.windows()
                .into_iter()
                .find(|entry| entry.window_id() != window.window_handle().window_id())
                .unwrap()
        });
        let mut reopened = VisualTestContext::from_window(reopened_handle, &visual.cx);
        reopened.run_until_parked();
        assert!(reopened.debug_bounds("editor-shell").is_some());
        assert!(reopened.debug_bounds("project-trust-restricted").is_none());
        click(visual, "project-switcher-trigger");
        // B was opened last, so the first selectable row targets its existing native window.
        visual.simulate_keystrokes("home enter");
        visual.run_until_parked();
        assert_eq!(visual.update(|_, cx| cx.windows().len()), 2);
        assert_eq!(
            reopened.update(|_, cx| cx.active_window().unwrap().window_id()),
            reopened_handle.window_id()
        );
        assert_eq!(
            visual.update(|_, cx| host.read(cx).editor.clone()),
            Some(original.clone())
        );
        assert!(
            visual
                .update(|_, cx| original.read(cx).editor.read(cx).value().to_string())
                .contains("A unsaved 中文")
        );
        assert_eq!(
            std::fs::read_to_string(a.path().join("a.txt")).unwrap(),
            "A original"
        );
    });
}

/// Forgetting a live project keeps its window, and closing cannot recreate its history record.
#[gpui::test]
fn project_switcher_removed_open_record_stays_removed_after_close(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, _| {
        pick_restricted_project(visual, project.path());
        click(visual, "project-switcher-trigger");
        click(visual, "project-remove-0");
        assert!(recent_path(visual, 0).is_none());
        assert!(visual.debug_bounds("native-menu-projects-opened").is_some());
        assert_eq!(window_count(visual), 1);
        assert!(project.path().is_dir());
        click(visual, "project-close-project");
        assert_eq!(window_count(visual), 0);
    });
    with_home(cx, profile.path(), |visual, _| {
        click(visual, "project-switcher-trigger");
        assert!(recent_path(visual, 0).is_none());
    });
}

/// A persisted list can exceed the card height; End still selects its oldest project.
#[gpui::test]
fn project_switcher_scrolls_all_recent_records_and_restores_focus(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    let projects = tempfile::tempdir().unwrap();
    // This external profile fixture represents projects loaded by a previous application run.
    seed_recent_projects(profile.path(), projects.path());
    let current = projects.path().join("current");
    std::fs::create_dir(&current).unwrap();
    with_home(cx, profile.path(), |visual, _| {
        pick_restricted_project(visual, &current);
        let original_focus = visual.update(|window, cx| window.focused(cx));
        visual.simulate_resize(size(px(800.), px(500.)));
        click(visual, "project-switcher-trigger");
        let menu = visual.debug_bounds("native-popup-menu").unwrap();
        assert!(menu.size.height <= px(460.) && menu.bottom() <= px(492.));
        visual.simulate_keystrokes("escape");
        visual.run_until_parked();
        assert_eq!(
            visual.update(|window, cx| window.focused(cx)),
            original_focus
        );
        click(visual, "project-switcher-trigger");
        visual.simulate_keystrokes("end up up up enter");
        visual.run_until_parked();
        let target = visual.update(|window, cx| {
            cx.windows()
                .into_iter()
                .find(|entry| entry.window_id() != window.window_handle().window_id())
                .unwrap()
        });
        let mut peer = VisualTestContext::from_window(target, &visual.cx);
        peer.run_until_parked();
        click(&mut peer, "project-trust-restricted");
        let root = peer.update(|window, cx| {
            let base = window.root::<Root>().unwrap().unwrap();
            let host = base
                .read(cx)
                .view()
                .clone()
                .downcast::<ProjectWindow>()
                .unwrap();
            host.read(cx)
                .editor
                .as_ref()
                .unwrap()
                .read(cx)
                .workspace
                .root()
                .to_owned()
        });
        assert_eq!(root.file_name().unwrap(), "project-23");
    });
}

/// Removing or opening a scrolled selection in another window keeps the highlighted row visible.
#[gpui::test]
fn project_switcher_live_changes_keep_keyboard_selection_visible(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    let projects = tempfile::tempdir().unwrap();
    seed_recent_projects(profile.path(), projects.path());
    let current = projects.path().join("current");
    std::fs::create_dir(&current).unwrap();
    with_home(cx, profile.path(), |visual, _| {
        pick_restricted_project(visual, &current);
        // A second explicitly created empty root observes the same profile history.
        visual.update(|_, cx| {
            super::super::controller::open(profile.path().to_owned(), None, cx).unwrap()
        });
        let home_handle = visual.update(|window, cx| {
            cx.windows()
                .into_iter()
                .find(|entry| entry.window_id() != window.window_handle().window_id())
                .unwrap()
        });
        let mut home = VisualTestContext::from_window(home_handle, &visual.cx);
        home.run_until_parked();
        click(visual, "project-switcher-trigger");
        visual.simulate_keystrokes("end up up up");
        visual.run_until_parked();
        click(&mut home, "project-switcher-trigger");
        let position = recent_bounds(&mut home, 0).unwrap().center();
        home.simulate_event(gpui_kit::ScrollWheelEvent {
            position,
            delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-5000.))),
            touch_phase: gpui_kit::TouchPhase::Moved,
            ..Default::default()
        });
        home.run_until_parked();
        click(&mut home, "project-remove-24");
        let menu = visual.debug_bounds("native-popup-menu").unwrap();
        let selected = visual.debug_bounds("native-menu-selected-label");
        assert!(
            selected.is_some_and(|bounds| menu.contains(&bounds.center())),
            "removed selection must fall back into view: {selected:?}"
        );
        visual.simulate_keystrokes("end up up up");
        visual.run_until_parked();
        // History changed in the peer dropdown; scroll its refreshed records into view before clicking.
        let position = home.debug_bounds("project-search").unwrap().center()
            + gpui_kit::point(px(0.), px(80.));
        home.simulate_event(gpui_kit::ScrollWheelEvent {
            position,
            delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-5000.))),
            touch_phase: gpui_kit::TouchPhase::Moved,
            ..Default::default()
        });
        home.run_until_parked();
        click(&mut home, "project-recent-23");
        click(&mut home, "project-trust-restricted");
        let menu = visual.debug_bounds("native-popup-menu").unwrap();
        let selected = visual.debug_bounds("native-menu-selected-label");
        assert!(
            selected.is_some_and(|bounds| menu.contains(&bounds.center())),
            "a selection moving to the open group must stay visible: {selected:?}"
        );
    });
}
