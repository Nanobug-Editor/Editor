//! Native gesture serials resolve only to actor-issued offer handles in their original instance/panel.
use super::*;
use crate::ui::plugin::transfer::Ingress;

/// Hover routes are bounded separately from post-drop resources, which stay owned by Manager.
#[derive(Default)]
pub(super) struct Routes(BTreeMap<(String, u64, u64), (String, api::ResourceHandle)>);
impl Routes {
    /// Called only after the actor checked its original instance epoch; no cold demand is started here.
    pub(super) fn deliver(
        &mut self,
        manager: &mut Manager,
        plugin: &str,
        panel: &str,
        epoch: u64,
        ingress: Ingress,
    ) -> anyhow::Result<()> {
        self.0
            .retain(|(owner, saved_epoch, _), _| owner != plugin || *saved_epoch == epoch);
        match ingress {
            Ingress::StartDrag {
                address,
                expected,
                alive,
                reply,
            } => {
                let result = manager.begin_drag_snapshot(plugin, panel, address, expected, alive);
                // A withdrawn GUI request cannot leave an unreachable source lease behind.
                if let Err(Ok(source)) = reply.send(result) {
                    let _ = manager.finish_drag(
                        plugin,
                        panel,
                        &source.metadata().handle,
                        transfer::DragOutcome::Cancelled,
                    );
                }
                Ok(())
            }
            Ingress::FinishDrag { handle, outcome } => manager
                .finish_drag(plugin, panel, &handle, outcome)
                .map_err(Into::into),
            Ingress::Enter {
                serial,
                address,
                expected,
                source,
                position,
            } => {
                if self.0.len() >= 16 {
                    anyhow::bail!("Native transfer route quota exceeded");
                }
                let offer = manager
                    .begin_transfer_snapshot(plugin, panel, address, expected, source, position)?;
                self.0
                    .insert((plugin.into(), epoch, serial), (panel.into(), offer.handle));
                Ok(())
            }
            Ingress::Update {
                serial,
                address,
                phase,
                position,
            } => {
                let key = (plugin.into(), epoch, serial);
                let Some((owner_panel, handle)) = self.0.get(&key).cloned() else {
                    return Ok(());
                };
                if owner_panel != panel {
                    anyhow::bail!("Native transfer belongs to another panel");
                }
                if matches!(
                    phase,
                    transfer::Phase::Drop | transfer::Phase::Cancel | transfer::Phase::Leave
                ) {
                    self.0.remove(&key);
                }
                manager
                    .update_transfer_snapshot(plugin, panel, &handle, address, phase, position)
                    .map_err(Into::into)
            }
        }
    }
}
