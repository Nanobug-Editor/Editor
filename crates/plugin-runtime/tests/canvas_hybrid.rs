//! Actual independent WASM output exercises shared UI/scene transaction admission and byte limits.
use plugin_runtime::{
    Manager, Package,
    plugin_protocol::{Environment, api, ui},
};
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// Keep the disposable owner directories alive until its real component retires.
struct Harness {
    manager: Manager,
    plugin: &'static str,
    panel: &'static str,
    _profile: tempfile::TempDir,
    _workspace: tempfile::TempDir,
}
impl Harness {
    /// Candidate paths are explicit; frozen historical Package inputs never change meaning.
    fn open() -> Self {
        Self::open_package("NANOBUG_CANVAS_EDITABLE_PACKAGE", "canvas-flow", "diagram")
    }
    /// Independent guests use identical admission; only their declared package/panel differs.
    fn open_package(variable: &str, plugin: &'static str, panel: &'static str) -> Self {
        let path =
            std::env::var_os(variable).expect("set the independently built D Package override");
        let package = Package::read(std::path::Path::new(&path)).unwrap();
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
        let availability = manager.availability(plugin).unwrap();
        let demand = manager
            .begin_demand(
                &availability,
                plugin_runtime::DemandReason::View(panel.into()),
                60_000,
            )
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            manager.poll();
            if let api::RequestUpdate::Completed { result } = demand.status() {
                result.unwrap();
                break;
            }
            assert!(Instant::now() < deadline, "Flow demand timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
        Self {
            manager,
            plugin,
            panel,
            _profile: profile,
            _workspace: workspace,
        }
    }
    /// Read the public accepted immutable snapshot, including its exact Arc identity.
    fn document(&self) -> Arc<ui::Document> {
        self.manager.live[self.plugin].views[self.panel].clone()
    }
    /// Fault payloads enter the same public command/guest-completion path as any plugin output.
    fn probe(&mut self, value: Value) -> anyhow::Result<()> {
        self.manager
            .invoke_command("canvas-flow", "publication-probe", value)
    }
    /// Every rejection must preserve the accepted Arc and the healthy component, with a typed cause.
    fn rejected(&mut self, value: Value, code: api::ErrorCode, message: &str) {
        let before = self.document();
        let error = self
            .probe(value)
            .err()
            .expect("scene transaction must reject");
        let failure = error
            .chain()
            .find_map(|cause| cause.downcast_ref::<api::Failure>())
            .unwrap_or_else(|| panic!("typed scene admission failure missing: {error:#}"));
        assert_eq!(failure.code, code, "{error:#}");
        assert!(
            failure.message.contains(message),
            "wrong rejection layer: {error:#}"
        );
        assert!(
            Arc::ptr_eq(&before, &self.document()),
            "one invalid peer changed accepted pixels"
        );
    }
}

/// Construct a public exact-base delta from the accepted snapshot, without borrowing private runtime state.
fn advance(document: &ui::Document) -> ui::CanvasScenePatch {
    let ui::Kind::Canvas(canvas) = &document.active_node("drawing").unwrap().kind else {
        panic!()
    };
    let scene = canvas.scene.as_ref().unwrap();
    ui::CanvasScenePatch {
        panel: "diagram".into(),
        canvas: "drawing".into(),
        base_ui_revision: document.revision,
        next_ui_revision: document.revision + 1,
        identity: scene.identity,
        base_revision: scene.revision,
        next_revision: scene.revision + 1,
        operations: vec![ui::CanvasSceneOperation::Advance],
        damage: None,
    }
}
/// Serialize the actual public transaction, never an internal host test object.
fn output(patches: Vec<ui::CanvasScenePatch>, peer: Option<api::ViewPatch>) -> Value {
    serde_json::to_value(api::Output {
        scene_patches: patches,
        view_patches: peer.into_iter().collect(),
        ..Default::default()
    })
    .unwrap()
}

/// Ordinary viewport changes and nested native owners use the reusable SDK helper through a real guest.
#[test]
#[ignore = "build Flow 1.3 through --plugin-cargo and set NANOBUG_CANVAS_EDITABLE_PACKAGE"]
fn retained_flow_publishes_local_ui_and_nested_canvas_without_full_scene_bytes() {
    let mut harness = Harness::open();
    let before = harness.document();
    let observations = harness.manager.runtime_observations();
    observations.start_capture();
    harness
        .manager
        .event(
            "canvas-flow",
            Some("diagram".into()),
            api::Notification::Ui(ui::UiEvent {
                revision: before.revision,
                node: "pan".into(),
                action: ui::Action::Click,
            }),
        )
        .unwrap();
    let after = harness.document();
    assert_eq!(
        after
            .active_node("drawing")
            .unwrap()
            .viewport
            .as_ref()
            .unwrap()
            .transform
            .unwrap()
            .x,
        20.
    );
    let bytes = observations
        .snapshot(None)
        .records
        .into_iter()
        .filter(|r| r.stage == plugin_runtime::observability::Stage::Publication)
        .filter_map(|r| r.bytes)
        .collect::<Vec<_>>();
    let full = serde_json::to_vec(&*before).unwrap().len() as u64;
    assert!(
        !bytes.is_empty() && bytes.iter().all(|bytes| *bytes < full),
        "actual encoded local publication must be smaller than its full base: {bytes:?}, full={full}"
    );
    harness
        .manager
        .event(
            "canvas-flow",
            Some("diagram".into()),
            api::Notification::Ui(ui::UiEvent {
                revision: after.revision,
                node: "nested".into(),
                action: ui::Action::Click,
            }),
        )
        .unwrap();
    let nested = harness.document();
    assert!(nested.active_node("nested-drawing").is_some());
    harness
        .manager
        .event(
            "canvas-flow",
            Some("diagram".into()),
            api::Notification::Ui(ui::UiEvent {
                revision: nested.revision,
                node: "pan".into(),
                action: ui::Action::Click,
            }),
        )
        .unwrap();
    assert!(harness.document().active_node("nested-drawing").is_some());
}

/// Conflicting peers, counterfeit bases and aggregate budgets fail before any live view replacement.
#[test]
#[ignore = "executes independently built Flow 1.3 public Output probes"]
fn retained_hybrid_rejects_invalid_peers_atomically_and_accepts_the_real_base() {
    let mut harness = Harness::open();
    let before = harness.document();
    let patch = advance(&before);
    harness.rejected(
        output(vec![patch.clone(), patch.clone()], None),
        api::ErrorCode::Conflict,
        "distinct Canvas",
    );
    let mut peer = patch.clone();
    peer.canvas = "other".into();
    peer.next_ui_revision += 1;
    harness.rejected(
        output(vec![patch.clone(), peer], None),
        api::ErrorCode::Conflict,
        "share one UI transaction",
    );
    harness.rejected(
        output(vec![patch.clone(); 65], None),
        api::ErrorCode::LimitExceeded,
        "output budget",
    );
    let peers = (0..9)
        .map(|i| {
            let mut value = patch.clone();
            value.panel = format!("panel-{i}");
            value
        })
        .collect();
    harness.rejected(
        output(peers, None),
        api::ErrorCode::LimitExceeded,
        "output budget",
    );
    let mut foreign = patch.clone();
    foreign.panel = "unowned".into();
    harness.rejected(
        output(vec![foreign], None),
        api::ErrorCode::PermissionDenied,
        "not owned",
    );
    let mut full = (*before).clone();
    full.revision = patch.next_ui_revision;
    let conflicting = api::Output {
        views: vec![api::View {
            panel: "diagram".into(),
            document: full.clone(),
        }],
        scene_patches: vec![patch.clone()],
        ..Default::default()
    };
    harness.rejected(
        serde_json::to_value(conflicting).unwrap(),
        api::ErrorCode::Conflict,
        "Conflicting full",
    );
    let wrong = api::ViewPatch {
        panel: "diagram".into(),
        base_revision: before.revision + 1,
        document: full.clone(),
        reused: vec![],
    };
    harness.rejected(
        output(vec![patch.clone()], Some(wrong)),
        api::ErrorCode::StaleRevision,
        "bases do not match",
    );
    let mut counterfeit = full.clone();
    counterfeit
        .root
        .try_visit_mut(&mut |node| {
            if node.id == "drawing"
                && let ui::Kind::Canvas(canvas) = &mut node.kind
            {
                canvas.scene.as_mut().unwrap().revision = patch.next_revision;
            }
            Ok(())
        })
        .unwrap();
    let peer = api::ViewPatch {
        panel: "diagram".into(),
        base_revision: before.revision,
        document: counterfeit,
        reused: vec![],
    };
    harness.rejected(
        output(vec![patch.clone()], Some(peer)),
        api::ErrorCode::InvalidRequest,
        "accepted artwork base",
    );
    let mut unknown = patch.clone();
    unknown.operations=vec![serde_json::from_value(json!({"op":"upsert_target","index":0,
        "target":{"id":"unknown","epoch":1,"space":"missing","geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":1,"h":1}}}})).unwrap()];
    harness.rejected(
        output(vec![unknown], None),
        api::ErrorCode::InvalidRequest,
        "group space",
    );
    // Ordinary shell and artwork share one exact base/next pair; references come only from `before`.
    let mut desired = (*before).clone();
    desired.revision = patch.next_ui_revision;
    desired
        .root
        .try_visit_mut(&mut |node| {
            if node.id == "drawing"
                && let ui::Kind::Canvas(canvas) = &mut node.kind
            {
                canvas.scene.as_mut().unwrap().revision = patch.next_revision;
            }
            if node.id == "exchange"
                && let ui::Kind::Text { text, .. } = &mut node.kind
            {
                *text = "paired status".into();
            }
            Ok(())
        })
        .unwrap();
    let publication = ui::retained_publication("diagram", Some(&before), desired.clone()).unwrap();
    assert_eq!(publication.output.scene_patches.len(), 1);
    assert_eq!(publication.output.view_patches.len(), 1);
    harness
        .probe(serde_json::to_value(publication.output).unwrap())
        .unwrap();
    assert_eq!(*harness.document(), desired);
    assert_eq!(before.revision, patch.base_ui_revision);
}

