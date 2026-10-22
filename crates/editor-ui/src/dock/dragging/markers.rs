//! Paint-only source and destination hatching leaves the real dock layout and pointer targets intact.
use super::*;
use gpui_kit::{BorderStyle, ContentMask, Hsla, PathBuilder, fill, outline, point};

impl LiveDrag {
    /// Draw the currently selected members at their unchanged source and proposed destination slots.
    pub fn markers(&self, cx: &App) -> AnyElement {
        let state = self.0.borrow();
        let Some(session) = state.as_ref() else {
            return Empty.into_any_element();
        };
        let origin = session.original.area_bounds.origin;
        let mut layer = div().absolute().inset_0();
        for (index, id) in session.members.iter().enumerate() {
            if let Some(bounds) = session.original.panel_bounds(*id) {
                layer = layer.child(marker(
                    format!("local-dock-source-{index}"),
                    Bounds::new(bounds.origin - origin, bounds.size),
                    cx.theme().warning,
                    *id == session.source,
                ));
            }
        }
        if let Some(destination) = session.destination(cx) {
            let mut target = div()
                .id("local-dock-drop-indicator")
                .debug_selector(|| "local-dock-drop-indicator".into())
                .test_support()
                .absolute()
                .left(destination.left() - origin.x)
                .top(destination.top() - origin.y)
                .w(destination.size.width)
                .h(destination.size.height);
            for (index, (id, bounds)) in session.rectangles(destination, cx).into_iter().enumerate()
            {
                target = target.child(marker(
                    format!("local-dock-target-{index}"),
                    Bounds::new(bounds.origin - destination.origin, bounds.size),
                    cx.theme().primary,
                    id == session.source,
                ));
            }
            layer = layer.child(target);
        }
        layer.into_any_element()
    }
}

/// One noninteractive marker uses clipped diagonal lines so the native contents remain recognizable.
fn marker(name: String, bounds: Bounds<Pixels>, color: Hsla, primary: bool) -> impl IntoElement {
    let name = gpui_kit::SharedString::from(name);
    div()
        .id(name.clone())
        .debug_selector(move || name.to_string())
        .test_support()
        .absolute()
        .left(bounds.left())
        .top(bounds.top())
        .w(bounds.size.width)
        .h(bounds.size.height)
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    window.paint_quad(fill(
                        bounds,
                        color.opacity(if primary { 0.12 } else { 0.06 }),
                    ));
                    window.with_content_mask(Some(ContentMask { bounds }), |window| {
                        let mut offset = -bounds.size.height;
                        while offset < bounds.size.width {
                            let mut path = PathBuilder::stroke(px(1.));
                            path.move_to(point(bounds.left() + offset, bounds.bottom()));
                            path.line_to(point(
                                bounds.left() + offset + bounds.size.height,
                                bounds.top(),
                            ));
                            if let Ok(path) = path.build() {
                                window.paint_path(
                                    path,
                                    color.opacity(if primary { 0.48 } else { 0.22 }),
                                );
                            }
                            offset += px(8.);
                        }
                    });
                    window.paint_quad(outline(
                        bounds,
                        color.opacity(if primary { 0.85 } else { 0.4 }),
                        BorderStyle::Dashed,
                    ));
                },
            )
            .absolute()
            .inset_0(),
        )
}
