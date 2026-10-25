//! Explicit Windows/D3D acceptance of hover ownership on the production editor and dock.

use super::*;
use anyhow::{Context as _, ensure};
use gpui_kit::{AnyWindowHandle, AsyncApp, MouseMoveEvent};
use std::{path::Path, time::Instant};

/// Representative server documentation exercises a signature, prose, inline code, and a separator.
struct DocumentationHover;

impl gpui_base::input::HoverProvider for DocumentationHover {
    fn hover(
        &self,
        _: &gpui_base::input::Rope,
        _: usize,
        _: &mut Window,
        _: &mut App,
    ) -> gpui_kit::Task<anyhow::Result<Option<Hover>>> {
        gpui_kit::Task::ready(Ok(Some(Hover {
            contents: HoverContents::Markup(lsp_types::MarkupContent {
                kind: lsp_types::MarkupKind::Markdown,
                value: "example::editor\n\n```rust\npub struct Alpha;\n```\n\n---\n\nsize = 0, align = 0x1, no Drop\n\nShows type details for the selected symbol. Use `Alpha` to inspect its documentation.\n\n支持中文说明，代码与正文保持清晰的层次。".into(),
            }),
            range: None,
        })))
    }
}

/// Bound the native process and retain screenshots without touching an existing user session.
#[test]
#[ignore = "opens private Windows/D3D windows; set NANOBUG_HOVER_OUTPUT; run with --test-threads=1"]
fn hover_native_windows_acceptance() {
    let output = std::env::var_os("NANOBUG_HOVER_OUTPUT").expect("set NANOBUG_HOVER_OUTPUT");
    let output = std::path::PathBuf::from(output);
    std::fs::create_dir_all(&output).unwrap();
    let log = std::fs::File::create(output.join("native.log")).unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "editor::hover_hit_test::native::hover_native_windows_child",
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
                "native hover acceptance failed; see {}",
                output.join("native.log").display()
            );
            return;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("native hover acceptance exceeded 60 seconds");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Own the actual Windows message loop in a disposable process rather than TestPlatform.
