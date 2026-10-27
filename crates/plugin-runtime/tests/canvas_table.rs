//! Actual table guest shares native text/semantics admission with an unrelated diagram guest.
use plugin_runtime::{
    Manager, Package,
    plugin_protocol::{Environment, api, ui},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
#[path = "support/packages.rs"]
mod test_packages;

/// Own a disposable ordinary workspace/profile until its actual WASM instance retires.
struct Table {
    manager: Manager,
    _profile: tempfile::TempDir,
    _workspace: tempfile::TempDir,
}
impl Table {
    /// Use the built independent package; candidate override never changes the historical input ledger.
    fn open() -> Self {
        Self::open_version("canvas-table-1.0.0")
    }
    /// C consumers use their own frozen package and never reinterpret the immutable B baseline.
    fn open_interchange() -> Self {
        Self::open_version("canvas-table-1.1.0")
    }
    fn open_version(version: &str) -> Self {
        let package = std::env::var_os("NANOBUG_CANVAS_TABLE_PACKAGE").map_or_else(
            || test_packages::package(version),
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
        let available = manager.availability("canvas-table").unwrap();
        let demand = manager
            .begin_demand(
                &available,
                plugin_runtime::DemandReason::View("table".into()),
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
            assert!(Instant::now() < deadline, "table guest demand timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
        Self {
            manager,
            _profile: profile,
            _workspace: workspace,
        }
    }
    /// Read only public published values.
    fn view(&self) -> Arc<ui::Document> {
        self.manager.live["canvas-table"].views["table"].clone()
    }
    /// Send ordinary native notifications through public Manager authority.
    fn send(&mut self, event: ui::UiEvent) -> anyhow::Result<()> {
        self.manager.event(
            "canvas-table",
            Some("table".into()),
            api::Notification::Ui(event),
        )
    }
    /// Admit exact current native presentation separately from publication.
    fn present(&mut self) -> Arc<ui::Document> {
        let document = self.view();
        let node = document.active_node("drawing").unwrap();
        let ui::Kind::Canvas(canvas) = &node.kind else {
            unreachable!()
        };
        let scene = canvas.scene.as_ref().unwrap();
        self.send(ui::UiEvent {
            revision: document.revision,
            node: "drawing".into(),
            action: ui::Action::Canvas(ui::CanvasEvent::ScenePresented {
                identity: scene.identity,
                scene_revision: scene.revision,
                source: document.source.clone(),
                file: document.file.clone(),
                projection: node.viewport.clone(),
                error: None,
            }),
        })
        .unwrap();
        document
    }
    /// Inspect actual guest-committed text/epoch without reading a native draft or domain undo stack.
    fn text(&self, index: usize) -> (String, u64) {
        let document = self.view();
        let ui::Kind::Canvas(canvas) = &document.active_node("drawing").unwrap().kind else {
            unreachable!()
        };
        let scene = canvas.scene.as_ref().unwrap();
        let target = &scene.targets[index];
        let binding = target.text_input.as_ref().unwrap();
        (
            scene
                .paragraph_text(&binding.paragraph, &binding.layout_key)
                .unwrap()
                .into(),
            target.epoch,
        )
    }
}
#[path = "canvas_table/interchange.rs"]
mod interchange;

/// One shared transport carries target identity, actual shown frame and public source evidence.
fn input(
    document: &ui::Document,
    target: &str,
    sequence: u64,
    kind: ui::CanvasInputKind,
) -> ui::UiEvent {
    let node = document.active_node("drawing").unwrap();
    let ui::Kind::Canvas(canvas) = &node.kind else {
        unreachable!()
    };
    let scene = canvas.scene.as_ref().unwrap();
    ui::UiEvent {
        revision: document.revision,
        node: "drawing".into(),
        action: ui::Action::Canvas(ui::CanvasEvent::Input {
            input: ui::CanvasInput {
                identity: scene.identity,
                presented_revision: scene.revision,
                target: target.into(),
                epoch: 1,
                sequence,
                source: document.source.clone(),
                file: document.file.clone(),
                projection: node.viewport.clone(),
                kind,
            },
        }),
    }
}

/// The guest edits an emoji cell and owns Undo, while readonly metadata and assistive SetValue stay scoped.
#[test]
#[ignore = "executes independent table WASM through public Manager; build package then run explicitly"]
fn canvas_table_edits_original_graphemes_and_keeps_domain_undo_in_guest() {
    let mut table = Table::open();
    let shown = table.present();
    let ui::Kind::Canvas(canvas) = &shown.active_node("drawing").unwrap().kind else {
        unreachable!()
    };
    let scene = canvas.scene.as_ref().unwrap();
    assert_eq!(scene.targets.len(), 6);
    assert_eq!(scene.semantics[0].row_count, Some(100000));
    table
        .send(input(
            &shown,
            "cell-1",
            1,
            ui::CanvasInputKind::Focus { focused: true },
        ))
        .unwrap();
    table
        .send(input(
            &shown,
            "cell-1",
            2,
            ui::CanvasInputKind::Text(ui::CanvasTextEdit {
                text_revision: 1,
                base_sequence: 0,
                range: ui::CanvasTextRange { start: 4, end: 15 },
                text: "中文".into(),
                selection: ui::CanvasTextRange { start: 10, end: 10 },
            }),
        ))
        .unwrap();
    assert_eq!(table.text(1).0, "e\u{301} 中文");
    assert_eq!(table.text(3).0, "42");
    table
        .send(input(
            &shown,
            "cell-3",
            3,
            ui::CanvasInputKind::A11y {
                semantic: "semantic-3".into(),
                action: ui::CanvasSemanticAction::SetValue,
                value: Some("99".into()),
            },
        ))
        .unwrap();
    assert_eq!(table.text(3), ("99".into(), 2));
    assert_eq!(table.text(1).0, "e\u{301} 中文");
    let current = table.view();
    table
        .send(ui::UiEvent {
            revision: current.revision,
            node: "undo".into(),
            action: ui::Action::Click,
        })
        .unwrap();
    assert_eq!(table.text(3), ("42".into(), 3));
    let current = table.view();
    table
        .send(ui::UiEvent {
            revision: current.revision,
            node: "undo".into(),
            action: ui::Action::Click,
        })
        .unwrap();
    assert_eq!(table.text(1), ("e\u{301} 👩‍💻".into(), 2));
    assert!(
        table
            .send(input(
                &shown,
                "cell-1",
                4,
                ui::CanvasInputKind::Focus { focused: true }
            ))
            .is_err(),
        "old cell callback cannot focus the replacement generation"
    );
    table.manager.uninstall("canvas-table", true).unwrap();
    assert!(!table.manager.live.contains_key("canvas-table"));
}
