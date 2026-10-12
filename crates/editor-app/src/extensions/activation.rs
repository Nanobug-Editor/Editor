//! Actual native visibility consumes a declared view; default layout flags are only metadata.
use super::*;
use gpui_kit::canvas;
use plugin_runtime::{Availability, DemandReason};
use worker::demands::DemandWait;

/// A view owns one wait and one opened notification for each exact live owner.
#[derive(Default)]
pub(super) struct ViewDemand {
    /// Deferred paint callbacks cannot revive a wait withdrawn by hide, selection or removal.
    generation: u64,
    availability: Option<Availability>,
    pending: Option<DemandWait>,
    ready: Option<plugin_runtime::ActivationReceipt>,
    opened: Option<String>,
}

impl ViewDemand {
    /// Hiding/switching/closing withdraws the original waiter, without stopping another caller.
    pub fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.pending.take();
        self.ready.take();
        self.availability = None;
    }
}

impl ExtensionPanel {
    /// Explicit show and nonzero body paint are demand; dock selection metadata alone is not.
    pub(super) fn demand_view(&self) {
        if !self.visible.get() {
            return;
        }
        let (Some(plugin), Some(_panel)) = (&self.active, &self.surface_id) else {
            return;
        };
        let availability = self
            .worker
            .state
            .lock()
            .unwrap()
            .availabilities
            .get(plugin)
            .cloned();
        let Some(availability) = availability else {
            return;
        };
        self.demand_view_at(availability);
    }

    /// An accepted command reveals with its original metadata identity, never a replacement's.
    pub(super) fn demand_view_at(&self, availability: Availability) {
        let Some(panel) = &self.surface_id else {
            return;
        };
        let mut demand = self.view_demand.borrow_mut();
        if demand.availability.as_ref() == Some(&availability) {
            return;
        }
        demand.pending.take();
        demand.ready.take();
        demand.availability = Some(availability.clone());
        if let Ok(wait) = self
            .worker
            .demand(availability, DemandReason::View(panel.clone()), None)
        {
            demand.pending = Some(wait);
        }
    }

    /// Deliver Opened only after the original receipt's owner is present in the current publication.
    /// First birth retains this waiter; a metadata replacement cancels it instead of retargeting.
    pub(super) fn poll_view_demand(&mut self, cx: &mut Context<Self>) {
        let Some(plugin) = self.active.clone() else {
            return;
        };
        let mut demand = self.view_demand.borrow_mut();
        let state = self.worker.state.lock().unwrap();
        if demand
            .availability
            .as_ref()
            .is_some_and(|original| state.availabilities.get(&plugin) != Some(original))
        {
            demand.cancel();
            return;
        }
        if let Some(result) = demand.pending.as_mut().and_then(DemandWait::try_take) {
            demand.pending.take();
            match result {
                Ok(receipt) => demand.ready = Some(receipt),
                Err(error) => {
                    self.preview_error = Some(error.to_string());
                    cx.notify();
                }
            }
        }
        let Some(receipt) = demand.ready.as_ref() else {
            return;
        };
        let Some(owner) = receipt.owner().map(str::to_owned) else {
            demand.ready.take();
            return;
        };
        if demand.opened.as_ref() == Some(&owner) {
            demand.ready.take();
            return;
        }
        // A receipt can precede its view publication by one actor turn; retain it until then.
        let Some(epoch) = state.owner_epoch(&plugin, &owner) else {
            return;
        };
        demand.opened = Some(owner);
        demand.ready.take();
        let _ = self.worker.tx.send(Work::Event(
            plugin,
            epoch,
            self.surface_id.clone(),
            PluginEvent::Command {
                id: "panel.opened".into(),
                arguments: None,
                context: None,
            },
        ));
    }

    /// A transparent canvas demands only a painted body with positive area inside the current mask.
    /// Defer the entity update until paint has released its borrow; cached render calls do no work.
    pub(super) fn view_demand_probe(&self, cx: &Context<Self>) -> AnyElement {
        let owner = cx.entity().downgrade();
        let generation = self.view_demand.borrow().generation;
        canvas(
            |_, _, _| (),
            move |bounds, _, window, cx| {
                // GPUI still invokes canvas paint outside a scroll clip. Its mask already intersects
                // all parent clips and the window viewport, so layout size alone cannot prove visibility.
                let visible = bounds.intersect(&window.content_mask().bounds);
                if visible.size.width > px(0.) && visible.size.height > px(0.) {
                    let owner = owner.clone();
                    cx.defer(move |cx| {
                        let _ = owner.update(cx, |panel, _| {
                            if panel.visible.get()
                                && panel.view_demand.borrow().generation == generation
                            {
                                panel.demand_view();
                            }
                        });
                    });
                }
            },
        )
        .absolute()
        .inset_0()
        .size_full()
        .into_any_element()
    }
}
