//! Ordinary independent guests exercise shown-frame authority, native text chains and target retirement.
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

/// Preserve immutable historical 1.0 input; a candidate override is only for explicitly rebuilding 1.1.
fn package() -> Package {
    std::env::var_os("NANOBUG_CANVAS_EDITABLE_PACKAGE").map_or_else(
        || test_packages::package("canvas-flow-1.1.0"),
        |path| Package::read(std::path::Path::new(&path)).unwrap(),
    )
}
/// Install and demand through the same public lifecycle used by normal plugin surfaces.
fn install() -> (Manager, tempfile::TempDir, tempfile::TempDir) {
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
    let package = package();
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
        assert!(Instant::now() < deadline, "editable guest demand timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
    (manager, profile, workspace)
}
/// Read only the public immutable published document.
fn document(manager: &Manager) -> Arc<ui::Document> {
    manager.live["canvas-flow"].views["diagram"].clone()
}
/// Native observations and target input have distinct envelopes, both owned by the declared panel.
fn send(manager: &mut Manager, event: ui::UiEvent) -> anyhow::Result<()> {
    manager.event(
        "canvas-flow",
        Some("diagram".into()),
        api::Notification::Ui(event),
    )
}
/// Publish the native frame evidence for exactly the currently observable source and projection.
fn present(manager: &mut Manager) -> Arc<ui::Document> {
    let document = document(manager);
    let node = document.active_node("drawing").unwrap();
    let ui::Kind::Canvas(canvas) = &node.kind else {
        panic!("ordinary Canvas missing")
    };
    let scene = canvas.scene.as_ref().unwrap();
    let event = ui::CanvasEvent::ScenePresented {
        identity: scene.identity,
        scene_revision: scene.revision,
        source: document.source.clone(),
        file: document.file.clone(),
        projection: node.viewport.clone(),
        error: None,
    };
    send(
        manager,
        ui::UiEvent {
            revision: document.revision,
            node: "drawing".into(),
            action: ui::Action::ViewportInput(ui::ViewportInput {
                content: node.viewport.as_ref().unwrap().content.unwrap(),
                event,
            }),
        },
    )
    .unwrap();
    document
}
/// Route a target event against its original shown document rather than guessing the latest drawing.
fn input(
    shown: &ui::Document,
    sequence: u64,
    epoch: u64,
    kind: ui::CanvasInputKind,
) -> ui::UiEvent {
    let node = shown.active_node("drawing").unwrap();
    let ui::Kind::Canvas(canvas) = &node.kind else {
        unreachable!()
    };
    let scene = canvas.scene.as_ref().unwrap();
    ui::UiEvent {
        revision: shown.revision,
        node: "drawing".into(),
        action: ui::Action::Canvas(ui::CanvasEvent::Input {
            input: ui::CanvasInput {
                identity: scene.identity,
                presented_revision: scene.revision,
                target: "start".into(),
                epoch,
                sequence,
                source: shown.source.clone(),
                file: shown.file.clone(),
                projection: node.viewport.clone(),
                kind,
            },
        }),
    }
}
/// Domain buttons are ordinary native UI actions and do not manufacture coordinate evidence.
fn click(manager: &mut Manager, id: &str) -> anyhow::Result<()> {
    send(
        manager,
        ui::UiEvent {
            revision: document(manager).revision,
            node: id.into(),
            action: ui::Action::Click,
        },
    )
}
/// Observe the guest's committed paragraph and target binding through public scene values.
fn source(manager: &Manager) -> (String, ui::TextInputSession, u64) {
    let document = document(manager);
    let ui::Kind::Canvas(canvas) = &document.active_node("drawing").unwrap().kind else {
        unreachable!()
    };
    let scene = canvas.scene.as_ref().unwrap();
    let target = &scene.targets[0];
    let binding = target.text_input.as_ref().unwrap();
    (
        scene
            .paragraph_text(&binding.paragraph, &binding.layout_key)
            .unwrap()
            .into(),
        binding.clone(),
        target.epoch,
    )
}
/// Native text replaces original whole graphemes; subsequent queued commits name that exact predecessor.
fn edit(revision: u64, base: u64, start: usize, end: usize, text: &str) -> ui::CanvasInputKind {
    ui::CanvasInputKind::Text(ui::CanvasTextEdit {
        text_revision: revision,
        base_sequence: base,
        range: ui::CanvasTextRange { start, end },
        text: text.into(),
        selection: ui::CanvasTextRange {
            start: start + text.len(),
            end: start + text.len(),
        },
    })
}

/// Accepted redraws retain only proven shown-target input; malformed input/acks never retire the guest.
#[test]
#[ignore = "executes independent Canvas 1.1 WASM through public Manager; build package then run explicitly"]
fn canvas_editable_shown_frame_text_chain_ack_backpressure_and_aba() {
    let (mut manager, _profile, _workspace) = install();
    let shown = present(&mut manager);
    send(
        &mut manager,
        input(&shown, 1, 1, ui::CanvasInputKind::Focus { focused: true }),
    )
    .unwrap();
    click(&mut manager, "pan").unwrap();
    send(&mut manager, input(&shown, 2, 1, edit(1, 0, 4, 15, "中文")))
        .expect("target remains valid in the actually displayed older frame");
    assert_eq!(source(&manager).0, "e\u{301} 中文");
    let before = document(&manager);
    let error = send(&mut manager, input(&shown, 3, 1, edit(1, 2, 1, 1, "x")))
        .expect_err("range inside e+combining must fail before WASM");
    assert_eq!(
        error.downcast_ref::<api::Failure>().unwrap().code,
        api::ErrorCode::InvalidRequest
    );
    assert_eq!(document(&manager), before);
    let queued = input(&shown, 3, 1, edit(1, 2, 10, 10, "a"));
    send(&mut manager, queued.clone()).unwrap();
    assert_eq!(source(&manager).0, "e\u{301} 中文a");
    assert!(
        send(&mut manager, queued).is_err(),
        "native input sequence cannot replay"
    );
    let before = document(&manager);
    assert!(click(&mut manager, "bad-ack").is_err());
    assert_eq!(
        document(&manager),
        before,
        "contradictory acknowledgement is an atomic publication rejection"
    );
    click(&mut manager, "recover").expect("same guest survives rejected ack");

    click(&mut manager, "hold").unwrap();
    let (baseline, binding, _) = source(&manager);
    let mut previous = 0;
    let mut sequence = 4;
    for index in 0..32 {
        let end = baseline.len() + index;
        send(
            &mut manager,
            input(
                &shown,
                sequence,
                1,
                edit(binding.text_revision, previous, end, end, "x"),
            ),
        )
        .unwrap();
        previous = sequence;
        sequence += 1;
    }
    let end = baseline.len() + 32;
    let error = send(
        &mut manager,
        input(
            &shown,
            sequence,
            1,
            edit(binding.text_revision, previous, end, end, "x"),
        ),
    )
    .unwrap_err();
    assert_eq!(
        error.downcast_ref::<api::Failure>().unwrap().code,
        api::ErrorCode::LimitExceeded
    );
    assert_eq!(
        source(&manager).0,
        baseline,
        "unacknowledged native drafts do not overwrite guest source"
    );
    click(&mut manager, "ack").unwrap();
    assert_eq!(source(&manager).0, format!("{baseline}{}", "x".repeat(32)));
    click(&mut manager, "undo").unwrap();
    assert_eq!(
        source(&manager).0,
        format!("{baseline}{}", "x".repeat(31)),
        "domain undo remains in guest"
    );
    let epoch = source(&manager).2;
    assert!(
        send(
            &mut manager,
            input(
                &shown,
                sequence,
                epoch,
                ui::CanvasInputKind::Focus { focused: true }
            )
        )
        .is_err(),
        "new target cannot borrow the older target's native frame proof"
    );
    let fresh = present(&mut manager);
    send(
        &mut manager,
        input(
            &fresh,
            sequence,
            epoch,
            ui::CanvasInputKind::Focus { focused: true },
        ),
    )
    .unwrap();
    let binding = source(&manager).1;
    let end = source(&manager).0.len();
    sequence += 1;
    send(
        &mut manager,
        input(
            &fresh,
            sequence,
            epoch,
            ui::CanvasInputKind::Composition(ui::CanvasComposition {
                phase: ui::CanvasCompositionPhase::Start,
                text_revision: binding.text_revision,
                base_sequence: 0,
                range: ui::CanvasTextRange { start: end, end },
                text: "ni".into(),
                selection: ui::CanvasTextRange { start: 2, end: 2 },
            }),
        ),
    )
    .unwrap();
    click(&mut manager, "remove-target").unwrap();
    let before = document(&manager);
    sequence += 1;
    let cancel = ui::CanvasInputKind::Composition(ui::CanvasComposition {
        phase: ui::CanvasCompositionPhase::Cancel,
        text_revision: binding.text_revision,
        base_sequence: 0,
        range: ui::CanvasTextRange { start: end, end },
        text: String::new(),
        selection: ui::CanvasTextRange { start: 0, end: 0 },
    });
    send(&mut manager, input(&fresh, sequence, epoch, cancel.clone()))
        .expect("old active preedit cleanup survives target retirement");
    sequence += 1;
    assert!(
        send(&mut manager, input(&fresh, sequence, epoch, cancel)).is_err(),
        "preedit cleanup lease is consumed exactly once"
    );
    send(
        &mut manager,
        input(
            &fresh,
            sequence,
            epoch,
            ui::CanvasInputKind::Focus { focused: false },
        ),
    )
    .unwrap();
    assert!(
        send(
            &mut manager,
            input(
                &fresh,
                sequence + 1,
                epoch,
                ui::CanvasInputKind::Focus { focused: false }
            )
        )
        .is_err(),
        "focus cleanup cannot replay"
    );
    assert!(
        click(&mut manager, "restore-target").is_err(),
        "retired target epoch cannot be resurrected"
    );
    assert_eq!(document(&manager), before);
    click(&mut manager, "fresh-target").unwrap();
    assert!(source(&manager).2 > epoch);
    pointer_chord_cleanup(&mut manager, sequence + 10);
    manager.uninstall("canvas-flow", true).unwrap();
    assert!(!manager.live.contains_key("canvas-flow"));
}

/// Releasing one native button cannot consume another held button's retirement cleanup lease.
fn pointer_chord_cleanup(manager: &mut Manager, sequence: u64) {
    use ui::{CanvasPointerInput, CanvasPointerPhase as Phase};
    let frame = present(manager);
    let epoch = source(manager).2;
    let pointer = |phase, button| {
        ui::CanvasInputKind::Pointer(CanvasPointerInput {
            pointer: 1,
            device: ui::CanvasPointerDevice::Mouse,
            phase,
            x: 30.,
            y: 30.,
            button,
            buttons: None,
            clicks: matches!(phase, Phase::Down | Phase::Up).then_some(1),
            pressure: None,
            modifiers: Default::default(),
            cancel_reason: (phase == Phase::Cancel)
                .then_some(ui::CanvasCancelReason::TargetRemoved),
        })
    };
    for (offset, phase, button) in [(0, Phase::Down, 0), (1, Phase::Down, 2), (2, Phase::Up, 2)] {
        send(
            manager,
            input(
                &frame,
                sequence + offset,
                epoch,
                pointer(phase, Some(button)),
            ),
        )
        .unwrap();
    }
    click(manager, "remove-target").unwrap();
    send(
        manager,
        input(&frame, sequence + 3, epoch, pointer(Phase::Cancel, None)),
    )
    .expect("remaining held button keeps its original-scoped cleanup lease");
    assert!(
        send(
            manager,
            input(&frame, sequence + 4, epoch, pointer(Phase::Cancel, None))
        )
        .is_err(),
        "pointer cleanup cannot replay after all held buttons were cancelled"
    );
}
