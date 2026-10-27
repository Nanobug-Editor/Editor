//! Detail presentation is verified through the actual package-backed native manager window.
use super::*;

/// Version rows and labeled information remain usable in a narrow, scaled native viewport.
pub(super) fn check_details(form: &mut VisualTestContext) {
    form.simulate_resize(size(px(850.), px(600.)));
    form.simulate_scale_factor_change(1.5);
    draw(form);
    let first = form.debug_bounds("market-version-1.1.0").unwrap();
    let installed = form.debug_bounds("market-version-1.0.0").unwrap();
    assert!(
        first.size.height >= px(52.),
        "version rows need room for readable metadata"
    );
    assert!(
        installed.top() >= first.bottom(),
        "version history must be a nonoverlapping vertical list"
    );
    assert!(
        form.debug_bounds("market-version-1.0.0-installed")
            .is_some()
    );
    let header = form.debug_bounds("plugin-manager-header").unwrap();
    let last_before = form.debug_bounds("market-version-0.9.1").unwrap();
    scroll(form, -99_999.);
    assert_eq!(header, form.debug_bounds("plugin-manager-header").unwrap());
    let last = form.debug_bounds("market-version-0.9.1").unwrap();
    assert!(last.top() < last_before.top());
    assert!(
        last.intersects(&form.debug_bounds("plugin-manager-detail-content").unwrap()),
        "the last version remains reachable by scrolling"
    );
    click(form, "plugin-detail-tabs-4");
    let frame = form.debug_bounds("plugin-manager-detail-content").unwrap();
    let id_label = form
        .debug_bounds("market-info-id-label")
        .expect("labeled plugin ID");
    let version_label = form.debug_bounds("market-info-version-label").unwrap();
    assert!(
        form.debug_bounds("market-info-maintainers-value")
            .unwrap()
            .size
            .height
            > form
                .debug_bounds("market-info-id-value")
                .unwrap()
                .size
                .height,
        "long information values must wrap"
    );
    assert_eq!(id_label.left(), version_label.left());
    assert_eq!(id_label.right(), version_label.right());
    for field in [
        "market-info-id-value",
        "market-info-repository-value",
        "market-info-permissions-value",
    ] {
        let value = form.debug_bounds(field).unwrap();
        assert!(
            value.left() >= frame.left() && value.right() <= frame.right(),
            "{field} escapes the content width"
        );
    }
    for group in [
        "market-info-basic",
        "market-info-compatibility",
        "market-info-statistics",
    ] {
        assert!(
            form.debug_bounds(group).is_some(),
            "missing information group {group}"
        );
    }
    let header = form.debug_bounds("plugin-manager-header").unwrap();
    scroll(form, -900.);
    assert_eq!(header, form.debug_bounds("plugin-manager-header").unwrap());
    assert!(form.debug_bounds("market-info-basic").unwrap().top() < frame.top());
    click(form, "plugin-detail-tabs-3");
    // The first row is still the reviewed newer release; scroll and information browsing never activate it.
    assert!(
        form.debug_bounds("plugin-manager-browse-version-1.1.0")
            .is_some()
    );
    assert!(
        form.debug_bounds("plugin-manager-installed-version-1.0.0")
            .is_some()
    );
}

/// Wheel the production content viewport instead of setting its offset from the test.
fn scroll(form: &mut VisualTestContext, delta: f32) {
    let viewport = form.debug_bounds("plugin-manager-detail-content").unwrap();
    form.simulate_event(gpui_kit::ScrollEvent {
        // Wheel and trackpad fixtures retain pointer-device semantics.
        source: gpui_kit::ScrollSource::Pointer,
        position: viewport.center(),
        delta: gpui_kit::ScrollDelta::Pixels(point(px(0.), px(delta))),
        touch_phase: gpui_kit::TouchPhase::Moved,
        modifiers: Default::default(),
    });
    draw(form);
}
