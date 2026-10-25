//! Explicit Windows/D3D acceptance captures real control painting under isolated workspace profiles.

use super::DEFAULT_ORDER;
use crate::{AppAssets, EditorApp, SessionState, theme, typography};
use anyhow::{Context as _, ensure};
use editor_core::Workspace;
use gpui_base::test_support::snapshots;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, AsyncApp, Bounds, ElementId, Entity, KeyDownEvent,
    KeyUpEvent, Keystroke, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    PlatformInput, Point, Role, WindowBounds, point, px, size,
};
use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    time::{Duration, Instant},
};

mod messages;
mod surface;

/// Run explicitly on the Windows desktop; capture destination is required so evidence is retained.
#[test]
#[ignore = "opens private Windows/D3D windows; requires NANOBUG_WINDOW_TOOLBAR_OUTPUT; run with --test-threads=1"]
fn window_toolbar_native_windows_acceptance() {
    let output = std::env::var_os("NANOBUG_WINDOW_TOOLBAR_OUTPUT")
        .map(std::path::PathBuf::from)
        .expect("provide NANOBUG_WINDOW_TOOLBAR_OUTPUT for native evidence");
    std::fs::create_dir_all(&output).unwrap();
    let log = std::fs::File::create(output.join("native-process.log")).unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "tests::window_toolbar::native::window_toolbar_native_windows_child",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(
                status.success(),
                "native child failed; see {}",
                output.join("native-process.log").display()
            );
            return;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "native child exceeded 60 seconds; see {}",
                output.join("native-process.log").display()
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// A bounded parent owns this child's process; the child owns the isolated Windows message loop.
#[test]
#[ignore = "spawn only through window_toolbar_native_windows_acceptance"]
fn window_toolbar_native_windows_child() {
    let output = std::env::var_os("NANOBUG_WINDOW_TOOLBAR_OUTPUT")
        .map(std::path::PathBuf::from)
        .expect("native parent must provide its evidence directory");
    let profile = tempfile::tempdir().unwrap();
    let result = Rc::new(RefCell::new(None::<Result<(), String>>));
    let completed = result.clone();
    gpui_kit::application()
        .with_assets(AppAssets)
        .with_quit_mode(gpui_kit::QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            typography::init(cx);
            editor_ui::init(cx);
            crate::app::shortcuts::bootstrap::configure(cx, profile.path().join("shortcuts.json"))
                .unwrap();
            cx.set_reduce_motion(true);
            cx.activate(true);
            cx.spawn(async move |cx| {
                let observed = exercise(profile.path(), &output, cx).await;
                *completed.borrow_mut() = Some(observed.map_err(|error| format!("{error:#}")));
                cx.update(|cx| cx.quit());
                drop(profile);
            })
            .detach();
        });
    result
        .borrow_mut()
        .take()
        .expect("native acceptance did not finish")
        .unwrap();
}

