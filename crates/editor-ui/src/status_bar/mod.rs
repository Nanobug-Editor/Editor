//! The editor status bar's alignment and density live in the local UI layer.

use gpui_base::h_flex;
use gpui_kit::{AnyElement, App, IntoElement, ParentElement, RenderOnce, Styled, Window, px};

/// Two-sided local status content with one shared alignment and density policy.
#[derive(IntoElement, Default)]
pub struct StatusBar {
    left: Vec<AnyElement>,
    right: Vec<AnyElement>,
}

impl StatusBar {
    /// Start an empty status bar with independent left and right content groups.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append child to the left-aligned group.
    pub fn left(mut self, child: impl IntoElement) -> Self {
        self.left.push(child.into_any_element());
        self
    }

    /// Append child to the right-aligned group.
    pub fn right(mut self, child: impl IntoElement) -> Self {
        self.right.push(child.into_any_element());
        self
    }
}

impl RenderOnce for StatusBar {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        h_flex()
            // Keep status contents vertically centered within the requested 32px bar.
            .h(px(32.))
            .w_full()
            .items_center()
            .gap_3()
            .px_2()
            .children(self.left)
            .child(h_flex().flex_1())
            .children(self.right)
    }
}
