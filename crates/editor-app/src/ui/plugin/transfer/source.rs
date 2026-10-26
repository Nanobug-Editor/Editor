//! Native source gestures borrow one actual press and the original actor-issued transient data lease.
use super::*;
use gpui_kit::{App, MouseButton, MouseMoveEvent, MouseUpEvent, Pixels, Point, Window, px};
use platform_windows::drag;
use plugin_runtime::plugin_protocol::transfer::{DragOutcome, MAX_BYTES};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// One pending native left-button press; moving or hiding never rebinds it to another source node.
struct Press {
    address: Address,
    expected: plugin_runtime::plugin_protocol::transfer::DragSource,
    start: Point<Pixels>,
    sink: Sink,
    alive: Arc<AtomicBool>,
    owner: isize,
    /// One exact held press, independent of the semantic data lease retained after a successful copy.
    gesture: Arc<AtomicBool>,
}
#[derive(Default)]
pub(super) struct Source(
    Rc<RefCell<Option<Press>>>,
    Rc<RefCell<Option<Arc<AtomicBool>>>>,
);
impl Source {
    pub(super) fn press(
        &self,
        scope: &windows::Scope,
        address: Address,
        expected: plugin_runtime::plugin_protocol::transfer::DragSource,
        start: Point<Pixels>,
    ) {
        self.clear();
        let Some((sink, alive, owner)) = scope.source_guard(&address, &expected) else {
            return;
        };
        let gesture = Arc::new(AtomicBool::new(true));
        *self.1.borrow_mut() = Some(gesture.clone());
        *self.0.borrow_mut() = Some(Press {
            address,
            expected,
            start,
            sink,
            alive,
            owner,
            gesture,
        });
    }
    pub(super) fn clear(&self) {
        self.0.borrow_mut().take();
        if let Some(gesture) = self.1.borrow_mut().take() {
            gesture.store(false, Ordering::Release);
        }
    }
    pub(super) fn listen(&self, window: &mut Window) {
        let state = self.0.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
            if !phase.capture() || event.pressed_button != Some(MouseButton::Left) {
                return;
            }
            let ready = state.borrow().as_ref().is_some_and(|press| {
                let delta = event.position - press.start;
                delta.x.abs() + delta.y.abs() >= px(4.)
            });
            if ready && let Some(press) = state.borrow_mut().take() {
                if press.alive.load(Ordering::Acquire) {
                    start(press, cx);
                    cx.stop_propagation();
                }
            }
        });
        let state = self.0.clone();
        let pending = self.1.clone();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, _| {
            if phase.capture() && event.button == MouseButton::Left {
                state.borrow_mut().take();
                if let Some(gesture) = pending.borrow_mut().take() {
                    gesture.store(false, Ordering::Release);
                }
            }
        });
    }
}
/// Native callbacks cannot fabricate source data: the actor returns only its original admitted resource.
fn start(press: Press, cx: &mut App) {
    let (reply, receiver) = futures::channel::oneshot::channel();
    if !(press.sink)(Ingress::StartDrag {
        address: press.address,
        expected: press.expected,
        alive: press.alive.clone(),
        reply,
    }) {
        return;
    }
    cx.spawn(async move |cx| {
        let Ok(Ok(resource)) = receiver.await else {
            return;
        };
        let handle = resource.metadata().handle;
        let result = if resource.is_live() && press.gesture.load(Ordering::Acquire) {
            cx.background_executor()
                .spawn(async move {
                    if !press.gesture.load(Ordering::Acquire) {
                        return Ok(drag::Outcome::Cancelled);
                    }
                    drag::start(press.owner, Arc::new(Reader(resource)), press.alive)
                })
                .await
                .map(|outcome| match outcome {
                    drag::Outcome::Dropped => DragOutcome::Dropped,
                    drag::Outcome::Cancelled => DragOutcome::Cancelled,
                })
        } else {
            Ok(DragOutcome::Cancelled)
        };
        if let Err(error) = &result {
            eprintln!("Native plugin drag failed: {error}");
        }
        (press.sink)(Ingress::FinishDrag {
            handle,
            outcome: result.unwrap_or(DragOutcome::Cancelled),
        });
    })
    .detach();
}
/// OS GetData remains lazy and observes owner/source/target revocation even after queued completion.
struct Reader(Arc<plugin_runtime::TransferOfferResource>);
impl drag::Provider for Reader {
    fn is_live(&self) -> bool {
        self.0.is_live()
    }
    fn formats(&self) -> Vec<String> {
        self.0
            .metadata()
            .formats
            .into_iter()
            .map(|format| format.mime)
            .collect()
    }
    fn read(&self, mime: &str) -> Result<drag::Data, drag::Error> {
        let data = self
            .0
            .read(&[mime.into()])
            .map_err(|_| drag::Error::Changed)?;
        match data {
            plugin_runtime::HostTransferData::Bytes(item) if item.bytes.len() <= MAX_BYTES => {
                if mime == "image/png"
                    && crate::ui::plugin::bitmap::input_format(&item.bytes)
                        .map_err(|_| drag::Error::Invalid)?
                        != plugin_runtime::plugin_protocol::api::ImageFormat::Png
                {
                    return Err(drag::Error::Invalid);
                }
                Ok(drag::Data::Bytes(item.bytes))
            }
            _ => Err(drag::Error::Invalid),
        }
    }
}
