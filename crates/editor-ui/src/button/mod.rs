//! Product button appearance; gpui-base owns activation, focus, and semantics.

use super::{Icon, Tooltip};
use crate::theme::ActiveTheme as _;
use gpui_base::{Button as BaseButton, StyledExt as _};
use gpui_kit::assets::IconName;
use gpui_kit::{
    AnyElement, App, ClickEvent, ElementId, FocusHandle, Global, Hsla, InteractiveElement,
    IntoElement, KeyBinding, NoAction, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _,
    px,
};

/// Register the local button context once per application, including independent native test apps.
struct ButtonKeymap;
impl Global for ButtonKeymap {}

#[derive(Clone, Copy)]
enum Variant {
    Default,
    Primary,
    Danger,
    Ghost,
    Custom(ButtonCustomVariant),
}

/// Colors supplied by a declarative plugin widget, resolved at its call site.
#[derive(Clone, Copy)]
pub struct ButtonCustomVariant {
    background: Hsla,
    foreground: Hsla,
    hover: Hsla,
    active: Hsla,
}

impl ButtonCustomVariant {
    /// Start custom colors from the active palette in cx; apply a theme before creating controls.
    pub fn new(cx: &App) -> Self {
        Self {
            background: cx.theme().button,
            foreground: cx.theme().button_foreground,
            hover: cx.theme().button_hover,
            active: cx.theme().button_active,
        }
    }

    /// Use color as the custom background.
    pub fn color(mut self, color: Hsla) -> Self {
        self.background = color;
        self
    }

    /// Use color as the custom label and icon foreground.
    pub fn foreground(mut self, color: Hsla) -> Self {
        self.foreground = color;
        self
    }

    /// Use color as the custom hover background.
    pub fn hover(mut self, color: Hsla) -> Self {
        self.hover = color;
        self
    }

    /// Use color as the custom pressed background.
    pub fn active(mut self, color: Hsla) -> Self {
        self.active = color;
        self
    }
}

/// Keeps the editor's visual variants behind one small interface.
#[derive(IntoElement)]
pub struct Button {
    base: BaseButton,
    style: StyleRefinement,
    variant: Variant,
    label: Option<SharedString>,
    /// Explicit accessible names take precedence over visible labels and icon-only tooltips.
    accessibility_label: Option<SharedString>,
    icon: Option<Icon>,
    children: Vec<AnyElement>,
    tooltip: Option<SharedString>,
    small: bool,
    compact: bool,
    outline: bool,
    loading: bool,
    disabled: bool,
    /// Owner-selected buttons project both their accessible state and local active wash.
    selected: bool,
    content_full_width: bool,
    reveal_group: Option<SharedString>,
    /// Scene-native forms scale local metrics without replacing Base behavior or focus.
    scale: f32,
}

impl Button {
    /// Register the native Enter boundary before taking the application default-keymap snapshot.
    /// Render keeps this idempotent call for standalone controls and independent test apps.
    pub fn init_keys(cx: &mut App) {
        if !cx.has_global::<ButtonKeymap>() {
            // A focused button receives its own keyboard click instead of confirming its parent.
            cx.bind_keys([KeyBinding::new("enter", NoAction, Some("EditorActivate"))]);
            cx.set_global(ButtonKeymap);
        }
    }

