//! Native measurement requests and replies remain bounded by their owning scene and sample IDs.
use super::*;

/// Generic wire samples negotiate their capability; native replies cannot escape their current canvas.
#[test]
fn canvas_text_measurement_roundtrips_and_rejects_unowned_replies() {
    let canvas: Canvas = serde_json::from_value(serde_json::json!({
        "paint": [], "measure_text": [{"id": "author", "texts": ["Wi", "中文作者"],
            "size": 12., "bold": true, "font": null}]
    }))
    .unwrap();
    let document = Document::new(Node::new("plot", Kind::Canvas(canvas)));
    document.validate().unwrap();
    assert_eq!(
        document.required_capabilities()["ui.canvas"],
        semver::Version::new(1, 5, 0)
    );
    let event = |id: &str, width: f32| UiEvent {
        node: "plot".into(),
        revision: document.revision,
        action: Action::Canvas(
            serde_json::from_value(serde_json::json!({
                "type": "text_measured", "widths": [{"id": id, "width": width}]
            }))
            .unwrap(),
        ),
    };
    document.validate_event(&event("author", 54.25)).unwrap();
    assert!(
        document
            .validate_event(&event("another-canvas", 54.25))
            .is_err()
    );
    assert!(document.validate_event(&event("author", -1.)).is_err());
    let mut stale = event("author", 54.25);
    stale.revision = document.revision + 1;
    assert!(document.validate_event(&stale).is_err());
    let restored: Document =
        serde_json::from_str(&serde_json::to_string(&document).unwrap()).unwrap();
    assert_eq!(restored, document);
}

/// Sample text, group IDs, font geometry and native reply batches cannot bypass scene budgets.
#[test]
fn canvas_text_measurement_bounds_samples_and_results() {
    let sample = TextMeasure {
        id: "sample".into(),
        texts: vec!["abc".into()],
        size: 12.,
        bold: false,
        font: None,
    };
    for change in 0..8 {
        let mut request = sample.clone();
        match change {
            0 => request.id.clear(),
            1 => request.texts.clear(),
            2 => request.texts = vec!["x".repeat(4097)],
            3 => request.texts = vec!["multiline\ntext".into()],
            4 => request.size = f32::NAN,
            5 => request.font = Some(String::new()),
            6 => request.texts = vec![String::new(); 257],
            _ => request.texts = vec!["x".repeat(4096); 17],
        }
        assert!(
            Canvas {
                measure_text: vec![request],
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
    assert!(
        Canvas {
            measure_text: vec![sample.clone(), sample.clone()],
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    let mut document = Document::new(Node::new(
        "plot",
        Kind::Canvas(Canvas {
            measure_text: vec![sample],
            ..Default::default()
        }),
    ))
    .revision(4);
    for widths in [
        vec![],
        vec![TextWidth {
            id: "sample".into(),
            width: f32::INFINITY,
        }],
    ] {
        assert!(
            document
                .validate_event(&UiEvent {
                    node: "plot".into(),
                    revision: 4,
                    action: Action::Canvas(CanvasEvent::TextMeasured { widths })
                })
                .is_err()
        );
    }
    document.revision = 5;
    assert!(
        document
            .validate_event(&UiEvent {
                node: "plot".into(),
                revision: 4,
                action: Action::Canvas(CanvasEvent::TextMeasured {
                    widths: vec![TextWidth {
                        id: "sample".into(),
                        width: 30.
                    }]
                })
            })
            .is_err()
    );
}
