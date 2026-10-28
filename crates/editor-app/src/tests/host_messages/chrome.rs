//! The dock title must not add a second stroke below the window caption separator.

use super::*;

/// Inspect clipped paint at the title's top edge and its actual one-pixel bottom separator.
#[gpui::test]
fn host_messages_header_has_one_caption_separator(cx: &mut TestAppContext) {
    with_messages(cx, |form, _, _| {
        for (dark, scale) in [(false, 1.), (true, 1.5), (false, 2.)] {
            form.update(|window, cx| {
                apply_theme(builtin_theme(dark), cx);
                window.refresh();
            });
            form.simulate_scale_factor_change(scale);
            draw(form);
            let caption = form.debug_bounds("custom-title-bar").unwrap();
            let content = form.debug_bounds("host-messages-panel").unwrap();
            form.update(|window, cx| {
                let scale = window.scale_factor();
                let x = f32::from(content.center().x) * scale;
                let top = point(
                    gpui_kit::ScaledPixels(x),
                    gpui_kit::ScaledPixels(f32::from(caption.bottom()) * scale + 0.25),
                );
                let bottom = point(
                    gpui_kit::ScaledPixels(x),
                    gpui_kit::ScaledPixels((f32::from(content.top()) - 0.5) * scale),
                );
                let quads = window.painted_quads();
                assert!(
                    !quads.iter().any(|quad| {
                        quad.bounds.contains(&top)
                            && quad.content_mask.bounds.contains(&top)
                            && quad.border_color.a > 0.
                            && quad.border_widths.bottom.0 > 0.
                    }),
                    "bottom-only title border must not paint an antialiased top edge"
                );
                let style = component_styles(cx, ThemeComponent::DockTitleBar).base;
                let color = style.border.unwrap_or(cx.theme().border);
                // Inspect the dock title's bottom edge separately from any leading split handle.
                let bottom_edge = f32::from(content.top()) * scale;
                let height = bottom_edge.round() - (bottom_edge - scale).round();
                assert!(
                    quads.iter().any(|quad| {
                        quad.bounds.contains(&bottom)
                            && quad.content_mask.bounds.contains(&bottom)
                            && (quad.bounds.size.height.0 - height).abs() < 0.01
                            && quad.background == color.into()
                    }),
                    "dock title must paint a single logical-pixel separator at scale {scale}"
                );
            });
        }
    });
}

