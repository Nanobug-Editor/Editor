//! Read and write upstream scroll state without copying text, row geometry or virtualized items.
use super::smooth::ScrollAxis;
use gpui_base::ScrollbarHandle;
use gpui_base::input::{EditorState, TextareaState};
use gpui_kit::{AnyElement, App, Entity, EntityId, IntoElement, Pixels, Point, Window, point, px};
use std::rc::Rc;

/// Current native offset and optional native bounds, all in logical pixels.
#[derive(Clone, Copy)]
pub(super) struct Metrics {
    /// Offset uses GPUI's negative content-translation convention.
    pub offset: Point<Pixels>,
    /// None delegates final clamping to the native input's private layout metrics.
    pub max: Option<Point<Pixels>>,
    /// Exact native line height when available; containers inherit their window's text style.
    pub line_height: Option<Pixels>,
}

impl Metrics {
    /// Clamp a requested translation to the known bounds; native inputs enforce their own minimum.
    pub fn clamp(self, offset: Point<Pixels>) -> Point<Pixels> {
        let max = self.max.unwrap_or(point(px(f32::MAX), px(f32::MAX)));
        point(
            offset.x.clamp(-max.x, px(0.)),
            offset.y.clamp(-max.y, px(0.)),
        )
    }
}

/// Context-aware offset access also supports native inputs whose scroll handle is private.
#[derive(Clone)]
pub(super) struct Target {
    /// Weak native owners return None after their source closes.
    pub read: Rc<dyn Fn(&App) -> Option<Metrics>>,
    /// Updates only the original viewport and its existing notification path.
    pub write: Rc<dyn Fn(Point<Pixels>, &mut Window, &mut App)>,
}

impl Target {
    /// Adapt any public Base scrollbar handle, including virtual trees and variable-height lists.
    pub fn handle<H: ScrollbarHandle + Clone>(handle: &H) -> Self {
        let read = handle.clone();
        let write = handle.clone();
        Self {
            read: Rc::new(move |_| {
                let viewport = read.viewport_bounds().size;
                let content = read.content_size();
                Some(Metrics {
                    offset: read.offset(),
                    max: Some(point(
                        (content.width - viewport.width).max(px(0.)),
                        (content.height - viewport.height).max(px(0.)),
                    )),
                    line_height: None,
                })
            }),
            write: Rc::new(move |offset, _, _| {
                write.set_offset(offset);
                // The surface/router already invalidates its captured owner. Precise external
                // wheels can write during input dispatch, where current_view is unavailable.
            }),
        }
    }
}

/// Add motion around the complete native source frame; colored-band copies stay ordinary paints.
pub(crate) fn editor(editor: &Entity<EditorState>, content: AnyElement) -> AnyElement {
    let source = editor.downgrade();
    let read = source.clone();
    native(
        editor.entity_id(),
        Target {
            read: Rc::new(move |cx| {
                let source = read.upgrade()?;
                let source = source.read(cx);
                Some(Metrics {
                    offset: source.scroll_offset(),
                    max: None,
                    line_height: source.line_height(),
                })
            }),
            write: Rc::new(move |offset, _, cx| {
                let _ = source.update(cx, |source, cx| source.set_scroll_offset(offset, cx));
            }),
        },
        content,
    )
}

/// Multiline form fields use the same private native clamping and original IME/Undo authority.
pub(crate) fn textarea(textarea: &Entity<TextareaState>, content: AnyElement) -> AnyElement {
    let source = textarea.downgrade();
    let read = source.clone();
    native(
        textarea.entity_id(),
        Target {
            read: Rc::new(move |cx| {
                let source = read.upgrade()?;
                let source = source.read(cx);
                Some(Metrics {
                    offset: source.scroll_offset(),
                    max: None,
                    line_height: source.line_height(),
                })
            }),
            write: Rc::new(move |offset, _, cx| {
                let _ = source.update(cx, |source, cx| source.set_scroll_offset(offset, cx));
            }),
        },
        content,
    )
}

/// Keep each native entity's animation independent of other editors mounted in the window.
fn native(id: EntityId, target: Target, content: AnyElement) -> AnyElement {
    super::SmoothScroll::custom(("native-wheel", id), target, ScrollAxis::Both, content)
        .into_any_element()
}
