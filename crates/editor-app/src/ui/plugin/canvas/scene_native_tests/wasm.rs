//! Actual independent guest output supplies native acceptance pixels, not a host-built substitute.
use super::*;
pub(super) mod editable;
pub(super) mod files;
#[path = "../../../../../../plugin-runtime/tests/support/packages.rs"]
mod test_packages;

/// Keep package authority and temporary paths alive through the last native window callback.
pub(super) struct InstalledFlow {
    /// Immutable public document after the guest's own zoom and pan commands.
    pub document: Document,
    /// Retained owner is retired before the disposable profile and workspace are removed.
    manager: plugin_runtime::Manager,
    /// Ordinary package/panel identifiers used only by this fixture driver.
    plugin: String,
    panel: String,
    /// Native destination serials are correlated only to this fixture's public Manager-issued offers.
    transfers: BTreeMap<u64, protocol::api::ResourceHandle>,
    _profile: tempfile::TempDir,
    _workspace: tempfile::TempDir,
}

/// Install through the public manager with only the permissions declared by this ordinary guest.
pub(super) fn install() -> InstalledFlow {
    let package = test_packages::package("canvas-flow-1.0.0");
    let mut installed = open(package, "canvas-flow", "diagram");
    // These declared noncoordinate buttons need no fabricated presentation receipt.
    for node in ["zoom", "pan"] {
        let revision = installed.document.revision;
        installed
            .manager
            .event(
                "canvas-flow",
                Some("diagram".into()),
                protocol::api::Notification::Ui(ui::UiEvent {
                    revision,
                    node: node.into(),
                    action: ui::Action::Click,
                }),
            )
            .unwrap();
        installed.document = installed.manager.live["canvas-flow"].views["diagram"]
            .as_ref()
            .clone();
    }
    installed
}
/// C uses its own candidate/frozen package without replacing immutable historical B inputs.
pub(super) fn install_transfer() -> InstalledFlow {
    let package = std::env::var_os("NANOBUG_CANVAS_TRANSFER_PACKAGE").map_or_else(
        || test_packages::package("canvas-flow-1.2.0"),
        |path| plugin_runtime::Package::read(std::path::Path::new(&path)).unwrap(),
    );
    open(package, "canvas-flow", "diagram")
}
/// Load independent B packages through their normal package reader, including candidate overrides.
pub(super) fn install_editable(kind: &str) -> InstalledFlow {
    let (id, version, panel, variable) = match kind {
        "flow" => (
            "canvas-flow",
            "1.1.0",
            "diagram",
            "NANOBUG_CANVAS_EDITABLE_PACKAGE",
        ),
        "table" => (
            "canvas-table",
            "1.0.0",
            "table",
            "NANOBUG_CANVAS_TABLE_PACKAGE",
        ),
        _ => panic!("unknown native editable fixture"),
    };
    let package = std::env::var_os(variable).map_or_else(
        || test_packages::package(&format!("{id}-{version}")),
        |path| plugin_runtime::Package::read(std::path::Path::new(&path)).unwrap(),
    );
    open(package, id, panel)
}
/// Activate a real ordinary WASM view before opening its isolated native acceptance window.
fn open(package: plugin_runtime::Package, plugin: &str, panel: &str) -> InstalledFlow {
    let profile = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let mut manager = plugin_runtime::Manager::open(
        profile.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let availability = manager.availability(plugin).unwrap();
    let demand = manager
        .begin_demand(
            &availability,
            plugin_runtime::DemandReason::View(panel.into()),
            60_000,
        )
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    loop {
        manager.poll();
        if let protocol::api::RequestUpdate::Completed { result } = demand.status() {
            result.unwrap();
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "native canvas package activation timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let document = manager.live[plugin].views[panel].as_ref().clone();
    InstalledFlow {
        document,
        manager,
        plugin: plugin.into(),
        panel: panel.into(),
        transfers: BTreeMap::new(),
        _profile: profile,
        _workspace: workspace,
    }
}
impl InstalledFlow {
    /// This fixture driver invokes the same public native transfer APIs as the production actor.
    pub(super) fn native_source(
        &mut self,
        ingress: crate::ui::plugin::transfer::Ingress,
    ) -> anyhow::Result<()> {
        use crate::ui::plugin::transfer::Ingress;
        match ingress {
            Ingress::StartDrag {
                address,
                expected,
                alive,
                reply,
            } => {
                let _ = reply.send(self.manager.begin_drag_snapshot(
                    &self.plugin,
                    &self.panel,
                    address,
                    expected,
                    alive,
                ));
            }
            Ingress::FinishDrag { handle, outcome } => {
                self.manager
                    .finish_drag(&self.plugin, &self.panel, &handle, outcome)?;
            }
            Ingress::Enter {
                serial,
                address,
                expected,
                source,
                position,
            } => {
                let offer = self.manager.begin_transfer_snapshot(
                    &self.plugin,
                    &self.panel,
                    address,
                    expected,
                    source,
                    position,
                )?;
                self.transfers.insert(serial, offer.handle);
            }
            Ingress::Update {
                serial,
                address,
                phase,
                position,
            } => {
                if let Some(handle) = self.transfers.get(&serial).cloned() {
                    if matches!(
                        phase,
                        protocol::transfer::Phase::Drop
                            | protocol::transfer::Phase::Leave
                            | protocol::transfer::Phase::Cancel
                    ) {
                        self.transfers.remove(&serial);
                    }
                    self.manager.update_transfer_snapshot(
                        &self.plugin,
                        &self.panel,
                        &handle,
                        address,
                        phase,
                        position,
                    )?;
                }
            }
        }
        self.document = self.manager.live[&self.plugin].views[&self.panel]
            .as_ref()
            .clone();
        Ok(())
    }
    /// Forward original native envelopes to public Manager admission and observe guest publication.
    pub(super) fn deliver(&mut self, events: &[ui::UiEvent]) -> anyhow::Result<()> {
        for event in events {
            self.manager.event(
                &self.plugin,
                Some(self.panel.clone()),
                protocol::api::Notification::Ui(event.clone()),
            )?;
        }
        self.document = self.manager.live[&self.plugin].views[&self.panel]
            .as_ref()
            .clone();
        Ok(())
    }
}
