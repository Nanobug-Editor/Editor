//! Real AccessKit frames validate search focus at the ordinary docked terminal entry point.

use super::*;

/// Opening find must transfer one accessible focus owner from the docked terminal to its input.
/// The explicit isolated profile contains restored history and never launches a child Shell here.
#[test]
#[ignore = "opens a real Windows window; set NANOBUG_TERMINAL_QA_ROOT and ME_EDITOR_PROFILE_HOME"]
fn native_terminal_search_accessibility_focus() {
    let root = PathBuf::from(
        std::env::var_os("NANOBUG_TERMINAL_QA_ROOT").expect("explicit temporary profile root"),
    );
    assert!(root.is_absolute() && root.starts_with(std::env::temp_dir()));
    assert_eq!(
        std::env::var_os("ME_EDITOR_PROFILE_HOME").map(PathBuf::from),
        Some(root.join("profile"))
    );
    let dark = std::env::var("NANOBUG_TERMINAL_QA_DARK").is_ok_and(|value| value == "1");
    let suffix = if dark { "dark" } else { "light" };
    let workspace = Workspace::open(root.join("workspace")).unwrap();
    let outcome = Rc::new(RefCell::new(None));
    let capture = outcome.clone();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .with_quit_mode(gpui_kit::QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            typography::init(cx);
            apply_theme(builtin_theme(dark), cx);
            let slot = Rc::new(RefCell::new(None));
            let view_slot = slot.clone();
            let window = cx
                .open_window(
                    gpui_kit::WindowOptions {
                        window_bounds: Some(gpui_kit::WindowBounds::Windowed(Bounds::new(
                            point(px(100.), px(100.)),
                            size(px(1200.), px(800.)),
                        ))),
                        ..Default::default()
                    },
                    move |window, cx| {
                        let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
                        // Exercise the same native terminal composition in both host palettes.
                        apply_theme(builtin_theme(dark), cx);
                        *view_slot.borrow_mut() = Some(app.clone());
                        window.set_window_title("Nanobug native terminal search focus probe");
                        cx.new(|cx| Root::new(app, window, cx))
                    },
                )
                .unwrap();
            let app = slot.borrow_mut().take().unwrap();
            cx.spawn(async move |cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                // Exercise the preceding terminal focus transition too; opening find from an
                // unfocused restored view alone does not exercise the dock's tracked focus scope.
                window
                    .update(cx, |_, window, cx| {
                        let panel = app.read(cx).terminal.read(cx);
                        panel
                            .grid
                            .as_ref()
                            .unwrap()
                            .read(cx)
                            .focus_handle()
                            .focus(window, cx);
                    })
                    .unwrap();
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                window
                    .update(cx, |_, _, cx| {
                        let terminal = app.read(cx).terminal.clone();
                        terminal.update(cx, |panel, cx| panel.open_search(cx));
                    })
                    .unwrap();
                cx.background_executor()
                    .timer(Duration::from_millis(350))
                    .await;
                let observed = window
                    .update(cx, |_, window, cx| -> anyhow::Result<()> {
                        anyhow::ensure!(window.is_a11y_active(), "native UIA activation required");
                        let json = window
                            .debug_a11y_tree_json()
                            .ok_or_else(|| anyhow::anyhow!("native accessible tree missing"))?;
                        std::fs::write(root.join(format!("search-focus-{suffix}.json")), &json)?;
                        let tree: serde_json::Value = serde_json::from_str(&json)?;
                        let focus = tree["gpui_focus"]
                            .as_str()
                            .ok_or_else(|| anyhow::anyhow!("find input has no accessible focus"))?;
                        let label = t!("terminal.find").to_string();
                        anyhow::ensure!(
                            tree["nodes"][focus].to_string().contains(&label),
                            "find must own named accessible focus: {}",
                            tree["nodes"][focus]
                        );
                        anyhow::ensure!(
                            !app.read(cx).terminal.read(cx).has_running_programs(),
                            "opening find must not restart the restored Shell"
                        );
                        window
                            .render_to_image()?
                            .save(root.join(format!("search-focus-{suffix}.png")))?;
                        Ok(())
                    })
                    .and_then(|value| value);
                *capture.borrow_mut() = Some(observed);
                window
                    .update(cx, |_, window, _| window.remove_window())
                    .unwrap();
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let _ = cx.update(|cx| cx.quit());
            })
            .detach();
        });
    outcome
        .borrow_mut()
        .take()
        .expect("native search inspection completed")
        .unwrap();
}