/// Restored hidden slots keep their Base indexes, but the first visible panel has no leading divider.
/// Showing the real Problems peer must restore exactly one ordinary separator at their shared edge.
#[gpui::test]
fn host_messages_hidden_predecessor_has_no_top_split_separator(cx: &mut TestAppContext) {
    use gpui_base::dock::{DockLayout, PanelId};

    with_messages(cx, |form, app, _| {
        form.update(|window, cx| {
            app.update(cx, |app, cx| {
                assert!(!app.session_state.problems_visible);
                // This paint fixture needs ordinary peers even when a reveal restores a regional choice.
                for panel in [app.messages.entity_id(), app.problems.panel.entity_id()] {
                    app.dock_presentation
                        .set_exclusive(PanelId::from(panel), false);
                }
                let editor = app.dock_presentation.panel_handle(app.editor_panel.clone());
                let problems = app
                    .dock_presentation
                    .panel_handle(app.problems.panel.clone());
                let messages = app.dock_presentation.panel_handle(app.messages.clone());
                app.dock_ui.reset_hidden_sizes();
                app.dock_area.update(cx, |area, cx| {
                    area.set_center(
                        DockLayout::h_split()
                            .child(DockLayout::tabs().panel_view(editor, cx), None)
                            .child(
                                DockLayout::v_split()
                                    .child(
                                        DockLayout::tabs().panel_view(problems, cx),
                                        Some(px(200.)),
                                    )
                                    .child(DockLayout::tabs().panel_view(messages, cx), None),
                                Some(px(320.)),
                            ),
                        window,
                        cx,
                    );
                });
                cx.notify();
            });
        });
        for (dark, scale) in [(false, 1.), (true, 1.5), (false, 2.)] {
            form.update(|window, cx| {
                apply_theme(builtin_theme(dark), cx);
                window.refresh();
            });
            form.simulate_scale_factor_change(scale);
            draw(form);
            let caption = form.debug_bounds("custom-title-bar").unwrap();
            let title = messages_title_bounds(form, &app);
            let content = form.debug_bounds("host-messages-panel").unwrap();
            assert!((title.top() - caption.bottom()).abs() < px(1.));
            form.update(|window, cx| {
                let top = point(
                    gpui_kit::ScaledPixels(f32::from(title.center().x) * scale),
                    gpui_kit::ScaledPixels((f32::from(title.top()) * scale).round() + 0.25),
                );
                let color = cx.theme().border;
                assert!(
                    !window.painted_quads().iter().any(|quad| {
                        quad.bounds.contains(&top)
                            && quad.content_mask.bounds.contains(&top)
                            && quad.background == color.into()
                    }),
                    "the hidden Problems predecessor must not paint a solid line above Messages"
                );
                // The enclosing horizontal split still separates Editor from this right-hand column.
                let edge = f32::from(content.left()) * scale;
                let divider = point(
                    gpui_kit::ScaledPixels(edge.round() + 0.25),
                    gpui_kit::ScaledPixels(f32::from(content.center().y) * scale),
                );
                // Base's explicit one-pixel handle size is snapped by layout before paint.
                let width = f32::from(window.pixel_snap(px(1.))) * scale;
                assert_eq!(
                    window
                        .painted_quads()
                        .iter()
                        .filter(|quad| {
                            quad.bounds.contains(&divider)
                                && quad.content_mask.bounds.contains(&divider)
                                && quad.background == color.into()
                                && (quad.bounds.size.width.0 - width).abs() < 0.01
                        })
                        .count(),
                    1,
                    "Editor and the Messages column retain one vertical split line at scale {scale}"
                );
            });

            form.update(|window, cx| {
                app.update(cx, |app, cx| app.show_problems(window, cx));
            });
            draw(form);
            let title = messages_title_bounds(form, &app);
            assert!(title.top() > caption.bottom() + px(50.));
            form.update(|window, cx| {
                let edge = f32::from(title.top()) * scale;
                let point = point(
                    gpui_kit::ScaledPixels(f32::from(title.center().x) * scale),
                    gpui_kit::ScaledPixels(edge.round() + 0.25),
                );
                let height = f32::from(window.pixel_snap(px(1.))) * scale;
                let color = cx.theme().border;
                let lines = window
                    .painted_quads()
                    .iter()
                    .filter(|quad| {
                        quad.bounds.contains(&point)
                            && quad.content_mask.bounds.contains(&point)
                            && quad.background == color.into()
                            && (quad.bounds.size.height.0 - height).abs() < 0.01
                    })
                    .count();
                assert_eq!(
                    lines, 1,
                    "visible Problems and Messages retain one split line at scale {scale}"
                );
            });
            click(form, "problem-close");
            assert!(!form.update(|_, cx| app.read(cx).session_state.problems_visible));
        }
    });
}

/// Base reveal may normalize split nodes, so locate the live Messages title after each visibility edit.
fn messages_title_bounds(form: &mut VisualTestContext, app: &Entity<EditorApp>) -> Bounds<Pixels> {
    use gpui_base::dock::{DockPlacement, PanelId};
    let node = form.update(|_, cx| {
        let app = app.read(cx);
        app.dock_area
            .read(cx)
            .layout(DockPlacement::Center)
            .unwrap()
            .find_panel_node(PanelId::from(app.messages.entity_id()))
            .unwrap()
    });
    let selector: &'static str =
        Box::leak(format!("local-dock-title-{}", node.as_u64()).into_boxed_str());
    form.debug_bounds(selector).expect("visible Messages title")
}
