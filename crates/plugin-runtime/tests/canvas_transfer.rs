//! Actual guests use the public Manager and its sealed native request boundary for interchange.
use plugin_runtime::{
    EditorRequest, Manager, Package,
    plugin_protocol::{
        Environment, api,
        transfer::{ClipboardOperation, Format},
        ui,
    },
};
use std::time::{Duration, Instant};
#[path = "support/packages.rs"]
mod test_packages;

/// Keep the ordinary scope/profile alive until its component and native request leases retire.
struct Harness {
    manager: Manager,
    _profile: tempfile::TempDir,
    _workspace: tempfile::TempDir,
}
impl Harness {
    /// Candidate overrides never replace frozen historical A/B package inputs.
    fn open() -> Self {
        let package = std::env::var_os("NANOBUG_CANVAS_TRANSFER_PACKAGE").map_or_else(
            || test_packages::package("canvas-flow-1.2.0"),
            |path| Package::read(std::path::Path::new(&path)).unwrap(),
        );
        let profile = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        let mut manager = Manager::open(
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
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            manager.poll();
            if let api::RequestUpdate::Completed { result } = demand.status() {
                result.unwrap();
                break;
            }
            assert!(
                Instant::now() < deadline,
                "interchange guest demand timed out"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        Self {
            manager,
            _profile: profile,
            _workspace: workspace,
        }
    }
    /// The guest reports only its public host call result in an ordinary declared text node.
    fn call(&mut self, operation: api::Operation) -> Result<api::Value, api::Failure> {
        self.manager
            .invoke_command(
                "canvas-flow",
                "exchange-probe",
                serde_json::to_value(operation).unwrap(),
            )
            .unwrap();
        let mut text = None;
        self.manager.live["canvas-flow"].views["diagram"]
            .root
            .visit(&mut |node| {
                if node.id == "exchange"
                    && let ui::Kind::Text { text: value } = &node.kind
                {
                    text = Some(value.clone());
                }
            });
        serde_json::from_str(&text.expect("public guest exchange observation")).unwrap()
    }
    /// Host adapters receive the same admitted, runtime-issued request as the native editor.
    fn clipboard(&mut self, operation: ClipboardOperation) -> EditorRequest {
        assert!(matches!(
            self.call(api::Operation::Editor {
                operation: api::EditorOperation::Clipboard { operation },
                timeout_ms: 30_000
            })
            .unwrap(),
            api::Value::Accepted(_)
        ));
        self.manager.take_editor_requests().pop().unwrap().1
    }
}

/// Count only external-provider reads; metadata and drag motion must never invoke this system boundary.
struct CountingDrop(std::sync::Arc<std::sync::atomic::AtomicUsize>);
impl plugin_runtime::TransferReader for CountingDrop {
    fn read(&self, _mime: &str) -> Result<plugin_runtime::HostTransferData, api::Failure> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        Ok(plugin_runtime::HostTransferData::Bytes(
            plugin_runtime::plugin_protocol::transfer::Item {
                mime: "text/html".into(),
                bytes: b"<table><tr><td>inert</td></tr></table>".to_vec(),
            },
        ))
    }
}

#[path = "canvas_transfer/clipboard.rs"]
mod clipboard;
#[path = "canvas_transfer/drops.rs"]
mod drops;
#[path = "canvas_transfer/imports.rs"]
mod imports;
#[path = "canvas_transfer/sources.rs"]
mod sources;
