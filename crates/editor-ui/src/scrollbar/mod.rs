//! Scrollbar paint states from the editor theme over gpui-base scrolling.

use crate::theme::ActiveTheme as _;
use crate::theme::Theme;
use gpui_base::{ScrollbarHandle, ScrollbarMode, ScrollbarStyles};
use gpui_kit::px;
use gpui_kit::{App, Global, Hsla, Pixels, Subscription};
use plugin_schema::ThemeComponent;
use std::time::Duration;

use crate::theme::component_styles;

mod motion;
mod native;
#[cfg(test)]
mod tests;

pub use motion::Scrollbar;
pub use native::native_editor;
pub(super) use native::native_textarea;
pub(crate) use native::with_external_scrollbars;

/// Match the tree's four-pixel padding when an editor or canvas viewport reaches its border.
const VIEWPORT_EDGE_INSET: Pixels = px(4.);

/// Shared paint states account for the viewport's existing padding without doubling the edge gap.
fn scrollbar_styles(
    normal: Hsla,
    hover: Hsla,
    active: Hsla,
    inset: Pixels,
    visible: bool,
) -> ScrollbarStyles {
    // Include the clearance in the hit target so the entire expanded thumb remains draggable.
    let track_width = px(8.) + inset;
    ScrollbarStyles::default()
        // Keep the hit target fixed while the thumb expands on hover or drag.
        .track(|track| track.width(track_width))
        .track_hover(|track| track.width(track_width))
        .track_active(|track| track.width(track_width))
        .thumb(|thumb| {
            thumb
                .bg(normal.opacity(0.55))
                .width(px(if visible { 6. } else { 0. }))
                .inset(inset)
                .radius(px(3.))
        })
        .thumb_hover(|thumb| {
            thumb
                .bg(hover.opacity(0.7))
                .width(px(if visible { 8. } else { 0. }))
                .inset(inset)
                .radius(px(4.))
        })
        .thumb_active(|thumb| {
            thumb
                .bg(active.opacity(0.8))
                .width(px(if visible { 8. } else { 0. }))
                .inset(inset)
                .radius(px(4.))
        })
}

/// Hold one observer so local palette synchronization cannot reset the embedded editor's skin.
struct ScrollbarProjection {
    _subscription: Subscription,
}

impl Global for ScrollbarProjection {}

/// Install after runtime colors are published; refresh the projection after each theme update.
pub fn install_scrollbar_theme(cx: &mut App) {
    if !cx.has_global::<ScrollbarProjection>() {
        let subscription = cx.observe_global::<Theme>(sync_scrollbar_theme);
        cx.set_global(ScrollbarProjection {
            _subscription: subscription,
        });
    }
    sync_scrollbar_theme(cx);
}

/// Base animates thickness only; the local activity lease supplies the shared visibility hold.
fn sync_scrollbar_theme(cx: &mut App) {
    let styles = resolved_scrollbar_styles(cx, VIEWPORT_EDGE_INSET, true);
    let theme = gpui_base::Theme::global_mut(cx);
    let scrollbar = theme.scrollbar.clone();
    let motion = scrollbar
        .motion()
        .with_idle(motion::IDLE)
        .with_enter(Duration::ZERO)
        .with_exit(Duration::ZERO)
        .with_expand(motion::TRANSITION);
    theme.scrollbar = scrollbar
        .with_mode(ScrollbarMode::Always)
        .with_motion(motion)
        .with_styles(styles);
}

/// Resolve the same colors and opacity for standalone controls and native editor scrollbars.
fn resolved_scrollbar_styles(cx: &App, inset: Pixels, visible: bool) -> ScrollbarStyles {
    let palette = cx.theme();
    let styles = component_styles(cx, ThemeComponent::Scrollbar);
    scrollbar_styles(
        styles.base.background.unwrap_or(palette.primary),
        styles.hover.background.unwrap_or(palette.primary),
        styles.active.background.unwrap_or(palette.primary),
        inset,
        visible,
    )
}

/// Trees/lists already inset their scroll-handle bounds; retain that existing edge clearance.
#[track_caller]
pub fn vertical_scrollbar<H: ScrollbarHandle + Clone>(handle: &H, _: &App) -> Scrollbar {
    Scrollbar::new(handle, px(0.), false)
}

/// Canvas viewports need the same edge clearance as the embedded editor's global projection.
#[track_caller]
pub fn vertical_viewport_scrollbar<H: ScrollbarHandle + Clone>(handle: &H, _: &App) -> Scrollbar {
    Scrollbar::new(handle, VIEWPORT_EDGE_INSET, true)
}

/// Two-dimensional native viewports reuse Base's original offset authority and both draggable rails.
#[track_caller]
pub fn two_axis_viewport_scrollbars<H: ScrollbarHandle + Clone>(handle: &H, _: &App) -> Scrollbar {
    Scrollbar::new(handle, VIEWPORT_EDGE_INSET, true).axis(gpui_base::ScrollbarAxis::Both)
}
