//! Assert the production header paint and comparison rails reached through a real plugin package.
use super::*;
use crate::ui::theme::ActiveTheme as _;
use gpui_base::test_support::snapshots;

/// Inspect visible chrome without copying the product's layout or scrollbar arithmetic.
pub(super) fn assert_comparison_chrome(visual: &mut VisualTestContext, count: usize) {
    let toolbar = visual
        .debug_bounds("comparison-toolbar")
        .expect("comparison toolbar");
    let mut headers = vec![toolbar];
    for index in 0..count {
        let selector: &'static str =
            Box::leak(format!("comparison-source-header-{index}").into_boxed_str());
        let header = visual
            .debug_bounds(selector)
            .expect("source header outside native scrolling");
        assert!((header.top() - toolbar.bottom()).abs() < px(1.));
        headers.push(header);
        let rail: &'static str =
            Box::leak(format!("aligned-vertical-scrollbar-{index}").into_boxed_str());
        assert_eq!(visual.debug_bounds(rail).is_some(), index + 1 == count);
    }
    visual.update(|window, cx| {
        let background = cx.theme().muted;
        let editor = crate::theme::component_styles(cx, plugin_schema::ThemeComponent::Editor).base;
        assert_ne!(
            background,
            editor.background.unwrap_or(cx.theme().background),
            "header surface must distinguish scrolling text"
        );
        for header in headers {
            let sample = point(header.right() - px(2.), header.center().y);
            let scale = window.scale_factor();
            let sample = point(
                gpui_kit::ScaledPixels(f32::from(sample.x) * scale),
                gpui_kit::ScaledPixels(f32::from(sample.y) * scale),
            );
            assert!(
                window.painted_quads().iter().any(|quad| {
                    quad.bounds.contains(&sample)
                        && quad.content_mask.bounds.contains(&sample)
                        && quad.background.as_solid() == Some(background)
                }),
                "toolbar and every source header must paint the same semantic background"
            );
        }
        assert!(
            !snapshots(window)
                .iter()
                .any(|node| node.path().iter().any(|part| {
                    matches!(part, gpui_kit::ElementId::Name(name) if name == "close-document-diff")
                })),
            "comparison closes through its regular tab control"
        );
    });
}
