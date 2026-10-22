//! Scroll streams retain content authority across repaints, while ordinary input stays scene-bound.
use super::*;

/// Retained drawing is explicitly negotiated; malformed coverage or competing transforms fail closed.
#[test]
fn retained_scroll_buffer_requires_version_and_valid_coverage() {
    let canvas = Canvas {
        scroll: Some(ScrollRange {
            content: 2000.,
            offset: 80.,
        }),
        scroll_epoch: Some(3),
        scroll_buffer: Some(ScrollBuffer {
            start: 0.,
            end: 600.,
        }),
        ..Default::default()
    };
    let document = Document::new(Node::new("timeline", Kind::Canvas(canvas.clone())));
    document.validate().unwrap();
    assert_eq!(
        document.required_capabilities()["ui.canvas"],
        semver::Version::new(1, 3, 0)
    );
    for buffer in [
        ScrollBuffer {
            start: 81.,
            end: 600.,
        },
        ScrollBuffer {
            start: 0.,
            end: 79.,
        },
        ScrollBuffer {
            start: 0.,
            end: 2001.,
        },
    ] {
        let mut invalid = canvas.clone();
        invalid.scroll_buffer = Some(buffer);
        assert!(invalid.validate().is_err());
    }
    let mut invalid = canvas.clone();
    invalid.scroll_epoch = None;
    assert!(invalid.validate().is_err());
    invalid = canvas;
    invalid.grid = true;
    assert!(invalid.validate().is_err());
}

/// Multiple deltas captured before the next frame must reach one unchanged scroll content.
#[test]
fn scroll_stream_survives_repaint_but_not_content_replacement() {
    let mut wire = serde_json::to_value(
        Document::new(Node::new(
            "timeline",
            Kind::Canvas(Canvas {
                scroll: Some(ScrollRange {
                    content: 2000.,
                    offset: 80.,
                }),
                ..Default::default()
            }),
        ))
        .revision(8),
    )
    .unwrap();
    wire["root"]["kind"]["scroll_epoch"] = 3.into();
    let mut document: Document = serde_json::from_value(wire)
        .expect("the public canvas contract supports a content-owned scroll stream");
    assert_eq!(
        document.required_capabilities()["ui.canvas"],
        semver::Version::new(1, 2, 0)
    );
    let event: UiEvent = serde_json::from_value(serde_json::json!({
        "revision": 7, "node": "timeline", "action": { "type": "canvas", "value": {
            "type": "scroll_input", "epoch": 3,
            "change": { "type": "delta", "delta_y": -80. }
        }}
    }))
    .unwrap();
    document
        .validate_event(&event)
        .expect("a repaint must retain queued deltas");
    let mut invalid = event.clone();
    invalid.revision = 9;
    assert_eq!(
        document.validate_event(&invalid).unwrap_err().code,
        crate::api::ErrorCode::StaleRevision
    );
    invalid = event.clone();
    invalid.action = Action::Canvas(CanvasEvent::Wheel {
        x: 0.,
        y: 0.,
        delta_x: 0.,
        delta_y: -80.,
        shift: false,
    });
    assert_eq!(
        document.validate_event(&invalid).unwrap_err().code,
        crate::api::ErrorCode::StaleRevision
    );
    invalid.action = Action::Canvas(CanvasEvent::Pointer {
        phase: PointerPhase::Down,
        x: 0.,
        y: 0.,
        button: 0,
        clicks: 1,
        shift: false,
    });
    assert_eq!(
        document.validate_event(&invalid).unwrap_err().code,
        crate::api::ErrorCode::StaleRevision
    );
    document.root.disabled = true;
    assert!(document.validate_event(&event).is_err());
    document.root.disabled = false;
    document.dialog = Some(Dialog::new(
        "modal",
        "Confirm",
        Node::text("message", "Wait"),
    ));
    assert!(document.validate_event(&event).is_err());
    document.dialog = None;
    invalid = event.clone();
    invalid.action = Action::Canvas(CanvasEvent::ScrollInput {
        epoch: 3,
        change: ScrollChange::Delta { delta_y: f32::NAN },
    });
    assert!(document.validate_event(&invalid).is_err());
    let mut replacement = serde_json::to_value(&document).unwrap();
    replacement["root"]["kind"]["scroll_epoch"] = 4.into();
    document = serde_json::from_value(replacement).unwrap();
    assert_eq!(
        document.validate_event(&event).unwrap_err().code,
        crate::api::ErrorCode::StaleRevision
    );
}
