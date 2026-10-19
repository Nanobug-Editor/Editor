//! Local themed language content uses Base modal, focus-trap and button behavior.
use super::*;
use crate::ui::controls::Button;
use crate::ui::theme::ActiveTheme as _;
use gpui_base::{StyledExt as _, TestSupportExt as _};
use gpui_kit::{
    InteractiveElement as _, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _,
};
use gpui_kit::{prelude::FluentBuilder as _, px};

impl LanguageFeatures {
    /// Only the bounded results scroll; source attribution and native actions remain reachable.
    fn content(&self, height: f32, cx: &mut Context<Self>) -> gpui_kit::Stateful<gpui_kit::Div> {
        let title = rust_i18n::t!(match self.feature {
            direct::Feature::References => "language_features.references",
            direct::Feature::CodeActions => "language_features.code_actions",
            direct::Feature::SignatureHelp => "language_features.signature_help",
        })
        .to_string();
        div()
            .id("language-features-body")
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(cx.theme().popover)
            .text_color(cx.theme().foreground)
            .capture_key_down(cx.listener(Self::navigate))
            .child(
                div()
                    // Stateful identity exposes the native heading's semantic role and name.
                    .id("language-features-heading")
                    .role(gpui_kit::Role::Heading)
                    .aria_label(title.clone())
                    .font_semibold()
                    .child(title)
                    .test_support(),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        rust_i18n::t!("language_features.source", source = self.owner.clone())
                            .to_string(),
                    ),
            )
            .child(self.result_list(height, cx))
            .child(self.footer(cx))
    }

    /// References and fixes use the same named local Base rows. Signature cards are readonly
    /// plain text, so they neither allocate row focus handles nor imitate an executable action.
    fn result_list(
        &self,
        height: f32,
        cx: &mut Context<Self>,
    ) -> gpui_kit::Stateful<gpui_kit::Div> {
        let labels = self.proposal_labels();
        let mut list = div()
            .id("language-feature-results")
            .h(px(height))
            .overflow_y_scroll()
            .track_scroll(&self.scroll);
        for (index, label) in labels.iter().enumerate() {
            list = list.child(
                Button::new(SharedString::from(format!("language-feature-row-{index}")))
                    .label(label.clone())
                    .accessibility_label(label.clone())
                    .tooltip(label.clone())
                    .track_focus(&self.rows[index])
                    .content_full_width()
                    .h(px(36.))
                    .w_full()
                    .flex_shrink_0()
                    .ghost()
                    .when(index == self.selected, |button| button.primary())
                    .debug_selector(move || format!("language-feature-row-{index}"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.selected = index;
                        this.choose(window, cx);
                    })),
            );
        }
        if let Some(Ok(direct::Result::SignatureHelp { help: Some(help) })) = &self.result {
            for (index, signature) in help.signatures.iter().enumerate() {
                list = list.child(self.signature_card(help, signature, index, cx));
            }
        } else if labels.is_empty() {
            let message = match &self.result {
                None => rust_i18n::t!("language_features.loading").to_string(),
                Some(Err(error)) => rust_i18n::t!(
                    "language_features.failed",
                    error = error.to_string().chars().take(4096).collect::<String>()
                )
                .to_string(),
                Some(Ok(_)) => rust_i18n::t!("language_features.empty").to_string(),
            };
            list = list.child(div().child(message));
        }
        list
    }

    /// Display all bounded overloads and the selected parameter without parsing provider markup.
    /// A signature-local index overrides the help-level index; the latter belongs only to the
    /// selected overload. The validated plain result remains the sole source of displayed data.
    fn signature_card(
        &self,
        help: &direct::SignatureHelp,
        signature: &direct::Signature,
        index: usize,
        cx: &Context<Self>,
    ) -> gpui_kit::Stateful<gpui_kit::Div> {
        let active = index == help.active_signature.unwrap_or(0) as usize;
        let parameter = signature
            .active_parameter
            .or_else(|| active.then_some(help.active_parameter).flatten())
            .and_then(|index| signature.parameters.get(index as usize));
        let mut card = div()
            .id(SharedString::from(format!(
                "language-signature-card-{index}"
            )))
            .debug_selector(move || format!("language-signature-card-{index}"))
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .mb_2()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(if active {
                cx.theme().primary
            } else {
                cx.theme().border
            })
            .child(
                div()
                    .id(SharedString::from(format!(
                        "language-signature-label-{index}"
                    )))
                    .role(gpui_kit::Role::Label)
                    .aria_label(signature.label.clone())
                    .font_semibold()
                    .child(signature.label.clone())
                    .test_support(),
            );
        if let Some(parameter) = parameter {
            let label = rust_i18n::t!(
                "language_features.parameter",
                parameter = parameter.label.clone()
            )
            .to_string();
            card = card.child(
                div()
                    .id(SharedString::from(format!(
                        "language-signature-parameter-{index}"
                    )))
                    .role(gpui_kit::Role::Label)
                    .aria_label(label.clone())
                    .text_color(cx.theme().primary)
                    .child(label)
                    // Base observes the painted native role/name; no test-only label is supplied.
                    .test_support(),
            );
            if let Some(documentation) = &parameter.documentation {
                card = card.child(div().text_sm().child(documentation.clone()));
            }
        }
        if let Some(documentation) = &signature.documentation {
            card = card.child(
                div()
                    .id(SharedString::from(format!(
                        "language-signature-documentation-{index}"
                    )))
                    .role(gpui_kit::Role::Label)
                    .aria_label(documentation.clone())
                    .text_sm()
                    .child(documentation.clone())
                    .test_support(),
            );
        }
        card
    }

    /// Dialog dismissal and the selected action use local buttons with visible localized names.
    fn footer(&self, cx: &mut Context<Self>) -> gpui_kit::Div {
        div()
            .flex()
            .justify_end()
            .gap_2()
            .child(
                Button::new("language-features-close")
                    .track_focus(&self.close_focus)
                    .label(rust_i18n::t!("language_features.close").to_string())
                    .accessibility_label(rust_i18n::t!("language_features.close").to_string())
                    .debug_selector(|| "language-features-close".into())
                    .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
            )
            .when(self.feature != direct::Feature::SignatureHelp, |footer| {
                footer.child(
                    Button::new("language-features-choose")
                        .track_focus(&self.choose_focus)
                        .label(rust_i18n::t!("language_features.choose").to_string())
                        .accessibility_label(rust_i18n::t!("language_features.choose").to_string())
                        .disabled(self.rows.is_empty())
                        .primary()
                        .debug_selector(|| "language-features-choose".into())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.choose(window, cx);
                        })),
                )
            })
    }

    /// Base owns Escape, Enter and focus trapping; appearance follows the current local theme.
    pub(super) fn modal(&self, window: &Window, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let available_height = (window.viewport_size().height / px(1.) - 100.).max(180.);
        let width = (window.viewport_size().width / px(1.) - 32.).clamp(180., 620.);
        let content = self.content((available_height - 160.).clamp(60., 300.), cx);
        let cancel = cx.entity().downgrade();
        let confirm = cancel.clone();
        gpui_base::Dialog::new(cx)
            .focus_handle(self.focus.clone())
            .close_on_backdrop_press(false)
            .on_cancel(move |_, window, cx| {
                let _ = cancel.update(cx, |this, cx| this.close(window, cx));
                true
            })
            .on_ok(move |_, window, cx| {
                confirm
                    .update(cx, |this, cx| this.choose(window, cx))
                    .unwrap_or(false)
            })
            .top(px(50.))
            .popup(
                gpui_base::DialogPopup::new()
                    .w(px(width))
                    .max_h(px(available_height))
                    .rounded(cx.theme().radius_lg)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(content),
            )
            .into_any_element()
    }
}
