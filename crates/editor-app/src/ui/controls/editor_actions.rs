//! Line-edge controls use Base's actual laid-out source geometry, without a second editor mount.
use gpui_base::input::{EditorState, RopeExt as _};
use gpui_kit::{
    AnyElement, App, AvailableSpace, Bounds, Entity, IntoElement, ParentElement, Pixels, Styled,
    div, point, px, size,
};
use std::rc::Rc;

/// Optional display-only projection for a source whose native bytes have aligned visual gaps.
pub(crate) type EditorActionProjection = Rc<dyn Fn(usize, &App) -> Option<Bounds<Pixels>>>;

/// Already approved ordinary native controls, grouped by UTF-8 byte anchors on either edge.
#[derive(Default)]
pub(crate) struct EditorActionElements {
    pub(crate) left: Vec<(usize, AnyElement)>,
    pub(crate) right: Vec<(usize, AnyElement)>,
}

/// Use the same local rails over either ordinary or aligned source geometry. The projection must
/// return `None` for stale/offscreen offsets; controls never guess positions during remeasurement.
pub(crate) fn with_projected_editor_actions(
    source: AnyElement,
    editor: &Entity<EditorState>,
    actions: EditorActionElements,
    projection: Option<EditorActionProjection>,
) -> AnyElement {
    div()
        .relative()
        .flex()
        .size_full()
        .min_h_0()
        .overflow_hidden()
        .child(div().w(px(76.)).flex_shrink_0())
        .child(div().flex_1().min_w_0().min_h_0().child(source))
        .child(div().w(px(76.)).flex_shrink_0())
        .child(rail(
            editor.clone(),
            actions.left,
            false,
            projection.clone(),
        ))
        .child(rail(editor.clone(), actions.right, true, projection))
        .into_any_element()
}

/// Offscreen or folded ranges have no control hit box. The canvas only positions Base buttons;
/// it never interprets input, scrolls the source, inserts rows, or mutates native document state.
fn rail(
    editor: Entity<EditorState>,
    actions: Vec<(usize, AnyElement)>,
    right: bool,
    projection: Option<EditorActionProjection>,
) -> AnyElement {
    use gpui_kit::prelude::FluentBuilder as _;
    gpui_kit::canvas(
        move |bounds, window, cx| {
            let mut controls = Vec::new();
            for (offset, mut content) in actions {
                let anchor = {
                    let state = editor.read(cx);
                    if offset > state.text().len()
                        || !state.text().is_char_boundary(offset)
                        || state.visible_row_range().is_none_or(|visible| {
                            !visible.contains(&state.text().offset_to_point(offset).row)
                        })
                    {
                        None
                    } else if let Some(project) = &projection {
                        project(offset, cx)
                    } else {
                        state.range_to_bounds(&(offset..offset))
                    }
                };
                let Some(anchor) = anchor else {
                    continue;
                };
                let height = content
                    .layout_as_root(
                        size(
                            AvailableSpace::Definite(bounds.size.width),
                            AvailableSpace::MaxContent,
                        ),
                        window,
                        cx,
                    )
                    .height;
                if anchor.top() < bounds.top() || anchor.top() + height > bounds.bottom() {
                    continue;
                }
                content.prepaint_at(point(bounds.left(), anchor.top()), window, cx);
                controls.push(content);
            }
            controls
        },
        |_, controls, window, cx| {
            for mut content in controls {
                content.paint(window, cx);
            }
        },
    )
    .absolute()
    .top_0()
    .h_full()
    .w(px(76.))
    .when(right, |rail| rail.right_0())
    .when(!right, |rail| rail.left_0())
    .into_any_element()
}
