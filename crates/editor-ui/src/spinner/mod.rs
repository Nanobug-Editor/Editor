//! Local loading motion around the editor's SVG icon.

use std::time::Duration;

use gpui_kit::{
    Animation, AnimationExt as _, App, IntoElement, ParentElement, RenderOnce, Transformation,
    Window, div, percentage,
};

use super::Icon;
use gpui_kit::assets::IconName;

/// Local loading motion around an immutable SVG icon.
#[derive(IntoElement)]
pub struct Spinner {
    icon: Icon,
    small: bool,
}

impl Spinner {
    /// Use the shared Loader icon with the default local size.
    pub fn new() -> Self {
        Self {
            icon: Icon::new(IconName::Loader),
            small: false,
        }
    }

    /// Animate the supplied icon instead of the catalog Loader.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = icon;
        self
    }

    /// Use the compact icon size for the loading animation.
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }
}

impl RenderOnce for Spinner {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div().child(
            (if self.small {
                self.icon.small()
            } else {
                self.icon
            })
            .with_animation(
                "editor-spinner",
                Animation::new(Duration::from_millis(800)).repeat(),
                |icon, delta| icon.transform(Transformation::rotate(percentage(delta))),
            ),
        )
    }
}