/// Each palette/scale case starts from a fresh workspace and then reopens its saved private record.
async fn exercise(profile: &Path, output: &Path, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let mut observations = Vec::new();
    for (dark, percent, label) in [(false, 100, "light-100"), (true, 150, "dark-150")] {
        let workspace = tempfile::tempdir()?;
        cx.update(|cx| theme::apply_theme(theme::builtin_theme(dark), cx));
        let (handle, app) = cx.update(|cx| open(workspace.path(), profile, cx))?;
        frame(handle, cx).await?;
        handle
            .update(cx, |_, window, _| surface::DpiRequest::new(window, percent))??
            .apply();
        frame(handle, cx).await?;
        let actual_scale = handle.update(cx, |_, window, _| window.scale_factor())?;
        ensure!(
            (actual_scale - percent as f32 / 100.).abs() < 0.001,
            "native DPI notification was not applied"
        );
        ensure!(
            visible_order(handle, cx)? == DEFAULT_ORDER,
            "incorrect default order"
        );
        capture(handle, &output.join(format!("{label}-default.png")), cx)?;
        messages::exercise(handle, &app, output, label, dark, cx).await?;

        // A quick press/release activates the original button and a second click hides it again.
        let messages = button_bounds(handle, "host-messages-toggle", cx)?.center();
        dispatch(handle, down(messages), cx)?;
        dispatch(handle, up(messages), cx)?;
        frame(handle, cx).await?;
        ensure!(
            messages_visible(handle, &app, cx)?,
            "short click did not open messages"
        );
        dispatch(handle, down(messages), cx)?;
        dispatch(handle, up(messages), cx)?;
        frame(handle, cx).await?;
        ensure!(
            !messages_visible(handle, &app, cx)?,
            "second short click did not close messages"
        );

        let first = button_bounds(handle, "explorer-panel-toggle", cx)?;
        let target = point(first.left() + px(1.), first.center().y);
        begin_hold(handle, "host-messages-toggle", cx).await?;
        dispatch(handle, moved(target), cx)?;
        frame(handle, cx).await?;
        capture(handle, &output.join(format!("{label}-insertion.png")), cx)?;
        dispatch(handle, up(target), cx)?;
        frame(handle, cx).await?;
        let expected = [
            "host-messages-toggle",
            "explorer-panel-toggle",
            "outline-toggle",
            "terminal-toggle",
        ];
        ensure!(
            visible_order(handle, cx)? == expected,
            "held drag did not reorder"
        );
        ensure!(
            !messages_visible(handle, &app, cx)?,
            "drag activated the message panel"
        );
        let root = handle.update(cx, |_, _, cx| app.read(cx).workspace.root().to_path_buf())?;
        ensure!(
            SessionState::load_in_profile(&root, profile).window_button_order == expected,
            "drop was not persisted to its isolated profile"
        );
        capture(handle, &output.join(format!("{label}-reordered.png")), cx)?;

        begin_hold(handle, "outline-toggle", cx).await?;
        let invalid = point(px(720.), px(350.));
        dispatch(handle, moved(invalid), cx)?;
        frame(handle, cx).await?;
        capture(handle, &output.join(format!("{label}-forbidden.png")), cx)?;
        dispatch(handle, up(invalid), cx)?;
        frame(handle, cx).await?;
        ensure!(
            visible_order(handle, cx)? == expected,
            "illegal drop changed order"
        );

        begin_hold(handle, "outline-toggle", cx).await?;
        let first = button_bounds(handle, "host-messages-toggle", cx)?;
        let cancel_target = point(first.left() + px(1.), first.center().y);
        dispatch(handle, moved(cancel_target), cx)?;
        let key = Keystroke::parse("escape")?;
        dispatch(
            handle,
            PlatformInput::KeyDown(KeyDownEvent {
                keystroke: key.clone(),
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        )?;
        dispatch(
            handle,
            PlatformInput::KeyUp(KeyUpEvent { keystroke: key }),
            cx,
        )?;
        dispatch(handle, up(cancel_target), cx)?;
        frame(handle, cx).await?;
        ensure!(
            visible_order(handle, cx)? == expected,
            "Escape failed to cancel drag"
        );
        ensure!(
            !handle.update(cx, |_, _, cx| app.read(cx).session_state.outline_visible)?,
            "cancelled drag activated Outline"
        );
        close(handle, &app, cx).await?;
        drop(app);

        let (reopened, restored) = cx.update(|cx| open(&root, profile, cx))?;
        frame(reopened, cx).await?;
        reopened
            .update(cx, |_, window, _| surface::DpiRequest::new(window, percent))??
            .apply();
        frame(reopened, cx).await?;
        let reopened_scale = reopened.update(cx, |_, window, _| window.scale_factor())?;
        ensure!(
            (reopened_scale - percent as f32 / 100.).abs() < 0.001,
            "reopened native DPI was not applied"
        );
        ensure!(
            visible_order(reopened, cx)? == expected,
            "reopened native window lost saved order"
        );
        ensure!(
            !messages_visible(reopened, &restored, cx)?,
            "reopen changed message visibility"
        );
        capture(reopened, &output.join(format!("{label}-reopened.png")), cx)?;
        observations.push(serde_json::json!({
            "theme": label, "scale_percent": percent, "actual_scale_factor": actual_scale,
            "reopened_scale_factor": reopened_scale, "default_order": DEFAULT_ORDER,
            "saved_and_reopened_order": expected, "short_click_verified": true,
            "long_press_millis": 450, "invalid_drop_unchanged": true, "escape_cancelled": true,
            "drag_did_not_activate_panel": true,
            "severity_colors_verified": ["blue: info", "orange: warning", "red: error"],
        }));
        close(reopened, &restored, cx).await?;
        drop(restored);
    }
    std::fs::write(
        output.join("evidence.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "platform": "WindowsPlatform / D3D11", "input": "ordinary Window::dispatch_event",
            "isolated_profile": true, "observations": observations, "passed": true,
        }))?,
    )?;
    Ok(())
}

/// Open an ordinary production editor using only this test's explicit workspace and profile.
fn open(
    root: &Path,
    profile: &Path,
    cx: &mut App,
) -> anyhow::Result<(AnyWindowHandle, Entity<EditorApp>)> {
    let workspace = Workspace::open(root)?;
    let session = SessionState::load_in_profile(workspace.root(), profile);
    let mut options = editor_ui::window::TitleBar::window_options();
    options.window_background = theme::window_background(cx);
    options.window_bounds = Some(WindowBounds::Windowed(Bounds::new(
        point(px(100.), px(80.)),
        size(px(1100.), px(800.)),
    )));
    options.show = true;
    options.focus = true;
    gpui_kit::open_window(options, cx, |window, cx| {
        window.set_window_title("Nanobug — window toolbar acceptance");
        cx.new(|cx| EditorApp::new_with_session(workspace, None, Some(session), window, cx))
    })
}