/// The guest generates large output itself; small command arguments cannot hit an earlier input quota.
#[test]
#[ignore = "executes independently built Flow 1.3 guest-generated retained byte budget probes"]
fn retained_guest_encoded_and_expanded_budgets_reject_at_scene_admission() {
    let mut harness = Harness::open();
    let revision = harness.document().revision + 1;
    harness
        .probe(json!({"mode":"seed-budget","ui_revision":revision}))
        .unwrap();
    let before = harness.document();
    assert!(serde_json::to_vec(&*before).unwrap().len() < ui::MAX_SCENE_PATCH_BYTES);
    harness.rejected(
        json!({"mode":"expanded-budget","ui_revision":revision}),
        api::ErrorCode::LimitExceeded,
        "expanded budget",
    );
    harness.rejected(
        json!({"mode":"encoded-budget","ui_revision":revision}),
        api::ErrorCode::LimitExceeded,
        "output budget",
    );
}

/// Single-cell selection and text changes encode only changed drawing/input/semantic objects.
#[test]
#[ignore = "build Table 1.2 through --plugin-cargo and set NANOBUG_CANVAS_TABLE_PACKAGE"]
fn retained_table_single_cell_text_and_selection_keep_publication_local() {
    let mut harness =
        Harness::open_package("NANOBUG_CANVAS_TABLE_PACKAGE", "canvas-table", "table");
    let shown = harness.document();
    let node = shown.active_node("drawing").unwrap();
    let ui::Kind::Canvas(canvas) = &node.kind else {
        panic!()
    };
    let scene = canvas.scene.as_ref().unwrap();
    harness
        .manager
        .event(
            "canvas-table",
            Some("table".into()),
            api::Notification::Ui(ui::UiEvent {
                revision: shown.revision,
                node: "drawing".into(),
                action: ui::Action::Canvas(ui::CanvasEvent::ScenePresented {
                    identity: scene.identity,
                    scene_revision: scene.revision,
                    source: shown.source.clone(),
                    file: shown.file.clone(),
                    projection: node.viewport.clone(),
                    error: None,
                }),
            }),
        )
        .unwrap();
    let event = |sequence, kind| {
        api::Notification::Ui(ui::UiEvent {
            revision: shown.revision,
            node: "drawing".into(),
            action: ui::Action::Canvas(ui::CanvasEvent::Input {
                input: ui::CanvasInput {
                    identity: scene.identity,
                    presented_revision: scene.revision,
                    target: "cell-1".into(),
                    epoch: 1,
                    sequence,
                    source: shown.source.clone(),
                    file: shown.file.clone(),
                    projection: node.viewport.clone(),
                    kind,
                },
            }),
        })
    };
    let observations = harness.manager.runtime_observations();
    observations.start_capture();
    harness
        .manager
        .event(
            "canvas-table",
            Some("table".into()),
            event(1, ui::CanvasInputKind::Focus { focused: true }),
        )
        .unwrap();
    harness
        .manager
        .event(
            "canvas-table",
            Some("table".into()),
            event(
                2,
                ui::CanvasInputKind::Text(ui::CanvasTextEdit {
                    text_revision: 1,
                    base_sequence: 0,
                    range: ui::CanvasTextRange { start: 4, end: 15 },
                    text: "中文".into(),
                    selection: ui::CanvasTextRange { start: 10, end: 10 },
                }),
            ),
        )
        .unwrap();
    let after = harness.document();
    let ui::Kind::Canvas(canvas) = &after.active_node("drawing").unwrap().kind else {
        panic!()
    };
    let scene = canvas.scene.as_ref().unwrap();
    let binding = scene.targets[1].text_input.as_ref().unwrap();
    assert_eq!(
        scene.paragraph_text(&binding.paragraph, &binding.layout_key),
        Some("e\u{301} 中文")
    );
    let untouched = scene.targets[3].text_input.as_ref().unwrap();
    assert_eq!(
        scene.paragraph_text(&untouched.paragraph, &untouched.layout_key),
        Some("42")
    );
    let bytes = observations
        .snapshot(None)
        .records
        .into_iter()
        .filter(|r| r.stage == plugin_runtime::observability::Stage::Publication)
        .filter_map(|r| r.bytes)
        .collect::<Vec<_>>();
    let full = serde_json::to_vec(&*shown).unwrap().len() as u64;
    assert_eq!(
        bytes.len(),
        2,
        "both actual selection/text callbacks must be observed"
    );
    assert!(
        bytes.iter().all(|bytes| *bytes < full / 2),
        "single-cell callbacks must omit unchanged scene data: {bytes:?}, full={full}"
    );
    eprintln!(
        "table selection/text actual publication bytes={bytes:?}, full document bytes={full}"
    );
}
