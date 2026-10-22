//! Small title-only cards follow the pointer while the native panel content stays in its original slot.
use super::*;

/// A gesture-owned view names the current moving members without rendering their content twice.
pub(super) struct FloatingGroup {
    /// Read-only gesture state supplies membership and the original grab offset.
    controller: LiveDrag,
    /// The Base owner is used only to end an abandoned gesture.
    area: WeakEntity<DockArea>,
    /// The surface's local chrome height determines compact card height.
    title_height: f32,
}

impl LiveDrag {
    /// Clear markers if GPUI retires the drag without a valid release or Escape callback.
    pub fn preview(
        &self,
        area: WeakEntity<DockArea>,
        _frames: PaintedFrames,
        title_height: f32,
        token: u64,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<FloatingGroup> {
        let controller = self.clone();
        let owner = window.window_handle();
        cx.new(|cx| {
            cx.on_release(move |this: &mut FloatingGroup, cx| {
                if this
                    .controller
                    .0
                    .borrow()
                    .as_ref()
                    .is_some_and(|session| session.token == token)
                {
                    let controller = this.controller.clone();
                    let area = this.area.clone();
                    let _ = owner.update(cx, |_, window, cx| {
                        let _ =
                            area.update(cx, |area, cx| controller.finish(false, area, window, cx));
                    });
                }
            })
            .detach();
            FloatingGroup {
                controller,
                area,
                title_height,
            }
        })
    }
}

impl Render for FloatingGroup {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.controller.0.borrow();
        let Some(session) = state.as_ref() else {
            return div().into_any_element();
        };
        let width = px(176.);
        let height = px(self.title_height);
        let gap = px(4.);
        let stack_height = (height + gap) * session.members.len() as f32 - gap;
        let grabbed = session
            .members
            .iter()
            .position(|id| *id == session.source)
            .unwrap_or(0);
        let cards = session
            .members
            .iter()
            .enumerate()
            .filter_map(|(index, id)| {
                let view = session.original.views.get(id)?;
                let label = PanelHandle::of(view)
                    .and_then(|handle| handle.drag_title(cx))
                    .unwrap_or_else(|| view.panel_name(cx).into());
                let primary = *id == session.source;
                let name = gpui_kit::SharedString::from(format!("local-dock-drag-card-{index}"));
                Some(
                    div()
                        .id(name.clone())
                        .debug_selector(move || name.to_string())
                        .test_support()
                        .absolute()
                        .left(px(0.))
                        .top((height + gap) * index as f32)
                        .w(width)
                        .h(height)
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .bg(cx
                            .theme()
                            .background
                            .opacity(if primary { 1. } else { 0.88 }))
                        .text_color(if primary {
                            cx.theme().foreground
                        } else {
                            cx.theme().muted_foreground
                        })
                        .border_1()
                        .border_color(if primary {
                            cx.theme().primary
                        } else {
                            cx.theme().border
                        })
                        .rounded(px(4.))
                        .overflow_hidden()
                        .child(div().truncate().child(label))
                        .into_any_element(),
                )
            })
            .collect::<Vec<_>>();
        let floating = self
            .controller
            .capture_escape(div().id("local-dock-floating-preview"), self.area.clone())
            .debug_selector(|| "local-dock-floating-preview".into())
            .test_support()
            .absolute()
            // GPUI retains the original title's grab offset. Move the cards within a transparent
            // root so even a very wide source keeps its grabbed card immediately under the pointer.
            .left(session.grab.x - px(12.))
            .top(session.grab.y - px(12.) - (height + gap) * grabbed as f32)
            .w(width)
            .h(stack_height)
            .children(cards);
        div()
            .relative()
            .w(width)
            .h(stack_height)
            .child(floating)
            .into_any_element()
    }
}
