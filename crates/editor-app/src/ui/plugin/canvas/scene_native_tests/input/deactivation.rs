//! A second owned native window observes real activation loss without touching other applications.
use super::*;

/// A separate root contains no plugin input, so it cannot blur a Canvas target through its tree.
struct ActivationPeer;
impl gpui_kit::Render for ActivationPeer {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui_kit::IntoElement {
        div().size_full()
    }
}

/// Open and activate a small owned HWND; the caller must close it before application shutdown.
pub(in crate::ui::plugin::canvas::scene_native_tests) fn open(
    cx: &mut App,
) -> anyhow::Result<gpui_kit::AnyWindowHandle> {
    Ok(cx
        .open_window(
            gpui_kit::WindowOptions {
                focus: true,
                window_bounds: Some(gpui_kit::WindowBounds::Windowed(Bounds::new(
                    point(px(500.), px(100.)),
                    size(px(96.), px(64.)),
                ))),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|_| ActivationPeer);
                cx.new(|cx| gpui_base::Root::new(view, window, cx))
            },
        )?
        .into())
}
