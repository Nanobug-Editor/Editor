//! Actual independent guest output supplies native acceptance pixels, not a host-built substitute.
use super::*;
#[path = "../../../../../../plugin-runtime/tests/support/packages.rs"]
mod test_packages;

/// Keep package authority and temporary paths alive through the last native window callback.
pub(super) struct InstalledFlow {
    /// Immutable public document after the guest's own zoom and pan commands.
    pub document: Document,
    /// Retained owner is retired before the disposable profile and workspace are removed.
    _manager: plugin_runtime::Manager,
    _profile: tempfile::TempDir,
    _workspace: tempfile::TempDir,
}

/// Install through the public manager with only the permissions declared by this ordinary guest.
pub(super) fn install() -> InstalledFlow {
    let package = test_packages::package("canvas-flow-1.0.0");
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
    let availability = manager.availability("canvas-flow").unwrap();
    let demand = manager
        .begin_demand(
            &availability,
            plugin_runtime::DemandReason::View("diagram".into()),
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
    // These declared noncoordinate buttons need no fabricated presentation receipt.
    // The native test renders the resulting guest projection and checks its actual pixels.
    for node in ["zoom", "pan"] {
        let revision = manager.live["canvas-flow"].views["diagram"].revision;
        manager
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
    }
    let document = manager.live["canvas-flow"].views["diagram"]
        .as_ref()
        .clone();
    InstalledFlow {
        document,
        _manager: manager,
        _profile: profile,
        _workspace: workspace,
    }
}
