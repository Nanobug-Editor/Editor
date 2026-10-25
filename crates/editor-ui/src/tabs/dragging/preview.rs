//! The native drag view draws one lifted card and retires abandoned gesture generations.

use super::{FileTabDragState, FileTabPreview};
use crate::theme::{ActiveTheme as _, component_styles};
use gpui_kit::{
    App, AppContext as _, Bounds, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    Pixels, Render, Styled, Window, div, img, prelude::FluentBuilder as _, px,
};
use plugin_schema::ThemeComponent;

/// Native GPUI owns this view's cursor offset, topmost painting and release lifetime.
pub(super) struct FloatingTab {
    /// Only the matching live generation may be retired when this view disappears.
    state: FileTabDragState,
    /// Source metadata remains immutable until the drag ends.
    preview: FileTabPreview,
    /// Full original dimensions keep a long source from jumping under the cursor.
    bounds: Bounds<Pixels>,
}

/// Build a floating `preview` for the active `state`; old views cannot retire later gestures.
pub(super) fn floating(
    state: &FileTabDragState,
    preview: FileTabPreview,
    window: &mut Window,
    cx: &mut App,
) -> Entity<FloatingTab> {
    let (token, bounds) = state
        .0
        .borrow()
        .session
        .as_ref()
        .map(|session| (Some(session.token), session.source_bounds))
        .unwrap_or_default();
    let state = state.clone();
    let owner = window.window_handle();
    cx.new(|cx| {
        cx.on_release(move |view: &mut FloatingTab, cx| {
            let retired = token.is_some_and(|token| view.state.0.borrow_mut().retire(Some(token)));
            if retired {
                let _ = owner.update(cx, |_, window, _| window.refresh());
            }
        })
        .detach();
        cx.observe_window_activation(window, |view, window, cx| {
            if !window.is_window_active() {
                view.state.cancel(window, cx);
            }
        })
        .detach();
        FloatingTab {
            state,
            preview,
            bounds,
        }
    })
}

impl Render for FloatingTab {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let style = component_styles(cx, ThemeComponent::EditorTabDragPreview).base;
        let invalid = self.state.0.borrow().target().is_none();
        div()
            .relative()
            .w(self.bounds.size.width)
            .h(self.bounds.size.height)
            .child(
                div()
                    .debug_selector(|| "file-tab-drag-preview".into())
                    .relative()
                    .top(px(-8.))
                    .size_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px(px(style.padding_x_px.unwrap_or(8.)))
                    .rounded(px(style.radius_px.unwrap_or(4.)))
                    .border_1()
                    .border_color(if invalid {
                        cx.theme().border
                    } else {
                        cx.theme().primary
                    })
                    .bg(style.background.unwrap_or(cx.theme().background))
                    .text_color(style.foreground.unwrap_or(cx.theme().foreground))
                    .text_size(px(style.font_size_px.unwrap_or(14.)))
                    .shadow_lg()
                    .child(
                        div()
                            .size(px(16.))
                            .flex_shrink_0()
                            .child(img(self.preview.icon.clone()).size_full()),
                    )
                    .child(
                        super::super::FileTabTitle::new(
                            "file-tab-drag-title",
                            self.preview.label.clone(),
                        )
                        .when(self.preview.external, |title| title.italic()),
                    )
                    .when(self.preview.preview, |card| {
                        card.child(super::super::preview_tab_badge(cx))
                    })
                    .when(invalid, |card| card.opacity(0.75)),
            )
    }
}
