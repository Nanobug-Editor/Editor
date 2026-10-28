//! Startup publication races exercise the actual guest and native wheel through public notifications.
use super::*;
use harness::NativeMarkdown;

/// A source-local toolbar must survive its body's nested native-editor render and later publication.
/// Exercise real package controls and tab withdrawal so deferred frame routing cannot revive old UI.
#[gpui::test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn delivered_markdown_first_open_keeps_toolbar_and_native_body_live(cx: &mut TestAppContext) {
    let original = "# Nanobug\n\n中文正文\n";
    let (mut fixture, ui) = NativeMarkdown::mount(
        cx,
        &[("README.md", original), ("plain.txt", "ordinary document")],
    );
    fixture.open("README.md", ui);
    assert!(ui.debug_bounds("editor-source-toolbar").is_some());
    assert!(ui.debug_bounds("plugin-ui-preview-root").is_some());
    fixture.focus_editor(ui);
    ui.simulate_keystrokes("ctrl-a");
    fixture.click("plugin-ui-format-bold", ui);
    let edited = ui.update(|_, cx| fixture.app.read(cx).editor.read(cx).text().to_string());
    assert!(
        edited.contains("**"),
        "the toolbar must still edit the native document"
    );
    fixture.focus_editor(ui);
    ui.simulate_keystrokes("ctrl-z");
    fixture.settle(ui);
    assert_eq!(
        ui.update(|_, cx| fixture.app.read(cx).editor.read(cx).text().to_string()),
        original
    );
    fixture.open("plain.txt", ui);
    assert!(ui.debug_bounds("editor-source-toolbar").is_none());
    assert!(ui.debug_bounds("plugin-ui-preview-root").is_none());
    fixture.open("README.md", ui);
    assert!(ui.debug_bounds("editor-source-toolbar").is_some());
    assert_eq!(
        ui.update(|_, cx| fixture.app.read(cx).editor.read(cx).text().to_string()),
        original
    );
}

/// Manual preview input during first publication must survive delayed source layout and locate replies.
/// Drive the real guest/Manager one publication at a time so startup cannot settle before the wheel.
#[gpui::test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn delivered_markdown_first_preview_scroll_does_not_return_to_top(cx: &mut TestAppContext) {
    use super::super::composable_tests::{publish, pump};
    let source = (0..100)
        .map(|index| format!("段落 {index:03}：首次打开后的预览滚动应保持当前位置。\n\n"))
        .collect::<String>();
    let (mut fixture, ui) = NativeMarkdown::mount(cx, &[("notes.md", &source)]);
    let path = fixture.directory.path().join("notes.md");
    ui.update(|window, cx| {
        fixture
            .app
            .update(cx, |app, cx| app.open_file(path, window, cx))
    });
    ui.run_until_parked();
    for _ in 0..5 {
        pump(&mut fixture.manager, &fixture.app, ui);
        fixture.manager.poll();
        publish(
            &mut fixture.manager,
            &mut fixture.renderer,
            &fixture.app,
            ui,
        );
        if ui.debug_bounds("plugin-ui-preview-scroll").is_some() {
            break;
        }
    }
    let preview = ui
        .debug_bounds("plugin-ui-preview-scroll")
        .expect("first visible preview");
    ui.simulate_event(gpui_kit::ScrollEvent {
        // Wheel and trackpad fixtures retain pointer-device semantics.
        source: gpui_kit::ScrollSource::Pointer,
        position: preview.center(),
        delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-900.))),
        ..Default::default()
    });
    ui.run_until_parked();
    let scrolled = ui.debug_bounds("plugin-ui-b-0-paragraph").unwrap();
    assert!(
        scrolled.bottom() < preview.top(),
        "wheel leaves the document beginning"
    );
    // Deliver the manual viewport but keep its reverse locate queued behind startup configuration.
    pump(&mut fixture.manager, &fixture.app, ui);
    fixture.manager.poll();
    // Native startup can finish publishing its locale after the first preview is already interactive.
    // Enter through the public manager notification instead of changing the guest's private driver.
    fixture
        .manager
        .event(
            "markdown",
            None,
            protocol::api::Notification::Theme(protocol::Environment {
                locale: "zh-CN".into(),
                workspace: fixture.directory.path().display().to_string(),
                ..Default::default()
            }),
        )
        .unwrap();
    publish(
        &mut fixture.manager,
        &mut fixture.renderer,
        &fixture.app,
        ui,
    );
    for _ in 0..8 {
        fixture.settle(ui);
    }
    let settled = ui.debug_bounds("plugin-ui-b-0-paragraph").unwrap();
    assert!(
        settled.bottom() < preview.top() - px(400.),
        "delayed startup synchronization must not undo manual preview scrolling: before={scrolled:?}, after={settled:?}, viewport={preview:?}"
    );
    assert_eq!(
        ui.update(|_, cx| fixture.app.read(cx).editor.read(cx).text().to_string()),
        source
    );
}
