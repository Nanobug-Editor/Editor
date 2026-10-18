//! A real tree sentinel activates through ordinary pointer input, followed by an independent UI timer.
use super::*;

/// Click only the painted timeout sentinel in this child and observe whether native dispatch returns.
pub(in super::super) async fn click(
    details: gpui_kit::AnyWindowHandle,
    cx: &mut gpui_kit::AsyncApp,
) {
    cx.background_executor()
        .timer(Duration::from_millis(200))
        .await;
    details
        .update(cx, |_, window, cx| {
            window.draw(cx).clear(cx);
            let sentinel = gpui_base::test_support::snapshots(window)
                .into_iter()
                .find(|node| {
                    node.role() == Some(gpui_kit::Role::TreeItem)
                        && node.visible()
                        && node.label().is_some_and(|label| label.contains("timeout"))
                })
                .expect("actual timeout retry row must be painted");
            let position = sentinel.bounds().center();
            eprintln!("native retry: dispatching pointer to actual timeout sentinel");
            window.dispatch_event(
                gpui_kit::PlatformInput::MouseDown(gpui_kit::MouseDownEvent {
                    position,
                    button: gpui_kit::MouseButton::Left,
                    modifiers: Default::default(),
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            );
            window.dispatch_event(
                gpui_kit::PlatformInput::MouseUp(gpui_kit::MouseUpEvent {
                    position,
                    button: gpui_kit::MouseButton::Left,
                    modifiers: Default::default(),
                    click_count: 1,
                }),
                cx,
            );
            eprintln!("native retry: pointer dispatch returned");
        })
        .unwrap();
    cx.background_executor()
        .timer(Duration::from_millis(50))
        .await;
    eprintln!("native retry: message loop timer returned");
}
