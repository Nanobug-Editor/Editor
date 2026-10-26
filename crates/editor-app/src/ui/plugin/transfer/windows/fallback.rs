//! Legacy image/file consumers retain GPUI's ordinary drop path, with native lists loaded after Drop.
use super::*;
use futures::StreamExt as _;

pub(super) struct Legacy {
    pub offer: Arc<drag::Offer>,
    pub position: drag::Point,
}
/// A finite passive inbox wakes only for real drops; it creates no polling or animation timer.
pub(super) fn start(
    window: gpui_kit::AnyWindowHandle,
    cx: &mut App,
) -> futures::channel::mpsc::Sender<Legacy> {
    let (sender, mut receiver) = futures::channel::mpsc::channel::<Legacy>(2);
    cx.spawn(async move |cx| {
        while let Some(drop) = receiver.next().await {
            let result = cx
                .background_executor()
                .spawn(async move {
                    drop.offer.read(
                        plugin_runtime::plugin_protocol::transfer::FILES_MIME,
                        256 * 1024,
                    )
                })
                .await;
            if let Ok(drag::Data::Files(paths)) = result {
                let _ = window.update(cx, |_, window, cx| {
                    let position = gpui_kit::point(
                        px(drop.position.x as f32 / window.scale_factor()),
                        px(drop.position.y as f32 / window.scale_factor()),
                    );
                    window.dispatch_event(
                        gpui_kit::PlatformInput::FileDrop(gpui_kit::FileDropEvent::Entered {
                            position,
                            paths: gpui_kit::ExternalPaths(paths.into_iter().collect()),
                        }),
                        cx,
                    );
                    // GPUI's drop hitboxes belong to the frame after Entered establishes active_drag.
                    // One passive post-render callback preserves that order without a polling timer.
                    window.refresh();
                    window.on_next_frame(move |window, cx| {
                        window.dispatch_event(
                            gpui_kit::PlatformInput::FileDrop(gpui_kit::FileDropEvent::Submit {
                                position,
                            }),
                            cx,
                        );
                    });
                });
            }
        }
    })
    .detach();
    sender
}
