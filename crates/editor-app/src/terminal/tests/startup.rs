//! First-launch layout is checked through the normal editor and a real owned Shell.

use super::*;

/// A fresh prompt starts in the first row and cannot create scrollback or an empty scrollbar.
#[gpui::test]
fn fresh_shell_starts_on_first_row_without_scrollbar(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let directory = tempfile::tempdir().unwrap();
    let (app, visual) = super::restore::open(cx, Workspace::open(directory.path()).unwrap());
    let toggle = visual.debug_bounds("terminal-toggle").unwrap();
    visual.simulate_click(toggle.center(), Default::default());
    wait(visual, &app, |visual| {
        painted(&app, visual).contains(if cfg!(windows) { "PS" } else { "$" })
    });
    let (first_ink_y, cell_height, history, scroll) = visual.update(|_, cx| {
        let terminal = app.read(cx).terminal.read(cx);
        let drawing = &terminal.grid.as_ref().unwrap().read(cx).drawing;
        let first_ink_y = drawing
            .paint
            .iter()
            .filter_map(|paint| match paint {
                GridPaint::Text { y, text, .. } if !text.trim().is_empty() => Some(*y),
                _ => None,
            })
            .reduce(f32::min)
            .expect("the real Shell painted its prompt");
        (
            first_ink_y,
            terminal.cell_height,
            terminal.sessions[0].engine.history(),
            drawing.scroll,
        )
    });
    assert!(
        first_ink_y < cell_height && history == 0 && scroll.is_none(),
        "fresh prompt must occupy the first row without a scroll rail: first_ink_y={first_ink_y}, cell_height={cell_height}, history={history}, scroll={scroll:?}"
    );
}

/// The real Windows frame must show a fresh Shell in row one without a scrollbar.
#[test]
#[ignore = "opens a native window and Shell; set NANOBUG_TERMINAL_QA_ROOT and ME_EDITOR_PROFILE_HOME"]
fn native_first_prompt_frame() {
    let root =
        PathBuf::from(std::env::var_os("NANOBUG_TERMINAL_QA_ROOT").expect("explicit QA root"));
    assert!(root.is_absolute() && root.starts_with(std::env::temp_dir()));
    assert_eq!(
        std::env::var_os("ME_EDITOR_PROFILE_HOME").map(PathBuf::from),
        Some(root.join("profile"))
    );
    std::fs::create_dir_all(root.join("workspace")).unwrap();
    let workspace = Workspace::open(root.join("workspace")).unwrap();
    let mut state = SessionState::for_workspace(workspace.root());
    state.workspace_trusted = true;
    state.native_terminal_visible = false;
    assert!(
        !state.file_path().unwrap().exists(),
        "use a fresh QA profile"
    );
    state.try_save().unwrap();
    let outcome = Rc::new(RefCell::new(None));
    let capture = outcome.clone();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .with_quit_mode(gpui_kit::QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            typography::init(cx);
            apply_theme(builtin_theme(false), cx);
            let slot = Rc::new(RefCell::new(None));
            let view_slot = slot.clone();
            let window = cx.open_window(
                gpui_kit::WindowOptions {
                    window_bounds: Some(gpui_kit::WindowBounds::Windowed(Bounds::new(
                        point(px(100.), px(100.)), size(px(1100.), px(800.)),
                    ))),
                    ..Default::default()
                },
                move |window, cx| {
                    let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
                    *view_slot.borrow_mut() = Some(app.clone());
                    window.set_window_title("Nanobug terminal startup regression");
                    cx.new(|cx| Root::new(app, window, cx))
                },
            ).unwrap();
            let app = slot.borrow_mut().take().unwrap();
            cx.spawn(async move |cx| {
                // Passive layout restoration deliberately starts no Shell. Use the same explicit
                // open action as the footer, after the initial dock has completed its first frame.
                cx.background_executor().timer(Duration::from_millis(250)).await;
                window.update(cx, |_, window, cx| {
                    app.update(cx, |app, cx| app.toggle_terminal(window, cx));
                }).unwrap();
                let deadline = Instant::now() + Duration::from_secs(10);
                let result = loop {
                    cx.background_executor().timer(Duration::from_millis(100)).await;
                    let observed = window.update(cx, |_, window, cx| -> anyhow::Result<bool> {
                        let terminal = app.read(cx).terminal.read(cx);
                        let Some(grid) = &terminal.grid else { return Ok(false); };
                        let drawing = &grid.read(cx).drawing;
                        let text: String = drawing.paint.iter().filter_map(|paint| match paint {
                            GridPaint::Text { text, .. } => Some(text.as_str()), _ => None,
                        }).collect();
                        if !text.contains(if cfg!(windows) { "PS" } else { "$" }) { return Ok(false); }
                        let first_y = drawing.paint.iter().filter_map(|paint| match paint {
                            GridPaint::Text { y, text, .. } if !text.trim().is_empty() => Some(*y), _ => None,
                        }).reduce(f32::min).unwrap();
                        let history = terminal.sessions[0].engine.history();
                        anyhow::ensure!(first_y < terminal.cell_height && history == 0 && drawing.scroll.is_none(), "native startup: first_y={first_y}, history={history}, scroll={:?}", drawing.scroll);
                        window.render_to_image()?.save(root.join("first-prompt.png"))?;
                        std::fs::write(root.join("first-prompt.json"), serde_json::to_vec_pretty(&serde_json::json!({ "first_ink_y": first_y, "cell_height": terminal.cell_height, "history": history, "scrollbar": drawing.scroll.is_some() }))?)?;
                        Ok(true)
                    }).and_then(|value| value);
                    match observed {
                        Ok(true) => break Ok(()),
                        Err(error) => break Err(error),
                        Ok(false) if Instant::now() >= deadline => break Err(anyhow::anyhow!("native Shell startup timed out")),
                        Ok(false) => {}
                    }
                };
                // This test owns the Shell; obtain its cleanup receipt before tearing down GPUI.
                let cleanup = window.update(cx, |_, _, cx| {
                    app.read(cx).terminal.read(cx).supervisor.shutdown()
                }).unwrap().and_then(|receipt| receipt.recv_timeout(Duration::from_secs(5)).map_err(Into::into));
                *capture.borrow_mut() = Some(result.and(cleanup));
                window.update(cx, |_, window, _| window.remove_window()).unwrap();
                cx.background_executor().timer(Duration::from_millis(100)).await;
                let _ = cx.update(|cx| cx.quit());
            }).detach();
        });
    outcome
        .borrow_mut()
        .take()
        .expect("native first frame completed")
        .unwrap();
}
