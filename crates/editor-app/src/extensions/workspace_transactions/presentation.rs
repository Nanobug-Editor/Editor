//! Native preview, attributed receipts and explicit whole-batch Undo share editor-owned controls.

use super::*;
use crate::ui::controls::Button;
use crate::ui::theme::ActiveTheme as _;
use gpui_kit::div;
use gpui_kit::{
    AnyElement, InteractiveElement as _, IntoElement as _, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _,
};

impl EditorApp {
    pub(super) fn prune_workspace_changes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .workspace_changes
            .prompt
            .as_ref()
            .is_some_and(|prompt| stopped(&prompt.request, &prompt.batch))
        {
            self.answer_workspace_preview(false, window, cx);
        }
        let retired: Vec<_> = self
            .workspace_changes
            .records
            .iter()
            .filter(|(_, record)| !record.batch.is_live())
            .map(|(key, _)| key.clone())
            .collect();
        for key in retired {
            if let Some(record) = self.workspace_changes.records.remove(&key) {
                // Drop large immutable buffers off the UI thread; retirement never restores user files.
                cx.background_executor()
                    .spawn(async move {
                        drop(record);
                    })
                    .detach();
            }
        }
    }

    fn answer_workspace_preview(
        &mut self,
        accept: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(mut prompt) = self.workspace_changes.prompt.take() else {
            return;
        };
        let accepted = accept && !stopped(&prompt.request, &prompt.batch);
        if let Some(answer) = prompt.answer.take() {
            let _ = answer.send(accepted);
        }
        if prompt.focus.contains_focused(window, cx) {
            if let Some(previous) = prompt.previous.take() {
                previous.focus(window, cx);
            }
        }
        cx.notify();
    }

    /// The local modal exposes every resource action and bounded text excerpts before user consent.
    pub(crate) fn render_workspace_plan(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.prune_workspace_changes(window, cx);
        let Some(prompt) = &self.workspace_changes.prompt else {
            return div().into_any_element();
        };
        let focus = prompt.focus.clone();
        let app = cx.entity().downgrade();
        let cancel = app.clone();
        let confirm = app.clone();
        // Local button text does not name the Base action: both channels must use the same locale.
        let cancel_label = t!("interaction.cancel").to_string();
        let confirm_label = t!("workspace_plan.apply").to_string();
        let details = div()
            .id("workspace-plan-details")
            .debug_selector(|| "workspace-plan-details".into())
            .flex()
            .flex_col()
            .gap_2()
            .max_h(px(400.))
            .overflow_y_scroll()
            .children(
                prompt
                    .details
                    .iter()
                    .cloned()
                    .map(|detail| div().text_sm().child(detail)),
            );
        let body = div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(cx.theme().popover)
            .text_color(cx.theme().popover_foreground)
            .child(
                div()
                    .id("workspace-plan-title")
                    // Keep Base's single modal role and expose its visible title as a named heading.
                    .role(gpui_kit::Role::Heading)
                    .aria_label(prompt.title.clone())
                    .font_semibold()
                    .child(prompt.title.clone()),
            )
            .child(
                div()
                    .text_sm()
                    .child(t!("workspace_plan.attribution", source = prompt.source).to_string()),
            )
            .child(
                div()
                    .text_sm()
                    .child(t!("workspace_plan.unsaved_notice").to_string()),
            )
            .child(details)
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("workspace-plan-cancel")
                            .label(cancel_label.clone())
                            .accessibility_label(cancel_label)
                            .debug_selector(|| "workspace-plan-cancel".into())
                            .on_click(move |_, window, cx| {
                                let _ = cancel.update(cx, |app, cx| {
                                    app.answer_workspace_preview(false, window, cx)
                                });
                            }),
                    )
                    .child(
                        Button::new("workspace-plan-confirm")
                            .primary()
                            .label(confirm_label.clone())
                            .accessibility_label(confirm_label)
                            .debug_selector(|| "workspace-plan-confirm".into())
                            .on_click(move |_, window, cx| {
                                let _ = confirm.update(cx, |app, cx| {
                                    app.answer_workspace_preview(true, window, cx)
                                });
                            }),
                    ),
            );
        let escape = app.clone();
        gpui_base::Dialog::new(cx)
            .focus_handle(focus)
            .close_on_backdrop_press(false)
            .on_cancel(move |_, window, cx| {
                let _ = escape.update(cx, |app, cx| {
                    app.answer_workspace_preview(false, window, cx)
                });
                true
            })
            // Enter never grants destructive consent through an inherited editor key binding.
            .on_ok(|_, _, _| false)
            .top(px(60.))
            .popup(
                gpui_base::DialogPopup::new()
                    .w(px(620.))
                    .max_h(px(600.))
                    .rounded(cx.theme().radius_lg)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(body),
            )
            .into_any_element()
    }

    /// The independent host entry remains usable after a plugin canceled its wait or a file step failed.
    pub(crate) fn render_workspace_receipts(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.workspace_changes.records.is_empty() {
            return div().into_any_element();
        }
        let mut content = div()
            .id("workspace-batch-receipts")
            .absolute()
            .bottom(px(36.))
            .left(px(12.))
            .max_w(px(480.))
            .max_h(px(220.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2();
        for (key, record) in &self.workspace_changes.records {
            if !record.batch.is_live() {
                continue;
            }
            let receipt = record.batch.receipt();
            let label = t!(match receipt.state {
                BatchState::Complete => "workspace_plan.complete",
                BatchState::Undone => "workspace_plan.undone",
                BatchState::UndoPartial => "workspace_plan.undo_partial",
                _ => "workspace_plan.partial",
            })
            .to_string();
            let mut card = div()
                .p_3()
                .rounded(px(6.))
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().popover)
                .text_color(cx.theme().popover_foreground)
                .flex()
                .flex_col()
                .gap_2()
                .child(format!("{} · {} · {label}", record.source, record.title))
                .child(
                    t!(
                        "workspace_plan.completed_steps",
                        count = receipt.completed.len(),
                        total = record.changes.len()
                    )
                    .to_string(),
                );
            if let Some(error) = receipt.failed {
                card = card.child(div().text_sm().child(error.message));
            }
            for conflict in receipt.conflicts {
                card = card.child(
                    div()
                        .text_sm()
                        .child(format!("{}: {}", conflict.target, conflict.reason)),
                );
            }
            if receipt.recoverable && receipt.state != BatchState::Undone {
                let key = key.clone();
                let disabled = self.workspace_changes.active;
                let undo_label = t!("workspace_plan.undo").to_string();
                card = card.child(
                    Button::new(format!("workspace-batch-undo-{}", key.2))
                        .label(undo_label.clone())
                        .accessibility_label(undo_label)
                        .disabled(disabled)
                        .debug_selector(|| "workspace-batch-undo".into())
                        .on_click(cx.listener(move |app, _, window, cx| {
                            app.start_workspace_undo(key.clone(), None, window, cx)
                        })),
                );
            }
            content = content.child(card);
        }
        content.into_any_element()
    }
}