#[test]
#[ignore = "spawn only through hover_native_windows_acceptance"]
fn hover_native_windows_child() {
    let output = std::path::PathBuf::from(std::env::var_os("NANOBUG_HOVER_OUTPUT").unwrap());
    let result = Rc::new(RefCell::new(None));
    let completed = result.clone();
    gpui_kit::application()
        .with_assets(AppAssets)
        .with_quit_mode(gpui_kit::QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            typography::init(cx);
            editor_ui::init(cx);
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
        .expect("native scenario did not finish")
        .unwrap();
}

/// Drive real-window input, inspect painted card bounds, and capture both live theme surfaces.
async fn exercise(output: &Path, cx: &mut AsyncApp) -> anyhow::Result<()> {
    for dark in [false, true] {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("hover.txt");
        std::fs::write(&path, "alpha beta\nsecond_word\n中文输入与滚动\n")?;
        let workspace = Workspace::open(directory.path())?;
        let mut session =
            SessionState::load_in_profile(workspace.root(), &directory.path().join("profile"));
        session.explorer_visible = true;
        session.outline_visible = false;
        session.messages_visible = false;
        session.problems_visible = false;
        let (handle, app) = cx.update(|cx| {
            theme::apply_theme(theme::builtin_theme(dark), cx);
            let mut options = editor_ui::window::TitleBar::window_options();
            options.window_bounds = Some(WindowBounds::Windowed(Bounds::new(
                point(px(100.), px(80.)),
                size(px(1000.), px(600.)),
            )));
            options.show = true;
            options.focus = true;
            gpui_kit::open_window(options, cx, |window, cx| {
                window.set_window_title("Nanobug — Hover acceptance");
                cx.new(|cx| {
                    EditorApp::new_with_session(workspace, Some(path), Some(session), window, cx)
                })
            })
        })?;
        frame(handle, Duration::from_millis(150), cx).await?;
        handle.update(cx, |_, _, cx| {
            let editor = app.read(cx).editor.clone();
            editor.update(cx, |state, cx| {
                state.lsp_mut().hover_provider = Some(Rc::new(DocumentationHover));
                let text = state.text().clone();
                let diagnostics = state.diagnostics_mut().unwrap();
                diagnostics.reset(&text);
                diagnostics.push(
                    gpui_base::input::Diagnostic::new(
                        lsp_types::Position::new(1, 0)..lsp_types::Position::new(1, 11),
                        "unresolved module, can't find module file: ../../plugin-runtime/tests/support/ui_demand.rs\nhelp: create the missing file or check the module path",
                    )
                    .with_severity(gpui_base::input::DiagnosticSeverity::Error)
                    .with_source("rust-analyzer")
                    .with_code("E0583"),
                );
                cx.notify();
            });
        })?;
        frame(handle, Duration::from_millis(100), cx).await?;
        for (label, range) in [("definition", 0..5), ("diagnostic", 11..22)] {
            let (input, symbol) = handle.update(cx, |_, _, cx| {
                let editor = app.read(cx).editor.read(cx);
                (
                    editor.input_bounds(),
                    editor.range_to_bounds(&range).unwrap().center(),
                )
            })?;
            dispatch(
                handle,
                PlatformInput::MouseMove(MouseMoveEvent {
                    position: symbol,
                    pressed_button: None,
                    modifiers: Default::default(),
                }),
                cx,
            )?;
            frame(handle, Duration::from_millis(1200), cx).await?;
            capture(
                handle,
                &output.join(format!(
                    "{}-{label}.png",
                    if dark { "dark" } else { "light" }
                )),
                cx,
            )?;
            ensure!(
                handle.update(cx, |_, _, cx| app
                    .read(cx)
                    .definition_popup_focus
                    .bounds
                    .get()
                    .is_some())?,
                "native {label} did not paint: {:?}",
                handle.update(cx, |_, _, cx| {
                    let app = app.read(cx);
                    (
                        app.editor.read(cx).hover_popover().is_some(),
                        app.pointer_hover_pending,
                        app.pointer_hover_symbol.clone(),
                        app.editor.read(cx).lsp().hover_provider.is_some(),
                    )
                })?
            );
            let splitter = point(input.left() - px(1.), symbol.y);
            dispatch(
                handle,
                PlatformInput::MouseDown(MouseDownEvent {
                    position: splitter,
                    button: MouseButton::Left,
                    modifiers: Default::default(),
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            )?;
            frame(handle, Duration::from_millis(100), cx).await?;
            ensure!(
                handle.update(cx, |_, _, cx| app
                    .read(cx)
                    .definition_popup_focus
                    .bounds
                    .get()
                    .is_none())?,
                "native splitter press retained {label}"
            );
            for position in [splitter + point(px(12.), px(0.)), symbol] {
                dispatch(
                    handle,
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position,
                        pressed_button: Some(MouseButton::Left),
                        modifiers: Default::default(),
                    }),
                    cx,
                )?;
            }
            frame(handle, Duration::from_millis(1200), cx).await?;
            handle.update(cx, |_, _, cx| -> anyhow::Result<()> {
                let editor = app.read(cx).editor.read(cx);
                ensure!(
                    editor.input_bounds().left() > input.left() + px(5.),
                    "native splitter did not resize"
                );
                ensure!(
                    editor.hover_popover().is_none() && editor.diagnostic_popover().is_none(),
                    "native resize resurrected {label}"
                );
                Ok(())
            })??;
            capture(
                handle,
                &output.join(format!(
                    "{}-{label}-drag.png",
                    if dark { "dark" } else { "light" }
                )),
                cx,
            )?;
            dispatch(
                handle,
                PlatformInput::MouseUp(MouseUpEvent {
                    position: symbol,
                    button: MouseButton::Left,
                    modifiers: Default::default(),
                    click_count: 1,
                }),
                cx,
            )?;
            frame(handle, Duration::from_millis(100), cx).await?;
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
        "Windows/D3D; production EditorApp; native Window::dispatch_event; light and dark; definition and diagnostic; real splitter resize; no popovers after delay\n",
    )?;
    Ok(())
}

/// Yield to real timers and redraw the actual D3D window before checking layout.
async fn frame(
    handle: AnyWindowHandle,
    duration: Duration,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    cx.background_executor().timer(duration).await;
    handle.update(cx, |_, window, cx| window.draw(cx).clear(cx))?;
    Ok(())
}

/// Inject one ordinary platform event into the owned native window without global mouse input.
fn dispatch(
    handle: AnyWindowHandle,
    event: PlatformInput,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    handle.update(cx, |_, window, cx| {
        window.activate_window();
        window.dispatch_event(event, cx);
    })?;
    Ok(())
}

/// Read back the visible renderer target for manual visual inspection, retaining its actual scale.
fn capture(handle: AnyWindowHandle, path: &Path, cx: &mut AsyncApp) -> anyhow::Result<()> {
    handle.update(cx, |_, window, _| {
        window
            .render_to_image()
            .context("D3D readback failed")?
            .save(path)
            .context("save native screenshot")
    })?
}
