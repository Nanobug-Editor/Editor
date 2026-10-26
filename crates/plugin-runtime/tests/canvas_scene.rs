//! Actual independent WASM consumers exercise public negotiation, selection and retirement.
use plugin_runtime::{
    Manager,
    plugin_protocol::{Environment, api, ui},
};

#[path = "support/packages.rs"]
mod test_packages;

/// Build the ordinary fixture through `--plugin-package` before explicitly running this test.
#[test]
#[ignore = "executes the frozen independent WASM package through Manager; run explicitly with --test-threads=1"]
fn canvas_scene_package_selects_after_zoom_and_rejects_late_coordinates() {
    let package = test_packages::package("canvas-flow-1.0.0");
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
    show(&mut manager);
    let initial = manager.live["canvas-flow"].views["diagram"].revision;
    manager
        .event(
            "canvas-flow",
            Some("diagram".into()),
            api::Notification::Ui(ui::UiEvent {
                revision: initial,
                node: "zoom".into(),
                action: ui::Action::Click,
            }),
        )
        .unwrap();
    let drawing = manager.live["canvas-flow"].views["diagram"].clone();
    let pointer = ui::Action::ViewportInput(ui::ViewportInput {
        content: ui::ContentSize {
            width: 320.,
            height: 200.,
        },
        event: ui::CanvasEvent::Pointer {
            phase: ui::PointerPhase::Down,
            x: 75.,
            y: 50.,
            button: 0,
            clicks: 1,
            shift: false,
        },
    });
    assert!(
        manager
            .event(
                "canvas-flow",
                Some("diagram".into()),
                api::Notification::Ui(ui::UiEvent {
                    revision: initial,
                    node: "drawing".into(),
                    action: pointer.clone()
                })
            )
            .is_err(),
        "coordinates from an old projection must be rejected"
    );
    assert!(
        manager
            .event(
                "canvas-flow",
                Some("diagram".into()),
                api::Notification::Ui(ui::UiEvent {
                    revision: drawing.revision,
                    node: "drawing".into(),
                    action: pointer.clone()
                })
            )
            .is_err(),
        "a published scene must not acquire pointer authority before native presentation"
    );
    let node = drawing.active_node("drawing").unwrap();
    let ui::Kind::Canvas(canvas) = &node.kind else {
        panic!("fixture canvas is missing")
    };
    let scene = canvas.scene.as_ref().unwrap();
    let receipt: ui::CanvasEvent = serde_json::from_value(serde_json::json!({
        "type":"scene_presented","identity":scene.identity,"scene_revision":scene.revision,
        "source":drawing.source,"file":drawing.file,"projection":node.viewport
    }))
    .unwrap();
    manager
        .event(
            "canvas-flow",
            Some("diagram".into()),
            api::Notification::Ui(ui::UiEvent {
                revision: drawing.revision,
                node: "drawing".into(),
                action: ui::Action::ViewportInput(ui::ViewportInput {
                    content: ui::ContentSize {
                        width: 320.,
                        height: 200.,
                    },
                    event: receipt,
                }),
            }),
        )
        .unwrap();
    manager
        .event(
            "canvas-flow",
            Some("diagram".into()),
            api::Notification::Ui(ui::UiEvent {
                revision: drawing.revision,
                node: "drawing".into(),
                action: pointer,
            }),
        )
        .unwrap();
    let selected = &manager.live["canvas-flow"].views["diagram"];
    let mut selected_text = None;
    selected.root.visit(&mut |node| {
        if node.id == "selection" {
            if let ui::Kind::Text { text } = &node.kind {
                selected_text = Some(text.clone());
            }
        }
    });
    assert_eq!(selected_text.as_deref(), Some("start"));
    manager.uninstall("canvas-flow", true).unwrap();
    assert!(!manager.live.contains_key("canvas-flow"));
    assert!(manager.image_resources().is_empty());
}

/// A malformed publication keeps the old displayed document and does not retire its WASM owner.
#[test]
#[ignore = "executes the frozen independent WASM package through Manager; run explicitly with --test-threads=1"]
fn canvas_scene_package_rejects_revision_reuse_and_aba_atomically() {
    let package = test_packages::package("canvas-flow-1.0.0");
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
    show(&mut manager);
    let before = manager.live["canvas-flow"].views["diagram"].revision;
    assert!(
        click(&mut manager, "rewind").is_err(),
        "reusing a scene drawing version must fail"
    );
    assert_eq!(
        manager.live["canvas-flow"]
            .views
            .get("diagram")
            .map(|doc| doc.revision),
        Some(before),
        "rejection must preserve the last accepted scene"
    );
    click(&mut manager, "recover").expect("the same live instance can publish a corrected scene");
    click(&mut manager, "replace").unwrap();
    let replacement = manager.live["canvas-flow"].views["diagram"].revision;
    assert!(
        click(&mut manager, "restore").is_err(),
        "retired scene identities cannot be resurrected"
    );
    assert_eq!(
        manager.live["canvas-flow"].views["diagram"].revision,
        replacement
    );
}

/// Send an ordinary currently-addressed native button action to the actual guest.
fn click(manager: &mut Manager, node: &str) -> anyhow::Result<()> {
    let revision = manager.live["canvas-flow"].views["diagram"].revision;
    manager.event(
        "canvas-flow",
        Some("diagram".into()),
        api::Notification::Ui(ui::UiEvent {
            revision,
            node: node.into(),
            action: ui::Action::Click,
        }),
    )
}

/// Installation is cold metadata; demand the declared view through the public activation lifecycle.
fn show(manager: &mut Manager) {
    let availability = manager.availability("canvas-flow").unwrap();
    let demand = manager
        .begin_demand(
            &availability,
            plugin_runtime::DemandReason::View("diagram".into()),
            60_000,
        )
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        manager.poll();
        if let api::RequestUpdate::Completed { result } = demand.status() {
            result.unwrap();
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "canvas view demand timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
