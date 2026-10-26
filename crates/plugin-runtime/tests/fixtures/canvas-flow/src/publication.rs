//! Actual WASM fault publications keep both input and wire-generation work below guest budgets.
use plugin_protocol::{api, ui};
use serde_json::Value;

/// Inject ordinary public wire data; the immutable normal publication baseline is cleared by dispatch.
pub(super) fn probe(arguments: Value) -> Result<api::Output, api::Failure> {
    serde_json::from_value(arguments)
        .map_err(|e| api::Failure::new(api::ErrorCode::InvalidRequest, e.to_string()))
}

/// Serialize a small typed Completion, then bulk-copy fixed legal path fragments into its wire arrays.
/// The returned string still enters ordinary WASM dispatch and complete host decoding/admission.
/// This avoids spending the finite interactive fuel on 35,000 identical floating-point conversions.
pub(super) fn budget_completion(payload: &str) -> Option<Result<String, String>> {
    if !payload.contains("publication-probe") {
        return None;
    }
    let invocation = serde_json::from_str::<api::Invocation>(payload).ok()?;
    let api::Input::Event {
        event: api::Notification::Command { id, arguments, .. },
        ..
    } = invocation.message
    else {
        return None;
    };
    if id != "publication-probe" || invocation.id == 0 {
        return None;
    }
    let arguments = arguments?;
    if !matches!(
        arguments.get("mode").and_then(Value::as_str),
        Some("seed-budget" | "expanded-budget" | "encoded-budget")
    ) {
        return None;
    }
    Some((|| {
        let result = template(arguments);
        let small = serde_json::to_string(&api::Completion {
            id: invocation.id,
            result,
        })
        .map_err(|e| e.to_string())?;
        // Both the needle and the header use typed serialization; only this fixed numeric path tail
        // is raw JSON. No user-controlled string, resource reference or authority is interpolated.
        let head = serde_json::to_string(&ui::ScenePathSegment::Move {
            x: 12345.678,
            y: 12345.678,
        })
        .map_err(|e| e.to_string())?;
        let needle = format!("[{head}]");
        let tail = r#",{"op":"line","x":12345.678,"y":12345.678}"#.repeat(35_000);
        let mut path = String::with_capacity(head.len() + tail.len() + 2);
        path.push('[');
        path.push_str(&head);
        path.push_str(&tail);
        path.push(']');
        Ok(small.replace(&needle, &path))
    })())
}

/// A fully typed small template retains exact Invocation/Completion and scene/UI addressing.
fn template(arguments: Value) -> Result<api::Output, api::Failure> {
    let Some(mode) = arguments.get("mode").and_then(Value::as_str) else {
        return Err(api::Failure::new(
            api::ErrorCode::InvalidRequest,
            "Missing budget mode",
        ));
    };
    let revision = arguments
        .get("ui_revision")
        .and_then(Value::as_u64)
        .ok_or_else(|| api::Failure::new(api::ErrorCode::InvalidRequest, "Missing UI revision"))?;
    let segments = vec![ui::ScenePathSegment::Move {
        x: 12345.678,
        y: 12345.678,
    }];
    let node = ui::SceneNode::Shape {
        id: "large-2".into(),
        geometry: ui::SceneGeometry::Path {
            segments,
            rule: Default::default(),
        },
        fill: Some(ui::SceneBrush::Solid { rgba: 255 }),
        stroke: None,
    };
    if mode == "seed-budget" {
        let canvas = ui::Canvas {
            scene: Some(ui::CanvasScene {
                identity: 501,
                revision: 1,
                roots: vec![node],
                targets: vec![],
                semantics: vec![],
            }),
            ..Default::default()
        };
        return Ok(api::Output {
            views: vec![api::View {
                panel: "diagram".into(),
                document: ui::Document::new(ui::Node::new("drawing", ui::Kind::Canvas(canvas)))
                    .revision(revision),
            }],
            ..Default::default()
        });
    }
    let mut insert = node;
    let ui::SceneNode::Shape { id, .. } = &mut insert else {
        unreachable!()
    };
    *id = "large-3".into();
    let operations = match mode {
        "encoded-budget" => vec![
            ui::CanvasSceneOperation::Insert {
                parent: None,
                index: 1,
                node: insert.clone(),
            },
            ui::CanvasSceneOperation::Insert {
                parent: None,
                index: 2,
                node: insert,
            },
        ],
        "expanded-budget" => vec![ui::CanvasSceneOperation::Insert {
            parent: None,
            index: 1,
            node: insert,
        }],
        _ => {
            return Err(api::Failure::new(
                api::ErrorCode::InvalidRequest,
                "Unknown publication probe mode",
            ));
        }
    };
    let patch = ui::CanvasScenePatch {
        panel: "diagram".into(),
        canvas: "drawing".into(),
        base_ui_revision: revision,
        next_ui_revision: revision + 1,
        identity: 501,
        base_revision: 1,
        next_revision: 2,
        operations,
        damage: None,
    };
    Ok(api::Output {
        scene_patches: vec![patch],
        ..Default::default()
    })
}