/// Await the original project cleanup task before destroying this owned native window.
async fn close(
    handle: AnyWindowHandle,
    app: &Entity<EditorApp>,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let task = handle
        .update(cx, |_, _, cx| {
            app.update(cx, |app, cx| app.prepare_project_shutdown(cx))
        })?
        .map_err(anyhow::Error::msg)?;
    task.await.map_err(anyhow::Error::msg)?;
    handle.update(cx, |_, window, _| window.remove_window())?;
    cx.background_executor()
        .timer(Duration::from_millis(80))
        .await;
    Ok(())
}

/// Yield to the Windows message loop, then draw the actual native surface for fresh observations.
async fn frame(handle: AnyWindowHandle, cx: &mut AsyncApp) -> anyhow::Result<()> {
    cx.background_executor()
        .timer(Duration::from_millis(100))
        .await;
    handle.update(cx, |_, window, cx| window.draw(cx).clear(cx))?;
    Ok(())
}

/// Read the original Base button identity; message keeps its established inner control ID.
fn button_bounds(
    handle: AnyWindowHandle,
    id: &str,
    cx: &mut AsyncApp,
) -> anyhow::Result<Bounds<Pixels>> {
    let button_id = if id == "host-messages-toggle" {
        "toggle-host-messages"
    } else {
        id
    };
    handle.update(cx, |_, window, _| {
        snapshots(window)
            .into_iter()
            .find(|node| {
                node.role() == Some(Role::Button)
                    && node.visible()
                    && node.path().iter().any(
                        |part| matches!(part, ElementId::Name(name) if name.as_ref() == button_id),
                    )
            })
            .map(|node| node.bounds())
            .with_context(|| format!("native button {id} missing"))
    })?
}

/// Sort observed native positions, independent of the host's saved ordering state.
fn visible_order(handle: AnyWindowHandle, cx: &mut AsyncApp) -> anyhow::Result<Vec<&'static str>> {
    let mut positions = Vec::new();
    for id in DEFAULT_ORDER {
        positions.push((id, button_bounds(handle, id, cx)?.left()));
    }
    positions.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    Ok(positions.into_iter().map(|(id, _)| id).collect())
}

/// Visibility is queried through the same dock projection used by the actual message toggle.
fn messages_visible(
    handle: AnyWindowHandle,
    app: &Entity<EditorApp>,
    cx: &mut AsyncApp,
) -> anyhow::Result<bool> {
    handle.update(cx, |_, _, cx| app.read(cx).messages_visible(cx))
}

/// Send one platform event to this test-owned window without global OS input injection.
fn dispatch(
    handle: AnyWindowHandle,
    input: PlatformInput,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    handle.update(cx, |_, window, cx| {
        // An ordinary OS click activates its recipient before delivering the press. Direct
        // dispatch owns no global pointer, so reproduce only that owned-window activation.
        if matches!(&input, PlatformInput::MouseDown(_)) {
            window.activate_window();
        }
        window.dispatch_event(input, cx);
    })?;
    Ok(())
}

/// Allow the real background timer to arm the held gesture while the native event loop runs.
async fn begin_hold(handle: AnyWindowHandle, id: &str, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let position = button_bounds(handle, id, cx)?.center();
    dispatch(handle, down(position), cx)?;
    cx.background_executor()
        .timer(Duration::from_millis(450))
        .await;
    dispatch(handle, moved(position + point(px(8.), px(0.))), cx)?;
    frame(handle, cx).await?;
    ensure!(
        handle.update(cx, |_, _, cx| cx.has_active_drag())?,
        "held movement did not start native drag"
    );
    Ok(())
}

/// Match the ordinary left-button press payload used by the Windows input adapter.
fn down(position: Point<Pixels>) -> PlatformInput {
    PlatformInput::MouseDown(MouseDownEvent {
        position,
        button: MouseButton::Left,
        modifiers: Default::default(),
        click_count: 1,
        first_mouse: false,
    })
}

/// The held movement flag retains capture while traversing the content area.
fn moved(position: Point<Pixels>) -> PlatformInput {
    PlatformInput::MouseMove(MouseMoveEvent {
        position,
        pressed_button: Some(MouseButton::Left),
        modifiers: Default::default(),
    })
}

/// Release the same gesture at the current destination instead of simulating a second click.
fn up(position: Point<Pixels>) -> PlatformInput {
    PlatformInput::MouseUp(MouseUpEvent {
        position,
        button: MouseButton::Left,
        modifiers: Default::default(),
        click_count: 1,
    })
}

/// Native readback captures the currently painted D3D render target with its actual DPI metadata.
fn capture(handle: AnyWindowHandle, path: &Path, cx: &mut AsyncApp) -> anyhow::Result<()> {
    handle.update(cx, |_, window, _| surface::capture(window, path))?
}
