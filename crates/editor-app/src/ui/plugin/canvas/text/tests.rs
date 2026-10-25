//! Native sample replies use proportional glyphs and remain cached across ordinary redraws.
use super::*;
use gpui_kit::{AppContext as _, TestAppContext, gpui};
use std::cell::RefCell;

/// GPUI windows emit bounded widths, reuse unchanged requests and remeasure changed faces.
/// Real Windows font advances are checked by the separate native plugin acceptance scenario.
#[gpui::test]
fn canvas_native_text_measurement_caches_requests_and_invalidates_on_font_change(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::typography::init(cx);
    });
    let requests: Vec<_> = [
        ("thin", "iii", false),
        ("wide", "WWW", false),
        ("bold", "iii", true),
    ]
    .into_iter()
    .map(|(id, value, bold)| protocol::ui::TextMeasure {
        id: id.into(),
        texts: vec![value.into()],
        size: 12.,
        bold,
        font: None,
    })
    .collect();
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let retained = Rc::new(RefCell::new(None));
    let saved = retained.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| {
            CanvasView::new(
                Canvas {
                    measure_text: requests,
                    ..Default::default()
                },
                move |event, _, _| output.borrow_mut().push(event),
                window,
                cx,
            )
        });
        view.update(cx, |view, _| view.font.family = Some("Segoe UI".into()));
        *saved.borrow_mut() = Some(view.clone());
        gpui_base::Root::new(view, window, cx)
    });
    visual.run_until_parked();
    let values: Vec<_> = events
        .borrow()
        .iter()
        .filter_map(|event| match event {
            CanvasEvent::TextMeasured { widths } => Some(widths.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0]
            .iter()
            .map(|width| width.id.as_str())
            .collect::<Vec<_>>(),
        ["thin", "wide", "bold"]
    );
    assert!(
        values[0]
            .iter()
            .all(|width| width.width.is_finite() && width.width > 0.)
    );
    let view = retained.borrow().as_ref().unwrap().clone();
    visual.update(|_, cx| view.update(cx, |_, cx| cx.notify()));
    visual.run_until_parked();
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|event| matches!(event, CanvasEvent::TextMeasured { .. }))
            .count(),
        1
    );
    visual.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.font.family = Some("Consolas".into());
            cx.notify();
        })
    });
    visual.run_until_parked();
    let values: Vec<_> = events
        .borrow()
        .iter()
        .filter_map(|event| match event {
            CanvasEvent::TextMeasured { widths } => Some(widths.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(values.len(), 2);
    assert!(
        values[1]
            .iter()
            .all(|width| width.width.is_finite() && width.width > 0.)
    );
}
