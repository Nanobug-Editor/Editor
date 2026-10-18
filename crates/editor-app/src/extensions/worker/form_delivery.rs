//! Private form receipts report actual public Manager admission without relaxing its scene gate.
use super::*;
use crate::ui::plugin::{FormChangeSink, FormReceipt};
use rust_i18n::t;
use std::{cell::Cell, rc::Rc};

#[cfg(test)]
#[path = "form_delivery_tests.rs"]
mod tests;

impl Worker {
    /// Background surfaces receive actual actor receipts. The existing inert recorded adapter
    /// continues its ordinary Event transport instead of fabricating successful Manager admission.
    pub(in crate::extensions) fn form_change_sink(
        &self,
        plugin: String,
        panel: String,
        epoch: u64,
        visible: Rc<Cell<bool>>,
    ) -> Option<FormChangeSink> {
        #[cfg(test)]
        if self.recording {
            return None;
        }
        let tx = self.tx.clone();
        Some(Rc::new(move |event, _| {
            let (reply, receipt) = futures::channel::oneshot::channel();
            if !visible.get() {
                let _ = reply.send(FormReceipt::Retired);
            } else {
                // A disconnected actor drops the sender; the native waiter treats that as retirement.
                let _ = tx.send(Work::FormChange {
                    plugin: plugin.clone(),
                    epoch,
                    panel: panel.clone(),
                    event,
                    reply,
                });
            }
            receipt
        }))
    }
}

/// A stale Change is rejected first and only its native owner can construct a new scene-bound one.
/// Retired incarnations and other failures are terminal; no Click/Submit can use this retry channel.
pub(super) fn deliver(
    manager: &mut Manager,
    plugin: &str,
    expected_epoch: u64,
    current_epoch: u64,
    panel: String,
    event: ui::UiEvent,
    reply: futures::channel::oneshot::Sender<FormReceipt>,
) -> anyhow::Result<()> {
    if expected_epoch != current_epoch {
        let _ = reply.send(FormReceipt::Retired);
        manager.runtime_logs().append(
            plugin,
            plugin_runtime::logs::LogLevel::Info,
            "host.ui.retired",
            t!("plugins.log_retired_callback").to_string(),
        );
        return Ok(());
    }
    let result = if matches!(event.action, ui::Action::Change(_)) {
        manager.event(plugin, Some(panel), api::Notification::Ui(event))
    } else {
        Err(api::Failure::new(
            api::ErrorCode::InvalidRequest,
            "Form delivery accepts only Change",
        )
        .into())
    };
    match result {
        Ok(()) => {
            let _ = reply.send(FormReceipt::Delivered);
            Ok(())
        }
        Err(error)
            if error
                .downcast_ref::<api::Failure>()
                .is_some_and(|failure| failure.code == api::ErrorCode::StaleRevision) =>
        {
            let _ = reply.send(FormReceipt::Stale);
            manager.runtime_logs().append(
                plugin,
                plugin_runtime::logs::LogLevel::Info,
                "host.ui.stale",
                format!("{error:#}"),
            );
            Ok(())
        }
        Err(error) => {
            let _ = reply.send(FormReceipt::Rejected);
            Err(error)
        }
    }
}
