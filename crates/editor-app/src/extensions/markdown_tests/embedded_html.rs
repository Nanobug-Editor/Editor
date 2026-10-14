//! Author HTML enters through a real guest Package and the existing native document/resource seam.

use super::*;
use harness::NativeMarkdown;

/// The README logo uses author alignment, intrinsic alt semantics and its declared clickable area.
#[gpui::test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn delivered_markdown_html_image_dimensions_match_pixels_and_keep_navigation(
    cx: &mut TestAppContext,
) {
    let original = "<p align=\"center\">\n  <a href=\"next.md\"><img src=\"logo.png\" alt=\"Nanobug 图标\" width=\"128\" height=\"128\" /></a>\n</p>\n";
    let (mut fixture, ui) =
        NativeMarkdown::mount(cx, &[("notes.md", original), ("next.md", "# 目标\n")]);
    std::fs::write(
        fixture.directory.path().join("logo.png"),
        image_preview::png(1024, 1024),
    )
    .unwrap();
    fixture.open("notes.md", ui);
    image_preview::complete(&mut fixture, ui);
    let mut image_id = String::new();
    fixture.manager.live["markdown"].views["preview"]
        .root
        .visit(&mut |node| {
            if matches!(node.kind, protocol::ui::Kind::Image { .. }) {
                image_id = node.id.clone();
            }
        });
    let image_selector = Box::leak(format!("plugin-image-pixels-{image_id}").into_boxed_str());
    let block_selector = Box::leak(format!("plugin-ui-{image_id}").into_boxed_str());
    let image = ui.debug_bounds(image_selector).unwrap();
    assert_eq!(image.size, size(px(128.), px(128.)));
    let preview = ui.debug_bounds("plugin-ui-preview-body").unwrap();
    assert!(
        (image.center().x - preview.center().x).abs() < px(1.),
        "README image must be centered in the preview: {image:?}, {preview:?}"
    );
    let block = ui.debug_bounds(block_selector).unwrap();
    assert_eq!(
        block.size.height, image.size.height,
        "loaded alt text is not a caption"
    );
    ui.simulate_click(image.center(), Default::default());
    ui.run_until_parked();
    fixture.settle(ui);
    assert_eq!(
        ui.update(|_, cx| fixture.app.read(cx).active_path.clone()),
        Some(
            fixture
                .directory
                .path()
                .join("next.md")
                .canonicalize()
                .unwrap()
        )
    );
    assert_eq!(
        std::fs::read_to_string(fixture.directory.path().join("notes.md")).unwrap(),
        original
    );
    assert!(ui.opened_url().is_none());
}

/// Check visible native HTML, controlled image pixels and source authority in both editor themes.
#[gpui::test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn delivered_markdown_embedded_html_renders_and_tracks_unsaved_source(cx: &mut TestAppContext) {
    let original = "<h2>HTML 中文标题</h2>\n\n正文 <u>下划线</u><br>换行 <strong>粗体</strong>\n\n<table><tr><td><code>表格值</code></td></tr></table>\n\n<img src='local.png' alt='中文图'>\n\n<script><img src='hidden.png'></script>\n";
    let (mut fixture, ui) = NativeMarkdown::mount(cx, &[("notes.md", original)]);
    std::fs::write(
        fixture.directory.path().join("local.png"),
        image_preview::png(48, 32),
    )
    .unwrap();
    fixture.open("notes.md", ui);
    image_preview::complete(&mut fixture, ui);
    let document = &fixture.manager.live["markdown"].views["preview"];
    assert!(document.validate().is_ok());
    let serialized = serde_json::to_string(document).unwrap();
    assert!(serialized.contains("<h2>HTML 中文标题</h2>"));
    assert!(serialized.contains("<u>下划线</u>"));
    assert!(serialized.contains("<code>表格值</code>"));
    assert!(!serialized.contains("hidden.png"));
    assert_eq!(fixture.manager.image_resources().len(), 1);
    let selector = "plugin-ui-b-0-html-embedded-html";
    for dark in [false, true] {
        ui.update(|window, cx| {
            apply_theme(builtin_theme(dark), cx);
            window.draw(cx).clear(cx);
        });
        let title = ui.debug_bounds(selector).expect("native HTML heading");
        assert!(title.size.width > px(20.) && title.size.height > px(15.));
    }
    fixture.focus_editor(ui);
    ui.simulate_keystrokes("ctrl-a");
    ui.simulate_input("前 <em>未保存中文</em> 后");
    ui.run_until_parked();
    fixture.settle(ui);
    let updated =
        serde_json::to_string(&fixture.manager.live["markdown"].views["preview"]).unwrap();
    assert!(updated.contains("<em>未保存中文</em>"));
    assert_eq!(
        std::fs::read_to_string(fixture.directory.path().join("notes.md")).unwrap(),
        original
    );
    assert!(ui.opened_url().is_none());
}

/// HTML links activate through a real native pointer gesture, then use normal editor navigation.
#[gpui::test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn delivered_markdown_embedded_html_link_opens_native_document(cx: &mut TestAppContext) {
    let original = "<a href='next.md'>打开中文</a>\n";
    let (mut fixture, ui) =
        NativeMarkdown::mount(cx, &[("notes.md", original), ("next.md", "# 目标\n")]);
    fixture.open("notes.md", ui);
    assert!(ui.opened_url().is_none());
    let bounds = ui
        .debug_bounds("plugin-ui-b-0-paragraph-embedded-html")
        .unwrap();
    ui.simulate_click(
        point(bounds.left() + px(12.), bounds.center().y),
        Default::default(),
    );
    ui.run_until_parked();
    fixture.settle(ui);
    assert_eq!(
        ui.update(|_, cx| fixture.app.read(cx).active_path.clone()),
        Some(
            fixture
                .directory
                .path()
                .join("next.md")
                .canonicalize()
                .unwrap()
        )
    );
    assert_eq!(
        ui.update(|_, cx| fixture.app.read(cx).editor.read(cx).text().to_string()),
        "# 目标\n"
    );
    assert!(ui.opened_url().is_none());
}
