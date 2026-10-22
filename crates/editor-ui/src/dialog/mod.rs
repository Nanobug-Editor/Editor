//! Editor dialog content layout and appearance.

use crate::scroll::ScrollableElement as _;
use crate::theme::ActiveTheme as _;
use gpui_base::StyledExt as _;
use gpui_kit::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, StyleRefinement, Styled, Window,
};

/// Native dialog body over gpui-base's focus trap and Escape/Enter handling.
///
/// AppDialog owns the title bar and native HWND; this surface fills only its body, leaving window
/// dragging and close controls reachable. Callers own staged edits and may veto dismissal/save.
pub fn native_modal(
    focus: gpui_kit::FocusHandle,
    content: AnyElement,
    cancel: impl Fn(&mut Window, &mut App) -> bool + 'static,
    confirm: impl Fn(&mut Window, &mut App) -> bool + 'static,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    use gpui_kit::{div, px};
    let viewport = window.viewport_size();
    let title_height = px(crate::PANEL_HEADER_HEIGHT);
    gpui_base::Dialog::new(cx)
        .focus_handle(focus)
        .close_on_backdrop_press(false)
        .on_cancel(move |_, window, cx| cancel(window, cx))
        .on_ok(move |_, window, cx| confirm(window, cx))
        // Base Dialog's deferred host starts at the viewport origin; reserve AppDialog's title bar.
        .top(title_height)
        .h((viewport.height - title_height).max(px(0.)))
        .popup(
            gpui_base::DialogPopup::new()
                .size_full()
                .flex()
                .flex_col()
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .child(content),
                ),
        )
        .into_any_element()
}

/// Content region styled by the editor while callers own its children.
#[derive(IntoElement)]
pub struct DialogContent {
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl DialogContent {
    /// Start an empty dialog body; the containing modal owns focus, completion and cancellation.
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }
}

