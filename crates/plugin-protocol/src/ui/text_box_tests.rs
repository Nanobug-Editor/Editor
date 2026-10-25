//! Bounded native text requires a capability and shares the existing drawing/content budgets.
use super::*;

/// One generic canvas text region contains no plugin identity or domain-specific information.
fn region() -> crate::Paint {
    crate::Paint::TextBox {
        rect: crate::Rect {
            x: 10.,
            y: 5.,
            w: 120.,
            h: 18.,
        },
        text: "中文 / proportional Wi".into(),
        color: 0x123456,
        size: 12.,
        bold: true,
        font: None,
        align: Alignment::End,
    }
}

/// The public wire format retains Unicode and declares the drawing's actual minimum capability.
#[test]
fn text_box_requires_canvas_14_and_roundtrips() {
    let canvas = Canvas {
        paint: vec![region()],
        ..Default::default()
    };
    let scene = Document::new(Node::new("plot", Kind::Canvas(canvas)));
    scene.validate().unwrap();
    assert_eq!(
        scene.required_capabilities()["ui.canvas"],
        semver::Version::new(1, 4, 0)
    );
    let restored: Document = serde_json::from_str(&serde_json::to_string(&scene).unwrap()).unwrap();
    assert_eq!(restored, scene);
}

/// Invalid geometry, fonts, multiline content and aggregate text cannot bypass canvas quotas.
#[test]
fn text_box_rejects_invalid_regions_and_content() {
    for change in 0..5 {
        let mut operation = region();
        let crate::Paint::TextBox {
            rect,
            text,
            size,
            font,
            ..
        } = &mut operation
        else {
            unreachable!()
        };
        match change {
            0 => rect.w = -1.,
            1 => rect.y = f32::INFINITY,
            2 => *size = f32::NAN,
            3 => *font = Some(String::new()),
            _ => *text = "two\nlines".into(),
        }
        assert!(
            Canvas {
                paint: vec![operation],
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
    let mut operation = region();
    let crate::Paint::TextBox { text, .. } = &mut operation else {
        unreachable!()
    };
    *text = "x".repeat(65536);
    assert!(
        Canvas {
            paint: vec![operation; 33],
            ..Default::default()
        }
        .validate()
        .is_err()
    );
}
