//! Real Windows/D3D acceptance children exercise the installed guest through production native routes.
//!
//! No TestAppContext, TestPlatform, copied editor, domain-specific host API or user profile is used.
//! The parent owns child termination and temporary storage; project shutdown is awaited before success.
use super::{Manager, Package, Worker};
use crate::*;
use anyhow::{Context as _, anyhow, ensure};
use gpui_kit::{AnyWindowHandle, AsyncApp};
use std::{cell::RefCell, fs};

mod bridge;
mod exit_tests;
mod fixture;
mod input;
mod lifecycle;
mod matrix;
mod scenario;
pub(super) mod scene_probe;
mod windows;
use bridge::Harness;
use fixture::Fixture;
use matrix::Case;

/// Observe the original event against the actor's actual Manager immediately before admission.
/// This diagnostic does not mutate the event, poll timers or replace the production result.
pub(super) fn probe_key_admission(
    manager: &Manager,
    id: &str,
    panel: Option<&str>,
    event: &plugin_runtime::plugin_protocol::api::Notification,
) -> Option<serde_json::Value> {
    use plugin_runtime::plugin_protocol::{
        api::Notification,
        ui::{Action, CanvasEvent},
    };
    if std::env::var_os("NANOBUG_GIT_NATIVE_HISTORY_PROBE").is_none() {
        return None;
    }
    let Notification::Ui(event) = event else {
        return None;
    };
    let Action::Canvas(CanvasEvent::Key { key, .. }) = &event.action else {
        return None;
    };
    if !matches!(key.as_str(), "home" | "end" | "enter") {
        return None;
    }
    let scene = manager
        .live
        .get(id)
        .and_then(|instance| instance.views.get(panel?));
    let facts = serde_json::json!({
        "stage": "worker_admission", "event": event, "panel": panel,
        "manager_revision": scene.map(|scene| scene.revision),
        "validation": scene.map(|scene| scene.validate_event(event).map_err(|error| format!("{error:#}"))),
        "processes": manager.live.get(id).map(|instance| instance.process_count()),
        "at_millis": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis(),
    });
    eprintln!("[git120-key-probe] {facts}");
    Some(facts)
}