    /// Create a local button with stable id; Base retains activation, focus and accessibility.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            base: BaseButton::new(id),
            style: StyleRefinement::default(),
            variant: Variant::Default,
            label: None,
            accessibility_label: None,
            icon: None,
            children: Vec::new(),
            tooltip: None,
            small: false,
            compact: false,
            outline: false,
            loading: false,
            disabled: false,
            selected: false,
            content_full_width: false,
            reveal_group: None,
            scale: 1.,
        }
    }

    /// Keep a behavior owner's activation/accessibility state while applying local appearance.
    /// Dialog and disclosure triggers supply the Base button without supplying product styling.
    pub fn with_base(mut self, base: BaseButton) -> Self {
        self.base = base;
        self
    }

    /// Apply a finite positive uniform scale to default native metrics; explicit Styled overrides win.
    pub fn scale(mut self, scale: f32) -> Self {
        debug_assert!(
            scale.is_finite() && scale > 0.,
            "button scale must be finite and positive"
        );
        self.scale = scale;
        self
    }

    /// Set the visible label; it also names the button unless accessibility_label overrides it.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set an explicit semantic label, independent of visible content and tooltips.
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    /// Keep controlled selection visible and expose it through Base button semantics.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self.base = self.base.selected(selected).aria_selected(selected);
        self
    }

    /// Report the owner's disclosure state without storing another mutable expansion state.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.base = self.base.aria_expanded(expanded);
        self
    }

    /// Publish the actual Base button hit region for native interaction acceptance.
    pub fn debug_selector(mut self, selector: impl Fn() -> String + 'static) -> Self {
        self.base = self.base.debug_selector(selector);
        self
    }

    /// Use an owner-managed handle for focus observation; Base still owns tab order and activation.
    pub fn track_focus(mut self, handle: &FocusHandle) -> Self {
        self.base = self.base.track_focus(handle);
        self
    }

    /// Constrain composed card content to the button's width; normal labels keep their intrinsic centered layout.
    pub fn content_full_width(mut self) -> Self {
        self.content_full_width = true;
        self
    }

    /// Place icon before the label without creating a separate focus target.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Show tooltip on the Base hover surface; icon-only buttons also use it as their default name.
    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Deliver native activation to handler with its original event, window and application context.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.base = self.base.on_click(handler);
        self
    }

    /// Set disabled presentation and Base activation rejection together.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Display loading motion and withhold activation while loading is true.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Select primary colors from the active product theme.
    pub fn primary(mut self) -> Self {
        self.variant = Variant::Primary;
        self
    }

    /// Select destructive-action colors from the active product theme.
    pub fn danger(mut self) -> Self {
        self.variant = Variant::Danger;
        self
    }

    /// Use a transparent resting background and the local hover/pressed washes.
    pub fn ghost(mut self) -> Self {
        self.variant = Variant::Ghost;
        self
    }

    /// Use the supplied custom palette while preserving Base button behavior.
    pub fn custom(mut self, custom: ButtonCustomVariant) -> Self {
        self.variant = Variant::Custom(custom);
        self
    }

    /// Keep an accent border and label while retaining a soft variant-colored background.
    pub fn outline(mut self) -> Self {
        self.outline = true;
        self
    }

    /// Select the compact visual size used in native toolbars.
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }

    /// Reduce horizontal content spacing without adding a second hit target.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// Reveal secondary row actions on pointer hover or keyboard focus, retaining their tab order.
    /// The named parent group owns hover; Base still owns this button's focus and activation.
    pub fn reveal_on_hover(mut self, group: impl Into<SharedString>) -> Self {
        self.reveal_group = Some(group.into());
        self
    }
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for Button {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        Self::init_keys(cx);
        let palette = cx.theme();
        let (background, foreground, hover, active) = match self.variant {
            Variant::Default => (
                palette.button,
                palette.button_foreground,
                palette.button_hover,
                palette.button_active,
            ),
            Variant::Primary => (
                palette.button_primary,
                palette.button_primary_foreground,
                palette.button_primary_hover,
                palette.button_primary_active,
            ),
            Variant::Danger => (
                palette.danger,
                palette.danger_foreground,
                palette.danger.opacity(0.85),
                palette.danger.opacity(0.7),
            ),
            Variant::Ghost => (
                palette.transparent,
                palette.foreground,
                palette.list_hover,
                palette.list_active,
            ),
            Variant::Custom(style) => (
                style.background,
                style.foreground,
                style.hover,
                style.active,
            ),
        };
        // Filled variants use their background as the accent; custom variants supply a readable label color.
        let outline_color = match self.variant {
            Variant::Primary | Variant::Danger => background,
            _ => foreground,
        };
        let (background, foreground, hover, active) = if self.outline {
            // Solid primary/danger variants receive a light tint; custom palettes keep their chosen fills.
            let (background, hover, active) = match self.variant {
                Variant::Primary | Variant::Danger => (
                    outline_color.opacity(0.08),
                    outline_color.opacity(0.13),
                    outline_color.opacity(0.20),
                ),
                _ => (background, hover, active),
            };
            (background, outline_color, hover, active)
        } else {
            (background, foreground, hover, active)
        };
        let height = if self.small { 24. } else { 28. };
        let icon = if self.loading {
            Some(Icon::new(IconName::Loader))
        } else {
            self.icon
        };
        let content = div()
            .flex()
            .items_center()
            .gap(px(window.rem_size() / px(1.) * 0.25 * self.scale))
            .when(self.content_full_width, |content| {
                content.w_full().min_w_0()
            })
            .when_some(icon, |this, icon| {
                this.child(icon.small().scale(self.scale))
            })
            .when_some(self.label.clone(), |this, label| this.child(label))
            .children(self.children);
        let semantic_label = self
            .accessibility_label
            .or_else(|| self.label.clone())
            .or_else(|| self.tooltip.clone());
        self.base
            .when_some(semantic_label, |button, label| {
                button.accessibility_label(label)
            })
            .key_context("EditorActivate")
            .disabled(self.disabled || self.loading)
            .h(px(height * self.scale))
            .min_w(px(if self.compact { height } else { 48. } * self.scale))
            .px(px(if self.compact { 4. } else { 10. } * self.scale))
            .rounded(palette.radius * self.scale)
            .when(self.outline, |button| {
                button
                    .border(px(self.scale))
                    .border_color(outline_color.opacity(0.65))
            })
            .bg(if self.selected {
                palette.list_active
            } else {
                background
            })
            .text_color(foreground)
            .text_size(px(if self.small { 12. } else { 13. } * self.scale))
            .cursor_pointer()
            .hover(|style| style.bg(hover))
            .active(|style| style.bg(active))
            .styles(|styles| styles.disabled(|style| style.opacity(0.5)))
            .child(content)
            .refine_style(&self.style)
            .when_some(self.reveal_group, |button, group| {
                button
                    .opacity(0.)
                    .group_hover(group, |style| style.opacity(1.))
                    .focus_visible(|style| style.opacity(1.))
            })
            .when_some(self.tooltip, |this, tooltip| {
                this.tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
            })
    }
}