impl ParentElement for DialogContent {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for DialogContent {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for DialogContent {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        gpui_kit::div()
            .flex()
            .flex_col()
            .w_full()
            .flex_1()
            // Expanded content must shrink into the popup so its own scroller, not the footer, grows.
            .min_h_0()
            .rounded(cx.theme().radius_lg)
            .children(self.children)
            .refine_style(&self.style)
    }
}

/// Local modal surface; Base retains focus trapping, Escape, confirmation and close vetoes.
#[derive(IntoElement)]
pub struct Dialog {
    /// Underlying behavior and callbacks, with no upstream presentation runtime.
    base: gpui_base::Dialog,
    /// Local title content announced by Base's dialog title element.
    title: gpui_kit::SharedString,
    /// Logical popup width, constrained to the current viewport during rendering.
    width: gpui_kit::Pixels,
    /// Whether to offer the local cancel button in the top corner.
    close_button: bool,
    /// Product content is rebuilt by the overlay owner on each frame.
    content: Option<Box<dyn FnOnce(DialogContent, &mut Window, &mut App) -> DialogContent>>,
    /// Explicit local footer choices, including plugin permission consent.
    footer: Option<AnyElement>,
}
impl Dialog {
    /// Construct an empty modal with Base interaction and local surface defaults.
    pub fn new(cx: &mut App) -> Self {
        Self {
            base: gpui_base::Dialog::new(cx),
            title: "".into(),
            width: gpui_kit::px(520.),
            close_button: true,
            content: None,
            footer: None,
        }
    }
    /// Keep one focus owner across overlay rerenders.
    pub fn focus_handle(mut self, focus: gpui_kit::FocusHandle) -> Self {
        self.base = self.base.focus_handle(focus);
        self
    }
    /// Set the visible, accessible title.
    pub fn title(mut self, title: impl Into<gpui_kit::SharedString>) -> Self {
        self.title = title.into();
        self
    }
    /// Set the desired popup width in logical pixels.
    pub fn width(mut self, width: gpui_kit::Pixels) -> Self {
        self.width = width;
        self
    }
    /// Control whether backdrop presses request cancellation.
    pub fn overlay_closable(mut self, value: bool) -> Self {
        self.base = self.base.close_on_backdrop_press(value);
        self
    }
    /// Enable or suppress the local corner cancel affordance.
    pub fn close_button(mut self, value: bool) -> Self {
        self.close_button = value;
        self
    }
    /// Supply the local content layout using the current window and theme.
    pub fn content(
        mut self,
        build: impl FnOnce(DialogContent, &mut Window, &mut App) -> DialogContent + 'static,
    ) -> Self {
        self.content = Some(Box::new(build));
        self
    }
    /// Mount explicit local action choices beneath the scrolling content.
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }
    /// Confirmation may veto dismissal while the business owner queues its operation.
    pub fn on_ok(
        mut self,
        callback: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.base = self.base.on_ok(callback);
        self
    }
    /// Cancellation may veto dismissal while preserving staged user input.
    pub fn on_cancel(
        mut self,
        callback: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.base = self.base.on_cancel(callback);
        self
    }
    /// Notify the overlay owner after Base accepts cancellation or confirmation.
    pub fn request_close(mut self, callback: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.base = self
            .base
            .request_close(move |_, window, cx| callback(window, cx));
        self
    }
    /// Retain Base's ordering and keyboard authority for stacked dialogs.
    pub fn layer(mut self, index: usize, topmost: bool) -> Self {
        self.base = self.base.layer(index, topmost);
        self
    }
}
impl RenderOnce for Dialog {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        use gpui_kit::{InteractiveElement as _, div, prelude::FluentBuilder as _, px};
        let viewport = window.viewport_size();
        let content = self
            .content
            .take()
            .map(|build| build(DialogContent::new(), window, cx));
        let header = gpui_base::h_flex()
            .items_center()
            .justify_between()
            .child(
                gpui_base::DialogTitle::new()
                    .font_semibold()
                    .child(self.title),
            )
            .when(self.close_button, |header| {
                header.child(gpui_base::DialogClose::new().trigger(|base| {
                    base.accessibility_label(rust_i18n::t!("plugins.cancel").to_string())
                        .child(super::Icon::new(gpui_kit::assets::IconName::Close))
                }))
            });
        // Base centers within its host. Reserve the native title bar in both origin and height,
        // so a tall permission dialog keeps its footer inside the actual window.
        let title_height = px(crate::PANEL_HEADER_HEIGHT);
        self.base
            .top(title_height)
            .h((viewport.height - title_height).max(px(0.)))
            .backdrop(
                gpui_base::DialogBackdrop::new()
                    .size_full()
                    .bg(cx.theme().overlay),
            )
            .popup(
                gpui_base::DialogPopup::new()
                    .w(self.width.min((viewport.width - px(24.)).max(px(0.))))
                    .max_h((viewport.height - px(88.)).max(px(0.)))
                    .rounded(cx.theme().radius_lg)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().popover)
                    .text_color(cx.theme().popover_foreground)
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .shadow_lg()
                    .child(header)
                    .child(
                        div()
                            .id("local-dialog-body")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scrollbar()
                            .children(content),
                    )
                    .children(self.footer),
            )
    }
}

/// Local footer aligns explicit consent buttons while the modal body scrolls independently.
#[derive(IntoElement)]
pub struct DialogFooter {
    children: Vec<AnyElement>,
}
impl DialogFooter {
    /// Create a footer without implicit choices or labels.
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }
}
impl ParentElement for DialogFooter {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}
impl RenderOnce for DialogFooter {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        gpui_base::h_flex()
            .gap_2()
            .w_full()
            .justify_end()
            .flex_shrink_0()
            .children(self.children)
    }
}

/// Confirmation dispatch originates inside this modal even if another view has taken focus.
#[derive(IntoElement)]
pub struct DialogAction {
    children: Vec<AnyElement>,
}
impl DialogAction {
    /// Wrap a local button whose accepted click should request Base confirmation.
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }
}
impl ParentElement for DialogAction {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}
impl RenderOnce for DialogAction {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        use gpui_kit::{
            InteractiveElement as _, StatefulInteractiveElement as _, Styled as _, div,
        };
        let focus = window
            .use_keyed_state("local-dialog-confirm", cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        div()
            .id("dialog-action")
            .child(div().absolute().size_0().track_focus(&focus))
            .on_click(move |_, window, cx| {
                focus.dispatch_action(
                    &gpui_base::actions::Confirm { secondary: false },
                    window,
                    cx,
                )
            })
            .children(self.children)
    }
}
