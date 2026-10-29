//! Explicit Windows/D3D acceptance of filtering, compact layout and native input composition.

use super::*;
use anyhow::{Context as _, ensure};
use gpui_kit::{AnyWindowHandle, AsyncApp, EntityInputHandler as _, MouseMoveEvent};
use std::{cell::RefCell, path::Path, time::Instant};

/// Bound the native child and keep screenshots outside the repository in the requested evidence folder.
#[test]
#[ignore = "opens private Windows/D3D windows; set NANOBUG_EXPLORER_FILTER_OUTPUT; run with --test-threads=1"]
fn explorer_filter_native_windows_acceptance() {
    let output = PathBuf::from(
        std::env::var_os("NANOBUG_EXPLORER_FILTER_OUTPUT")
            .expect("set NANOBUG_EXPLORER_FILTER_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let log = std::fs::File::create(output.join("native.log")).unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "explorer::filter::native_tests::explorer_filter_native_windows_child",
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
                "native filter acceptance failed: {}",
                output.join("native.log").display()
            );
            return;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("native filter acceptance exceeded 60 seconds");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Run the production native event loop in a disposable process with isolated session data.
#[test]
#[ignore = "spawn only through explorer_filter_native_windows_acceptance"]
fn explorer_filter_native_windows_child() {
    let output = PathBuf::from(std::env::var_os("NANOBUG_EXPLORER_FILTER_OUTPUT").unwrap());
    let profile = tempfile::tempdir().unwrap();
    let result = Rc::new(RefCell::new(None));
    let completed = result.clone();
    gpui_kit::application()
        .with_assets(AppAssets)
        .with_quit_mode(gpui_kit::QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            typography::init(cx);
            editor_ui::init(cx);
            app::shortcuts::bootstrap::configure(cx, profile.path().join("shortcuts.json"))
                .unwrap();
            cx.set_reduce_motion(true);
            cx.activate(true);
            cx.spawn(async move |cx| {
                *completed.borrow_mut() = Some(
                    exercise(&output, cx)
                        .await
                        .map_err(|error| format!("{error:#}")),
                );
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
    result
        .borrow_mut()
        .take()
        .expect("native filter scenario did not finish")
        .unwrap();
}

/// Exercise real platform dispatch, D3D layout and the input's public IME handler in both palettes.
async fn exercise(output: &Path, cx: &mut AsyncApp) -> anyhow::Result<()> {
    for dark in [false, true] {
        let directory = tempfile::tempdir()?;
        std::fs::create_dir_all(directory.path().join("src/deep"))?;
        let path = directory.path().join("src/deep/文件Main.RS");
        std::fs::write(&path, "selected file\n")?;
        let initial = directory.path().join("unrelated.txt");
        std::fs::write(&initial, "other\n")?;
        let workspace = Workspace::open(directory.path())?;
        let target = workspace.root().join("src/deep/文件Main.RS");
        let mut session =
            SessionState::load_in_profile(workspace.root(), &directory.path().join("profile"));
        session.explorer_visible = true;
        session.outline_visible = false;
        session.problems_visible = false;
        session.messages_visible = false;
        let (handle, app) = cx.update(|cx| {
            apply_theme(builtin_theme(dark), cx);
            let mut options = editor_ui::window::TitleBar::window_options();
            options.window_bounds = Some(WindowBounds::Windowed(Bounds::new(
                point(px(100.), px(80.)),
                size(px(1000.), px(700.)),
            )));
            options.show = true;
            options.focus = true;
            gpui_kit::open_window(options, cx, |window, cx| {
                window.set_window_title("Nanobug — Explorer filter acceptance");
                cx.new(|cx| {
                    EditorApp::new_with_session(workspace, Some(initial), Some(session), window, cx)
                })
            })
        })?;
        frame(handle, cx).await?;
        for font_size in [14., 20.] {
            handle.update(cx, |_, window, cx| {
                typography::set_font_size(cx, font_size);
                let app = app.read(cx);
                let focus = app.editor.focus_handle(cx);
                let bounds = app
                    .tree_state
                    .read(cx)
                    .scroll_handle()
                    .0
                    .borrow()
                    .base_handle
                    .bounds();
                focus.focus(window, cx);
                window.dispatch_event(
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position: bounds.bottom_left() + point(px(20.), -px(30.)),
                        pressed_button: None,
                        modifiers: Default::default(),
                    }),
                    cx,
                );
                editor_ui::testing::press_key(window, "ctrl-f", cx);
            })?;
            frame(handle, cx).await?;
            handle.update(cx, |_, window, cx| -> anyhow::Result<()> {
                let app = app.read(cx);
                ensure!(
                    app.explorer_filter.visible,
                    "hover Ctrl+F did not show native filter"
                );
                ensure!(
                    !app.editor.read(cx).search_session().open,
                    "document Find also opened"
                );
                let input = app.explorer_filter.input.clone();
                let bounds = input.read(cx).input_bounds();
                ensure!(
                    bounds.size.height > px(0.) && bounds.size.height <= px(24.),
                    "small native input has invalid height: {bounds:?}"
                );
                ensure!(
                    input.read(cx).focus_handle(cx).is_focused(window),
                    "native input did not acquire focus"
                );
                editor_ui::testing::input_text(window, "mAiN.rS", cx);
                Ok(())
            })??;
            frame(handle, cx).await?;
            handle.update(cx, |_, _, cx| -> anyhow::Result<()> {
                let tree = app.read(cx).tree_state.read(cx);
                ensure!(
                    tree.entry(3)
                        .is_some_and(|entry| Path::new(entry.item().id.as_str()) == target),
                    "native case-insensitive result missing"
                );
                ensure!(
                    tree.entry(4).is_none(),
                    "unrelated file survived native filter"
                );
                Ok(())
            })??;
            capture(
                handle,
                &output.join(format!(
                    "{}-{font_size}.png",
                    if dark { "dark" } else { "light" }
                )),
                cx,
            )?;
            handle.update(cx, |_, window, cx| {
                editor_ui::testing::press_key(window, "ctrl-a", cx);
                let input = app.read(cx).explorer_filter.input.clone();
                input.update(cx, |input, cx| {
                    input.replace_and_mark_text_in_range(None, "文件", Some(0..2), window, cx);
                    assert!(input.marked_text_range(window, cx).is_some());
                    input.replace_text_in_range(None, "文件", window, cx);
                });
            })?;
            frame(handle, cx).await?;
            let position = handle
                .update(cx, |_, _, cx| {
                    let tree = app.read(cx).tree_state.read(cx);
                    let index = tree.index_of(&target.to_string_lossy().into_owned().into())?;
                    let scroll = tree.scroll_handle().0.borrow();
                    let bounds = scroll.base_handle.bounds();
                    let count = (0..).map_while(|index| tree.entry(index)).count();
                    let height = scroll.last_item_size?.contents.height / count as f32;
                    Some(point(
                        bounds.center().x,
                        bounds.top()
                            + height * (index as f32 + 0.5)
                            + scroll.base_handle.offset().y,
                    ))
                })?
                .context("Chinese query did not retain native file row")?;
            click(handle, position, cx)?;
            frame(handle, cx).await?;
            handle.update(cx, |_, _, cx| -> anyhow::Result<()> {
                let app = app.read(cx);
                ensure!(
                    !app.explorer_filter.visible,
                    "native file click did not hide filter"
                );
                ensure!(
                    app.active_path.as_ref() == Some(&target),
                    "native click opened the wrong file"
                );
                ensure!(
                    app.editor.read(cx).value().to_string() == "selected file\n",
                    "native click did not load the selected document"
                );
                ensure!(
                    app.explorer_filter.input.read(cx).value().is_empty(),
                    "native hide retained stale query"
                );
                Ok(())
            })??;
            capture(handle, &output.join("after-native-click.png"), cx)?;
            // The reserved compact title slot puts Search first, immediately left of Locate.
            let button = handle.update(cx, |_, _, cx| {
                let body = app
                    .read(cx)
                    .tree_state
                    .read(cx)
                    .scroll_handle()
                    .0
                    .borrow()
                    .base_handle
                    .bounds();
                point(
                    body.right() + px(4.) - px(108.),
                    body.top() - px(4.) - px(PANEL_HEADER_HEIGHT / 2.),
                )
            })?;
            click(handle, button, cx)?;
            frame(handle, cx).await?;
            handle.update(cx, |_, window, cx| -> anyhow::Result<()> {
                ensure!(
                    app.read(cx).explorer_filter.visible,
                    "native Search button did not show filter"
                );
                editor_ui::testing::press_key(window, "ctrl-f", cx);
                ensure!(
                    !app.read(cx).explorer_filter.visible,
                    "focused Ctrl+F did not hide filter"
                );
                Ok(())
            })??;
        }
        let shutdown = handle
            .update(cx, |_, _, cx| {
                app.update(cx, |app, cx| app.prepare_project_shutdown(cx))
            })?
            .map_err(anyhow::Error::msg)?;
        shutdown.await.map_err(anyhow::Error::msg)?;
        handle.update(cx, |_, window, _| window.remove_window())?;
    }
    std::fs::write(
        output.join("passed.txt"),
        "Windows/D3D; light/dark; 14/20 px; hover/focused Ctrl+F; compact native input; case-insensitive filter; IME handler composition/commit; file selection hides; Search button; normal process exit\n",
    )?;
    Ok(())
}

/// Let the native loop deliver input subscriptions and paint before checking `handle`'s frame.
async fn frame(handle: AnyWindowHandle, cx: &mut AsyncApp) -> anyhow::Result<()> {
    cx.background_executor()
        .timer(Duration::from_millis(100))
        .await;
    handle.update(cx, |_, window, cx| window.draw(cx).clear(cx))?;
    Ok(())
}

/// Dispatch one ordinary click at `position` inside the owned native window.
fn click(
    handle: AnyWindowHandle,
    position: Point<Pixels>,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    handle.update(cx, |_, window, cx| {
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position,
                pressed_button: None,
                modifiers: Default::default(),
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                position,
                button: MouseButton::Left,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.draw(cx).clear(cx);
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position,
                button: MouseButton::Left,
                modifiers: Default::default(),
                click_count: 1,
            }),
            cx,
        );
    })?;
    Ok(())
}

/// Save the actual rendered image of `handle` to `path` for visual inspection.
fn capture(handle: AnyWindowHandle, path: &Path, cx: &mut AsyncApp) -> anyhow::Result<()> {
    handle.update(cx, |_, window, _| {
        window
            .render_to_image()
            .context("native D3D readback")?
            .save(path)
            .context("save native filter screenshot")
    })?
}