/// Log the unmodified admission result before the actor's ordinary post-event poll can advance scenes.
pub(super) fn probe_key_result(
    manager: &Manager,
    id: &str,
    facts: Option<serde_json::Value>,
    result: &anyhow::Result<()>,
) {
    let Some(mut facts) = facts else { return };
    facts["stage"] = "worker_result".into();
    facts["result"] = serde_json::json!(result.as_ref().map_err(|error| format!("{error:#}")));
    facts["processes"] = serde_json::json!(
        manager
            .live
            .get(id)
            .map(|instance| instance.process_count())
    );
    facts["at_millis"] = serde_json::json!(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    eprintln!("[git120-key-probe] {facts}");
}

/// First run the complete short scenario in one combination before spending eight native startups.
#[test]
#[ignore = "requires Windows desktop/D3D11, NANOBUG_GIT_CANDIDATE and NANOBUG_GIT_NATIVE_OUTPUT"]
fn git_native_workbench_smoke() {
    assert!(
        std::env::var_os("NANOBUG_GIT_NATIVE_HISTORY_ONLY").is_none(),
        "full workbench acceptance refuses the shortened history-only workflow"
    );
    matrix::run(Case::SMOKE).unwrap();
}

/// A separate diagnostic receipt never counts a shortened history run as full workbench acceptance.
#[test]
#[ignore = "requires Windows desktop, candidate/output and NANOBUG_GIT_NATIVE_HISTORY_ONLY"]
fn git_native_history_keyboard_probe() {
    assert!(std::env::var_os("NANOBUG_GIT_NATIVE_HISTORY_ONLY").is_some());
    matrix::run(Case::SMOKE).unwrap();
}

/// One native startup samples independent current-input operations across real guest timer phases.
#[test]
#[ignore = "requires Windows desktop, candidate/output, HISTORY_ONLY and HISTORY_PHASES diagnostic flags"]
fn git_native_history_keyboard_phase_probe() {
    assert!(std::env::var_os("NANOBUG_GIT_NATIVE_HISTORY_ONLY").is_some());
    assert!(std::env::var_os("NANOBUG_GIT_NATIVE_HISTORY_PHASES").is_some());
    matrix::run(Case::SMOKE).unwrap();
}

/// Every case receives a separate Windows message loop, locale, renderer, workspace and profile.
#[test]
#[ignore = "requires Windows desktop/D3D11, NANOBUG_GIT_CANDIDATE and NANOBUG_GIT_NATIVE_OUTPUT"]
fn git_native_visual_matrix() {
    assert!(
        std::env::var_os("NANOBUG_GIT_NATIVE_HISTORY_ONLY").is_none(),
        "full visual acceptance refuses the shortened history-only workflow"
    );
    for case in Case::ALL {
        matrix::run(case).unwrap();
        eprintln!("native matrix case passed: {}", case.name);
    }
}

/// Ordinary libtest creates a genuine WindowsPlatform here; the test-support feature only opens observations.
#[test]
#[ignore = "spawn through the native parent tests with parent-owned private roots"]
fn git_native_matrix_child() {
    let case = Case::from_environment().unwrap();
    let fixture = Rc::new(Fixture::prepare(case).unwrap());
    let result = Rc::new(RefCell::new(None::<Result<(), String>>));
    let completed = result.clone();
    rust_i18n::set_locale(case.locale);
    gpui_kit::application()
        .with_assets(AppAssets)
        // The child releases its strong handles before explicitly quitting the last native window.
        .with_quit_mode(gpui_kit::QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            typography::init(cx);
            apply_theme(builtin_theme(case.dark), cx);
            cx.set_reduce_motion(true);
            extensions::init(cx);
            app::shortcuts::bootstrap::configure(cx, fixture.profile.join("shortcuts.json"))
                .unwrap();
            ui::controls::init(cx);
            cx.activate(true);
            let harness = open(case, fixture, cx).unwrap();
            cx.spawn(async move |cx| {
                lifecycle::run(harness, completed, cx).await;
            })
            .detach();
        });
    result
        .borrow_mut()
        .take()
        .expect("native scenario did not report an outcome")
        .unwrap();
}

/// Reuse the normal editor constructor and panel observers, replacing only cfg(test)'s recorded transport.
fn open(case: Case, fixture: Rc<Fixture>, cx: &mut App) -> anyhow::Result<Harness> {
    let mut session = SessionState::load_in_profile(&fixture.workspace, &fixture.profile);
    session.workspace_trusted = true;
    session.messages_visible = false;
    session.explorer_visible = false;
    session
        .plugin_panel_visibility
        .insert("git/staging".into(), true);
    session
        .plugin_panel_visibility
        .insert("git/history".into(), true);
    session.try_save()?;
    let worker = Arc::new(Worker::start_background(
        extensions::profile_runtime_root(&fixture.profile),
        extensions::environment(&fixture.workspace, cx),
        true,
    ));
    let mut options = gpui_kit::component::TitleBar::window_options();
    options.window_background = theme::window_background(cx);
    options.window_bounds = Some(WindowBounds::Windowed(Bounds::centered(
        None,
        size(px(1280.), px(900.)),
        cx,
    )));
    options.show = true;
    options.focus = true;
    options.inactive_frame_interval = None;
    let initial_file = fixture.workspace.join(fixture::REVIEW_PATH);
    let workspace = Workspace::open(&fixture.workspace)?;
    let actor = worker.clone();
    let (main, app) = gpui_kit::open_window(options, cx, move |window, cx| {
        window.set_window_title("Nanobug — Git native acceptance");
        let app = cx.new(|cx| {
            EditorApp::new_with_session(workspace, Some(initial_file), Some(session), window, cx)
        });
        // Before any viewer mounts, swap the recording adapter for the production mailbox.
        // Every declaration, view, document request, process and timer thereafter belongs to this actor.
        app.read(cx).extensions.clone().update(cx, |owner, cx| {
            owner.worker = actor;
            owner.poll(cx);
        });
        app
    })?;
    Ok(Harness::new(case, fixture, main, app, worker))
}
