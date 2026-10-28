//! Return exact Resize admission to its native owner, without retrying or editing actor input.
use super::*;
use std::{cell::Cell, rc::Rc};

impl Worker {
    /// The normal actor retains its original epoch; hidden or retired surfaces cannot be revived.
    pub(in crate::extensions) fn resize_sink(
        &self,
        plugin: String,
        panel: String,
        epoch: u64,
        visible: Rc<Cell<bool>>,
    ) -> Option<crate::ui::plugin::ResizeSink> {
        #[cfg(test)]
        if self.recording {
            return None;
        }
        let tx = self.tx.clone();
        Some(Rc::new(move |event, _| {
            let (reply, receipt) = futures::channel::oneshot::channel();
            if visible.get() {
                let _ = tx.send(Work::CanvasResize {
                    plugin: plugin.clone(),
                    epoch,
                    panel: panel.clone(),
                    event,
                    reply,
                });
            } else {
                let _ = reply.send(false);
            }
            receipt
        }))
    }
}

/// Only a typed stale rejection permits the native owner to resample. Malformed input and guest
/// failures keep ordinary diagnostics; neither retirement nor success can start a retry loop.
pub(super) fn deliver(
    manager: &mut Manager,
    plugin: &str,
    expected_epoch: u64,
    current_epoch: u64,
    panel: String,
    event: ui::UiEvent,
    reply: futures::channel::oneshot::Sender<bool>,
) -> anyhow::Result<()> {
    if expected_epoch != current_epoch {
        let _ = reply.send(false);
        return Ok(());
    }
    let result = if crate::ui::plugin::is_canvas_resize(&event.action) {
        manager.event(plugin, Some(panel), api::Notification::Ui(event))
    } else {
        Err(api::Failure::new(
            api::ErrorCode::InvalidRequest,
            "Resize delivery accepts only canvas dimensions",
        )
        .into())
    };
    let stale = result.as_ref().err().is_some_and(|error| {
        error
            .downcast_ref::<api::Failure>()
            .is_some_and(|failure| failure.code == api::ErrorCode::StaleRevision)
    });
    let _ = reply.send(stale);
    if stale {
        manager.runtime_logs().append(
            plugin,
            plugin_runtime::logs::LogLevel::Info,
            "host.ui.stale",
            format!("{:#}", result.as_ref().unwrap_err()),
        );
        Ok(())
    } else {
        result
    }
}
