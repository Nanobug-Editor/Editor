//! Adjustable native panes keep Base capture/state while the local UI supplies divider appearance.
use crate::theme::ActiveTheme;
use gpui_kit::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, div,
    prelude::FluentBuilder as _, px,
};
use std::rc::Rc;
mod preferences;
pub use preferences::split_container_with_layout;
#[cfg(test)]
mod tests;

/// Render portable children with their declared initial axis extent. Base retains resize state;
/// explicit extents stop growing with flexible peers but remain adjustable by the native divider.
pub fn split_container(
    id: SharedString,
    vertical: bool,
    children: Vec<(Option<f32>, AnyElement)>,
    _: &App,
) -> AnyElement {
    render_split(id, vertical, children, None)
}

/// Share Base's native behavior between ordinary panes and plugin-owned preference projections.
fn render_split(
    id: SharedString,
    vertical: bool,
    children: Vec<(Option<f32>, AnyElement)>,
    binding: Option<(
        gpui_kit::Entity<gpui_base::ResizableState>,
        preferences::ResizeSink,
    )>,
) -> AnyElement {
    let group = if vertical {
        gpui_base::v_resizable(id.clone())
    } else {
        gpui_base::h_resizable(id.clone())
    };
    let group = if let Some((state, sink)) = binding {
        group
            .with_state(&state)
            .on_resize(move |native, window, cx| {
                let sizes = native
                    .read(cx)
                    .sizes()
                    .iter()
                    .map(|v| f32::from(*v))
                    .collect::<Vec<_>>();
                let total: f32 = sizes.iter().sum();
                if total.is_finite() && total > 0. && sizes.iter().all(|v| *v > 0.) {
                    sink(sizes.into_iter().map(|v| v / total).collect(), window, cx);
                }
            })
    } else {
        group
    };
    let mut group = group.with_handle_appearance(Rc::new(move |_, _, cx| {
        let selector = format!("plugin-split-divider-{id}");
        Some(
            div()
                .debug_selector(move || selector.clone().into())
                .bg(cx.theme().border)
                .when(vertical, |line| line.h(px(1.)).w_full())
                .when(!vertical, |line| line.w(px(1.)).h_full())
                .into_any_element(),
        )
    }));
    for (initial_size, child) in children {
        group = group.child(
            gpui_base::resizable_panel()
                .size_range(px(100.)..gpui_kit::Pixels::MAX)
                .when_some(initial_size, |panel, extent| {
                    panel.size(px(extent)).flex_none()
                })
                .child(child),
        );
    }
    group.into_any_element()
}
