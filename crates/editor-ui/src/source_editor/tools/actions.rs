//! Product code-action menu and deferred keyboard routing over the original Base source.
use super::*;
use crate::scroll::SmoothScrollable as _;

impl EditorTools {
    /// Confirm one current action from the live menu, then dismiss its Base overlay state.
    fn confirm_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A late provider response must not reinterpret a click against another menu incarnation.
        if self.editor.read(cx).code_action_menu_state().revision() != self.actions_revision {
            return;
        }
        let item = self
            .editor
            .read(cx)
            .code_action_menu_state()
            .items
            .get(self.selected)
            .cloned();
        if let Some(item) = item {
            self.editor.update(cx, |state, cx| {
                state.perform_code_action(&item, window, cx);
                state.dismiss_code_action_overlay(cx);
            });
        }
    }
    /// Base forwards action-menu navigation before cursor motion; completion retains the host's router.
    fn route_actions(
        &mut self,
        action: Box<dyn gpui_kit::Action>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let count = self.editor.read(cx).code_action_menu_state().items.len();
        if action.as_any().is::<gpui_base::input::Escape>() {
            self.editor
                .update(cx, |state, cx| state.dismiss_code_action_overlay(cx));
        } else if action.as_any().is::<gpui_base::input::Enter>() {
            self.confirm_action(window, cx);
        } else if action.as_any().is::<gpui_base::input::MoveDown>() {
            if count > 0 {
                self.selected = (self.selected + 1) % count;
            }
        } else if action.as_any().is::<gpui_base::input::MoveUp>() {
            if count > 0 {
                self.selected = (self.selected + count - 1) % count;
            }
        } else {
            return false;
        }
        self.actions_scroll.scroll_to_item(self.selected);
        cx.notify();
        true
    }
    /// Paint live Base code actions without copying document state or adopting another popup runtime.
    pub(super) fn render_actions(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui_kit::AnyElement> {
        let actions = self.editor.read(cx).code_action_menu_state().clone();
        let action_view = if actions.open {
            if self.actions_revision != actions.revision() {
                self.actions_revision = actions.revision();
                self.selected = 0;
            }
            let anchor = {
                let state = self.editor.read(cx);
                let cursor = state.cursor();
                state
                    .range_to_bounds(&(cursor..cursor))
                    .map(|bounds| gpui_kit::point(bounds.left(), bounds.bottom()))
                    .unwrap_or_else(|| state.input_bounds().origin)
            };
            let owner = cx.entity().downgrade();
            self.editor.update(cx, |state, _| {
                state.set_overlay_action_handler(move |kind, action, window, cx| {
                    if kind != InputOverlayKind::CodeAction
                        || !(action.as_any().is::<gpui_base::input::Escape>()
                            || action.as_any().is::<gpui_base::input::Enter>()
                            || action.as_any().is::<gpui_base::input::MoveUp>()
                            || action.as_any().is::<gpui_base::input::MoveDown>())
                    {
                        return false;
                    }
                    // The editor entity is already borrowed while forwarding; defer actions back to UI ownership.
                    let owner = owner.clone();
                    window.defer(cx, move |window, cx| {
                        let _ =
                            owner.update(cx, |tools, cx| tools.route_actions(action, window, cx));
                    });
                    true
                })
            });
            let style = crate::menu::MenuStyle::current(cx);
            let panel = gpui_base::v_flex()
                .id("local-code-actions")
                .debug_selector(|| "local-code-actions".into())
                .role(gpui_kit::Role::Menu)
                .w(px(320.))
                .max_h(px(400.).min((window.viewport_size().height - px(16.)).max(px(0.))))
                .overflow_y_scroll()
                .track_scroll(&self.actions_scroll)
                .occlude()
                .p_1()
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().popover)
                .shadow_md()
                .children(actions.items.iter().enumerate().map(|(index, item)| {
                    style
                        .row(
                            ("local-code-action", index),
                            item.action.title.clone(),
                            index == self.selected,
                        )
                        .selected(index == self.selected)
                        .aria_selected(index == self.selected)
                        .child(item.action.title.clone())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.selected = index;
                            this.confirm_action(window, cx);
                        }))
                }))
                .child(crate::vertical_scrollbar(&self.actions_scroll, cx))
                .smooth_scroll(&self.actions_scroll, crate::scroll::ScrollAxis::Vertical);
            // A physical caret anchor lets the popup escape source clipping and stay inside the window.
            Some(
                gpui_kit::deferred(
                    gpui_kit::anchored()
                        .position(anchor)
                        .snap_to_window_with_margin(px(8.))
                        .child(panel),
                )
                .with_priority(21)
                .into_any_element(),
            )
        } else {
            None
        };
        action_view.map(IntoElement::into_any_element)
    }
}
