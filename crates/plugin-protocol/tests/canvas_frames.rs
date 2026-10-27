//! Independent guests request a visible frame through the ordinary versioned output contract.
use plugin_protocol::{api, ui};

/// Requests preserve explicit scene ownership and cannot use zero as an ambiguous request token.
#[test]
fn canvas_frame_requests_round_trip_without_changing_a_view() {
    let request = ui::CanvasFrameRequest {
        panel: "timeline".into(),
        canvas: "drawing".into(),
        identity: 81,
        request: 1,
    };
    request.validate().unwrap();
    let output = api::Output {
        canvas_frames: vec![request.clone()],
        ..Default::default()
    };
    let wire = serde_json::to_value(output).unwrap();
    assert!(wire["views"].as_array().unwrap().is_empty());
    assert_eq!(wire["canvas_frames"][0]["request"], 1);
    let decoded: api::Output = serde_json::from_value(wire).unwrap();
    assert_eq!(decoded.canvas_frames, vec![request.clone()]);
    let mut invalid = request;
    invalid.request = 0;
    assert_eq!(
        invalid.validate().unwrap_err().code,
        api::ErrorCode::InvalidRequest
    );
}

/// A virtualized legal content range can exceed drawing coordinates while awaiting refill.
#[test]
fn canvas_frame_opportunity_can_report_a_large_content_viewport() {
    ui::CanvasFrameOpportunity {
        identity: 81,
        request: 1,
        sequence: 1,
        viewport: plugin_protocol::Rect {
            x: 2_000_000.,
            y: 3_000_000.,
            w: 640.,
            h: 360.,
        },
        interval_ms: 16.,
        visible: true,
    }
    .validate()
    .unwrap();
}
