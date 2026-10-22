//! Measure actual native paints and subtree CPU cost without driving frames from a sampling timer.
use super::*;
use gpui_kit::{AnyElement, Element, ElementId, GlobalElementId, InspectorElementId, LayoutId};
use std::time::Instant;

/// One actual paint, recorded after the complete native content subtree finishes.
pub(super) struct Sample {
    /// Wall-clock completion time, independent of the test's periodic offset observations.
    pub at: Instant,
    /// Sum of native layout-request, prepaint and paint call durations, excluding idle gaps
    /// between phases. Platform presentation and the window's layout solver use frame cadence.
    pub cpu: Duration,
    /// Committed native source offset at the end of the paint.
    pub offset: Point<gpui_kit::Pixels>,
    /// Inactive windows may be throttled by the platform and cannot certify active-window FPS.
    pub active: bool,
    /// Original comparison authority's visible logical rows, exposing accidental full layouts.
    pub native_rows: Option<usize>,
}

/// An uncached observation boundary around the same cached native subtree used in production.
pub(super) struct Measured {
    /// Original cached view, with all native state and input preserved.
    pub content: Entity<CachedHarness>,
    /// Original source or list supplies the committed scroll coordinate.
    pub host: Entity<Harness>,
    /// Append-only records live only during this explicitly invoked native test.
    pub samples: Rc<RefCell<Vec<Sample>>>,
}
impl Render for Measured {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Probe {
            content: self.content.clone().into_any_element(),
            host: self.host.clone(),
            samples: self.samples.clone(),
            cpu: Duration::ZERO,
        }
    }
}

/// Transparent timing element adds no notification, animation callback or layout node.
struct Probe {
    /// Complete original subtree, including its native scroll wrapper.
    content: AnyElement,
    /// Read-only access to the original source or virtual-list offset.
    host: Entity<Harness>,
    /// Shared output buffer allocated once before the experiment.
    samples: Rc<RefCell<Vec<Sample>>>,
    /// Accumulated native call time, reset after paint so cached frames cannot include idle waits.
    cpu: Duration,
}
impl IntoElement for Probe {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for Probe {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut gpui_kit::App,
    ) -> (LayoutId, ()) {
        let started = Instant::now();
        let layout = self.content.request_layout(window, cx);
        self.cpu = started.elapsed();
        (layout, ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: gpui_kit::Bounds<gpui_kit::Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut gpui_kit::App,
    ) {
        let started = Instant::now();
        self.content.prepaint(window, cx);
        self.cpu += started.elapsed();
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: gpui_kit::Bounds<gpui_kit::Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut gpui_kit::App,
    ) {
        let started = Instant::now();
        self.content.paint(window, cx);
        let at = Instant::now();
        let host = self.host.read(cx);
        self.samples.borrow_mut().push(Sample {
            at,
            cpu: self.cpu + at.duration_since(started),
            offset: native_offset(host, cx),
            active: window.is_window_active(),
            native_rows: matches!(host.kind, Kind::Comparison)
                .then(|| {
                    host.editor
                        .read(cx)
                        .visible_row_range()
                        .map(|range| range.len())
                })
                .flatten(),
        });
        self.cpu = Duration::ZERO;
    }
}

/// Report moving-frame cadence and enforce an explicit hardware acceptance threshold when set.
pub(super) fn verify(samples: &[Sample]) {
    let moving: Vec<_> = samples
        .windows(2)
        .filter(|pair| pair[1].offset != pair[0].offset)
        .map(|pair| &pair[1])
        .collect();
    assert!(
        moving.len() > 10,
        "the native test must observe enough actual moving frames"
    );
    let active_count = moving.iter().filter(|sample| sample.active).count();
    let mut intervals: Vec<_> = moving
        .windows(2)
        .filter(|pair| pair[0].active && pair[1].active)
        .map(|pair| pair[1].at.duration_since(pair[0].at).as_secs_f64() * 1000.)
        .collect();
    assert!(
        !intervals.is_empty(),
        "native measurement must include foreground frames"
    );
    let fps = intervals.len() as f64 * 1000. / intervals.iter().sum::<f64>();
    let mut cpu: Vec<_> = moving
        .iter()
        .filter(|sample| sample.active)
        .map(|sample| sample.cpu.as_secs_f64() * 1000.)
        .collect();
    intervals.sort_by(f64::total_cmp);
    cpu.sort_by(f64::total_cmp);
    let p95 = |values: &[f64]| values[(values.len() - 1) * 95 / 100];
    eprintln!(
        "native scroll: moving_frames={}, active_frames={active_count}, fps={fps:.2}, frame_ms_p50={:.3}, frame_ms_p95={:.3}, subtree_cpu_ms_p95={:.3}",
        moving.len(),
        intervals[intervals.len() / 2],
        p95(&intervals),
        p95(&cpu)
    );
    eprintln!(
        "native scroll worst: frame_ms_max={:.3}, subtree_cpu_ms_max={:.3}, native_rows_max={:?}, complete_source_frames={}",
        intervals.last().unwrap(),
        cpu.last().unwrap(),
        moving.iter().filter_map(|sample| sample.native_rows).max(),
        moving
            .iter()
            .filter(|sample| sample.native_rows.is_some_and(|rows| rows > 64))
            .count()
    );
    // Native hardware acceptance can guard the reported occasional validation spike as well
    // as average cadence. Ordinary test-platform runs do not impose a machine-specific limit.
    if let Ok(maximum) = std::env::var("NANOBUG_SCROLL_MAX_FRAME_MS") {
        let maximum: f64 = maximum
            .parse()
            .expect("NANOBUG_SCROLL_MAX_FRAME_MS is positive");
        assert!(maximum > 0. && maximum.is_finite());
        assert!(
            *intervals.last().unwrap() <= maximum,
            "actual moving frame {:.3} ms must stay within {maximum} ms",
            intervals.last().unwrap()
        );
    }
    if let Ok(minimum) = std::env::var("NANOBUG_SCROLL_MIN_FPS") {
        let minimum: f64 = minimum
            .parse()
            .expect("NANOBUG_SCROLL_MIN_FPS is a positive number");
        assert!(minimum > 0. && minimum.is_finite());
        // The collapsed fixture places successive hunks forty source rows apart. A viewport
        // crossing a disclosure needs their complete original input envelope, including that
        // hidden interval; the uncollapsed fixture needs only the physical viewport's rows.
        let native_budget = if std::env::var_os("NANOBUG_DIFF_NATIVE_COLLAPSED").is_some() {
            128
        } else {
            32
        };
        assert!(
            moving
                .iter()
                .filter(|sample| sample.active)
                .all(|sample| sample.native_rows.is_none_or(|rows| rows <= native_budget)),
            "warmed comparison scrolling must not perform a complete source layout"
        );
        assert!(
            active_count >= 100,
            "high-refresh acceptance requires at least 100 actual foreground moving frames"
        );
        assert!(
            fps >= minimum,
            "actual moving-frame rate {fps:.2} must reach {minimum}"
        );
        assert!(
            p95(&cpu) < 1000. / minimum,
            "native subtree CPU cost must fit the frame budget"
        );
    }
}
