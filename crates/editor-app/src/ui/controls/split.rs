//! Adjustable native panes keep Base capture/state while the local UI supplies divider appearance.
use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, div,
    prelude::FluentBuilder as _, px,
};
use std::rc::Rc;

/// Render portable children with their declared initial axis extent. Base retains resize state;
/// explicit extents stop growing with flexible peers but remain adjustable by the native divider.
pub(crate) fn split_container(
    id: SharedString,
    vertical: bool,
    children: Vec<(Option<f32>, AnyElement)>,
    _: &App,
) -> AnyElement {
    let group = if vertical {
        gpui_base::v_resizable(id.clone())
    } else {
        gpui_base::h_resizable(id.clone())
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
