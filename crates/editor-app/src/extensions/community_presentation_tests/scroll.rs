//! Real SDK markers follow native scrolling, using the shared document/provider publication seam.

use super::*;

/// Open the CRLF/non-BMP fixture through the normal file lifecycle before starting its providers.
fn open_scroll_fixture(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    path: &Path,
) -> String {
    // The same prefix as physical acceptance fixes both coverage byte ranges; the plain tail only
    // supplies enough real document height to make all three source rows leave the viewport.
    let source = format!(
        "// TODO 中文😀\r\n// FIXME 第二项\r\n// COVERED 中文😀\r\n// UNCOVERED 行\r\n{}",
        (1..=200)
            .map(|line| format!("可滚动正文 {line:03} 中文😀\r\n"))
            .collect::<String>()
    );
    // Native tabs retain canonical paths. Close that exact file identity before changing the
    // temporary backing file, so reopening cannot simply activate the old in-memory document.
    let native_path = path.canonicalize().unwrap();
    visual.update(|window, cx| {
        app.update(cx, |owner, cx| {
            owner.close_tab(native_path.clone(), window, cx)
        })
    });
    std::fs::write(path, &source).unwrap();
    visual
        .update(|window, cx| app.update(cx, |owner, cx| owner.open_file(native_path, window, cx)));
    visual.simulate_keystrokes("ctrl-home");
    visual.run_until_parked();
    assert_eq!(
        visual.update(|_, cx| app.read(cx).editor.read(cx).text().to_string()),
        source,
        "the ordinary file path must load the prepared source before provider calls"
    );
    source
}

/// Real native scrolling removes offscreen marker elements and restores them on their source rows.
#[gpui::test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn community_decorations_hide_offscreen_markers_and_restore_after_scroll(cx: &mut TestAppContext) {
    crate::tests::with_shortcut_editor(cx, true, Vec::new(), |visual, app, path| {
        let source = open_scroll_fixture(visual, &app, path);
        let (_root, mut manager) = providers(path.parent().unwrap(), &["bookmarks", "coverage"]);
        events(visual, &app, &mut manager);
        let bookmark = command(visual, &app, &mut manager, "bookmarks", "bookmark-line")
            .pop()
            .unwrap();
        let coverage = command(visual, &app, &mut manager, "coverage", "scan-coverage")
            .pop()
            .unwrap();
        let ranges = positions(visual, &app, &mut manager, "coverage", "read-coverage");
        assert_eq!(ranges[0].range, Some(api::TextRange { start: 40, end: 61 }));
        assert_eq!(ranges[1].range, Some(api::TextRange { start: 63, end: 79 }));
        // These bounded selectors observe the actual painted marker elements, including their hit
        // regions. TestPlatform has no active Windows accessibility client; native acceptance checks
        // Windows UIA separately.
        let markers = [
            (coverage.clone(), "coverage-line-2", 40),
            (coverage, "coverage-line-3", 63),
            (bookmark, "bookmark-1", 0),
        ]
        .map(|(resource, marker, offset)| {
            let selector: &'static str = Box::leak(
                format!(
                    "decoration-{}-{}-{marker}",
                    resource.handle().instance,
                    resource.handle().resource
                )
                .into_boxed_str(),
            );
            (selector, offset)
        });
        for (selector, offset) in markers {
            gutter_position(visual, &app, selector, offset);
        }
        let viewport = visual.update(|_, cx| app.read(cx).editor.read(cx).input_bounds());
        visual.simulate_event(gpui_kit::ScrollWheelEvent {
            position: viewport.center(),
            delta: gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-820.))),
            touch_phase: gpui_kit::TouchPhase::Moved,
            modifiers: Default::default(),
        });
        visual.run_until_parked();
        // More than one stable paint rules out a stale frame while the native scroll event settles.
        for _ in 0..3 {
            visual.update(|window, cx| window.draw(cx).clear(cx));
            let first = visual.update(|_, cx| {
                app.read(cx)
                    .editor
                    .read(cx)
                    .visible_row_range()
                    .unwrap()
                    .start
            });
            assert!(
                first > 3,
                "all three source rows must be above the viewport"
            );
            for (selector, _) in markers {
                assert!(
                    visual.debug_bounds(selector).is_none(),
                    "offscreen marker {selector} still owns a painted hit region: {:?}",
                    visual.debug_bounds(selector)
                );
            }
        }
        visual.simulate_event(gpui_kit::ScrollWheelEvent {
            position: viewport.center(),
            delta: gpui_kit::ScrollDelta::Pixels(point(px(0.), px(820.))),
            touch_phase: gpui_kit::TouchPhase::Moved,
            modifiers: Default::default(),
        });
        visual.run_until_parked();
        for (selector, offset) in markers {
            gutter_position(visual, &app, selector, offset);
        }
        assert_eq!(std::fs::read_to_string(path).unwrap(), source);
    });
}
