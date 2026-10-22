//! Local tree-row appearance; gpui-base owns selection, keyboard navigation and virtualization.
use super::Icon;
mod icon;
use crate::theme::ActiveTheme as _;
use crate::{theme::component_styles, typography};
use gpui_kit::StatefulInteractiveElement as _;
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Div, InteractiveElement, IntoElement, ParentElement, Stateful, Styled, div, px,
};
pub use icon::TreeRowIcon;
use plugin_schema::ThemeComponent;

/// Optional inline row content; native owners retain callbacks and checkbox state.
#[derive(Default)]
pub struct TreeRowExtras {
    /// Leading control appears after disclosure and before the file icon.
    pub leading: Option<gpui_kit::AnyElement>,
    /// Secondary count or status beside the truncated label, using the current muted color.
    pub suffix: String,
}

/// Select the compact Explorer theme contract without changing configuration-tree presentation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TreeRowAppearance {
    Standard,
    Explorer,
}

/// Presentation state independent of Base's selection and the provider's business actions.
#[derive(Clone, Copy, Default)]
pub struct TreeRowState {
    /// The native tree's committed selection retains its selected color under the pointer.
    pub selected: bool,
    /// Keep the hover color while this row owns an open context menu.
    pub context_menu: bool,
}

impl From<bool> for TreeRowState {
    /// Existing callers supplying only selection retain ordinary pointer hover behavior.
    fn from(selected: bool) -> Self {
        Self {
            selected,
            context_menu: false,
        }
    }
}

impl TreeRowAppearance {
    /// Apply Explorer's live typography and selected/base tokens to a native row.
    fn apply(
        self,
        row: Stateful<Div>,
        depth: usize,
        state: TreeRowState,
        cx: &App,
    ) -> Stateful<Div> {
        let selected = state.selected;
        if self == Self::Standard {
            let hover = cx.theme().list_hover;
            return row
                .when(!selected && state.context_menu, |row| row.bg(hover))
                .when(!selected, |row| row.hover(move |row| row.bg(hover)));
        }
        let styles = component_styles(cx, ThemeComponent::ExplorerRow);
        let style = if selected {
            styles.selected
        } else {
            styles.base
        };
        let tree_style = component_styles(cx, ThemeComponent::ExplorerTree).base;
        let hover = styles.hover.background.unwrap_or(cx.theme().list_hover);
        let radius = styles.selected.radius_px.unwrap_or(5.);
        let highlighted = selected || state.context_menu;
        // Resolve the same theme tokens as Explorer, including live font sizing and selection.
        row.h_auto()
            .min_h(px(24.))
            .gap_1()
            .px(px(style.padding_x_px.unwrap_or(4.)))
            .py(px(style.padding_y_px.unwrap_or(0.3)))
            .pl(px(8. + depth as f32 * 14.))
            .rounded(px(if highlighted { radius } else { 0. }))
            // Match Explorer's background-only selection, without a leading accent border.
            .bg(if !selected && state.context_menu {
                hover
            } else {
                style.background.unwrap_or(cx.theme().background)
            })
            .when(!selected, |row| {
                row.hover(move |row| row.bg(hover).rounded(px(radius)))
            })
            .text_color(style.foreground.unwrap_or(cx.theme().foreground))
            .text_size(
                tree_style
                    .font_size_px
                    .map(px)
                    .unwrap_or(typography::font_size(cx)),
            )
            .font_family(cx.theme().mono_font_family.clone())
    }
}

/// Build an aligned native row identified by `id`, with `label`, `icon` and indentation `depth`.
/// `folder`/`expanded` control disclosure; `toggle` handles its click in the original window.
/// `state` accepts selection alone or a menu highlight, `invalid` marks an error, and `appearance`
/// resolves geometry/colors from `cx`. `extras` supplies optional leading controls and a muted suffix.
/// The returned stateful row leaves business selection to Base.
pub fn tree_row(
    id: String,
    label: String,
    depth: usize,
    icon: impl Into<TreeRowIcon>,
    folder: bool,
    expanded: bool,
    state: impl Into<TreeRowState>,
    invalid: bool,
    appearance: TreeRowAppearance,
    extras: TreeRowExtras,
    cx: &App,
    toggle: Option<std::rc::Rc<dyn Fn(&mut gpui_kit::Window, &mut App)>>,
) -> Stateful<Div> {
    let state = state.into();
    let disclosure = if folder {
        Icon::new(if expanded {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        })
        .size(px(12.))
        .into_any_element()
    } else {
        div().size(px(12.)).into_any_element()
    };
    let row = div()
        .id(id)
        .h(px(34.))
        .w_full()
        .flex()
        .items_center()
        .gap(px(5.))
        .pl(px(8. + depth as f32 * 14.))
        .pr(px(6.))
        .rounded(px(4.))
        .bg(if state.selected {
            cx.theme().accent
        } else {
            cx.theme().transparent
        })
        .text_color(if invalid {
            cx.theme().danger
        } else {
            cx.theme().foreground
        })
        .text_sm();
    appearance
        .apply(row, depth, state, cx)
        .child(
            div()
                .id("disclosure")
                // Explorer centers its 12px disclosure glyph in a 16px slot beside the icon.
                .size(px(if appearance == TreeRowAppearance::Explorer {
                    16.
                } else {
                    12.
                }))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .child(disclosure)
                .on_mouse_down(gpui_kit::MouseButton::Left, move |_, _, cx| {
                    if folder {
                        cx.stop_propagation();
                    }
                })
                .on_click(move |_, window, cx| {
                    if let Some(toggle) = &toggle {
                        cx.stop_propagation();
                        toggle(window, cx);
                    }
                }),
        )
        .children(extras.leading)
        .child(icon.into().element())
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap_1()
                .child(div().min_w_0().truncate().child(label))
                .when(!extras.suffix.is_empty(), |row| {
                    row.child(
                        div()
                            .flex_shrink_0()
                            .text_color(cx.theme().muted_foreground)
                            .child(extras.suffix),
                    )
                }),
        )
}
