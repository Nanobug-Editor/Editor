//! Native adjacent-column gestures and transparent resize handles, independent of plugin workers.
use gpui_kit::{
    AnyElement, CursorStyle, InteractiveElement, IntoElement, ParentElement, Styled, div, px,
};

/// Only presentation widths are retained: providers remain responsible for their data and persistence.
#[derive(Default)]
pub struct ColumnResize {
    /// Provider-owned identity; a changed value retires gestures and unacknowledged widths.
    epoch: u64,
    /// Logical left edge of the first column.
    origin: f32,
    /// Latest provider widths, used as the reference for text rectangle projection.
    base: Vec<f32>,
    /// Locally previewed widths, retained through late drawing replies.
    widths: Vec<f32>,
    /// Uncompressed provider or local intent and its original viewport, never cumulative deltas.
    preferred: Layout,
    /// Optional native fit policy.
    viewport: Option<Viewport>,
    /// Latest native width, independent of the provider snapshot's timestamp.
    measured_width: Option<f32>,
    /// Validated lower bound for each adjacent column.
    minimum: Vec<f32>,
    /// Provider and local prefix positions keep each text projection constant time.
    base_edges: Vec<f32>,
    edges: Vec<f32>,
    /// Captured separator gesture; its starting widths stay fixed until release.
    drag: Option<Drag>,
    /// Latest submitted widths awaiting acknowledgement, independent of a newer uncommitted drag.
    pending: Option<Layout>,
}
/// A provider opts into local reflow by declaring its artwork's viewport and one flexible column.
#[derive(Clone, Copy)]
pub struct Viewport {
    /// Drawing reference width, including fixed margins; may exceed a narrow actual viewport.
    pub width: f32,
    /// Column to stretch first; other columns yield excess width only after it reaches its minimum.
    pub flexible: usize,
}
/// A width vector is always paired with the viewport where it was chosen, so shrinking is reversible.
#[derive(Clone, Default)]
struct Layout {
    /// Preferred column widths before any temporary viewport compression.
    widths: Vec<f32>,
    /// Viewport where these widths were authored; absent for fixed-total layouts.
    viewport_width: Option<f32>,
}
/// A press fixes the adjacent pair and anchor, so reversing a clamped drag remains continuous.
struct Drag {
    /// Left column of the captured adjacent pair.
    index: usize,
    /// Logical press position before any width changes.
    anchor: f32,
    /// Widths at press time, independent of subsequent drawings.
    widths: Vec<f32>,
}

impl ColumnResize {
    /// Synchronize a validated declaration. Drawing-only replies cannot roll back an active gesture
    /// or an unacknowledged release. `viewport` permits frame-local fitting; without it the total
    /// width stays fixed. `epoch`, `origin`, widths and minima describe this provider snapshot.
    pub fn configure(
        &mut self,
        epoch: u64,
        origin: f32,
        widths: &[f32],
        minimum: &[f32],
        viewport: Option<Viewport>,
    ) {
        let reset = self.epoch != epoch
            || self.origin != origin
            || self.base.len() != widths.len()
            || self.viewport.map(|value| value.flexible) != viewport.map(|value| value.flexible)
            || (viewport.is_none()
                && (self.base.iter().sum::<f32>() - widths.iter().sum::<f32>()).abs() > 0.25)
            || self.minimum != minimum;
        if reset {
            self.drag = None;
            self.pending = None;
        }
        self.viewport = viewport;
        self.minimum = minimum.to_vec();
        if self.pending.as_ref().is_some_and(|submitted| {
            fitted(
                submitted,
                viewport,
                viewport.map(|value| value.width),
                minimum,
            )
            .iter()
            .zip(widths)
            .all(|(a, b)| (a - b).abs() < 0.25)
        }) {
            self.pending = None;
        }
        if self.drag.is_none() && self.pending.is_none() {
            self.preferred = Layout {
                widths: widths.to_vec(),
                viewport_width: viewport.map(|value| value.width),
            };
        }
        self.epoch = epoch;
        self.origin = origin;
        self.base = widths.to_vec();
        self.base_edges = edges(origin, &self.base);
        self.reflow();
    }
    /// Fit the current intent directly to `width` measured this native frame, with no guest event.
    /// Return whether geometry changed so callers can refresh transparent cursor handles.
    pub fn resize_viewport(&mut self, width: f32) -> bool {
        if self.viewport.is_none()
            || !width.is_finite()
            || width < 0.
            || self.measured_width == Some(width)
        {
            return false;
        }
        self.measured_width = Some(width);
        self.reflow();
        true
    }
    /// Always derive the projection from uncompressed intent; narrow viewports never erase preferences.
    fn reflow(&mut self) {
        self.widths = fitted(
            &self.preferred,
            self.viewport,
            self.measured_width,
            &self.minimum,
        );
        self.edges = edges(self.origin, &self.widths);
    }
    /// Clear all ownership when the containing surface is disabled, replaced or transformed.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    /// The five-pixel band is transparent and shares its geometry with the cursor handles.
    pub fn begin(&mut self, x: f32) -> bool {
        self.drag = self
            .edges
            .iter()
            .enumerate()
            .skip(1)
            .take(self.widths.len().saturating_sub(1))
            .find(|(_, edge)| (x - **edge).abs() <= 5.)
            .map(|(edge, _)| Drag {
                index: edge - 1,
                anchor: x,
                widths: self.widths.clone(),
            });
        self.drag.is_some()
    }
    /// Constant work per column, with absolute positions so clamping can always be reversed.
    pub fn move_to(&mut self, x: f32) -> bool {
        let Some(drag) = &self.drag else {
            return false;
        };
        if !x.is_finite() {
            return false;
        }
        let i = drag.index;
        let total = drag.widths.iter().sum::<f32>();
        let minimum_total = self.minimum.iter().sum::<f32>();
        let scale = if self.viewport.is_some() && minimum_total > 0. {
            (total / minimum_total).min(1.)
        } else {
            1.
        };
        let delta = (x - drag.anchor).clamp(
            self.minimum[i] * scale - drag.widths[i],
            (drag.widths[i + 1] - self.minimum[i + 1] * scale)
                .max(self.minimum[i] * scale - drag.widths[i]),
        );
        let mut next = drag.widths.clone();
        next[i] += delta;
        next[i + 1] -= delta;
        if next == self.widths {
            return false;
        }
        self.widths = next;
        self.preferred = Layout {
            widths: self.widths.clone(),
            viewport_width: self
                .viewport
                .map(|value| self.measured_width.unwrap_or(value.width)),
        };
        self.edges = edges(self.origin, &self.widths);
        true
    }
    /// Emit one final value for a real change. Keep it locally until the provider acknowledges it.
    pub fn finish(&mut self) -> Option<Vec<f32>> {
        let drag = self.drag.take()?;
        if drag.widths == self.widths {
            return None;
        }
        self.pending = Some(self.preferred.clone());
        Some(self.widths.clone())
    }
    /// Cancel unfinished presentation, leaving an already submitted release intact.
    pub fn cancel(&mut self) {
        if self.drag.take().is_some() {
            self.preferred = self.pending.clone().unwrap_or_else(|| Layout {
                widths: self.base.clone(),
                viewport_width: self.viewport.map(|value| value.width),
            });
        }
        self.reflow();
    }
    /// Whether the native surface currently owns the pointer, including outside its original bounds.
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }
    /// Current internal boundaries; the outside edges are never resize targets.
    pub fn dividers(&self) -> impl Iterator<Item = f32> + '_ {
        self.edges
            .iter()
            .copied()
            .skip(1)
            .take(self.widths.len().saturating_sub(1))
    }
    /// Stretch a declared text rectangle by its column's width delta, retaining native text/clip rules.
    pub fn project(&self, column: usize, x: f32, width: f32) -> (f32, f32) {
        if column >= self.widths.len() {
            return (x, width);
        }
        (
            x + self.edges[column] - self.base_edges[column],
            (width + self.widths[column] - self.base[column]).max(0.),
        )
    }
    /// A full-row background follows only the table's total width, never an inner separator.
    pub fn span_width(&self, width: f32) -> f32 {
        (width + self.widths.iter().sum::<f32>() - self.base.iter().sum::<f32>()).max(0.)
    }
    /// Transparent GPUI hit areas use the system horizontal resize cursor, without drawing a line.
    pub fn handles(&self) -> AnyElement {
        div()
            .absolute()
            .left_0()
            .top_0()
            .size_full()
            .children(self.dividers().enumerate().map(|(index, x)| {
                div()
                    .id(("column-resize", index))
                    .absolute()
                    .left(px(x - 5.))
                    .top_0()
                    .bottom_0()
                    .w(px(10.))
                    .cursor(CursorStyle::ResizeColumn)
            }))
            .into_any_element()
    }
}
/// Reflow one immutable preferred layout into the actual viewport while preserving outer margins.
fn fitted(
    layout: &Layout,
    viewport: Option<Viewport>,
    measured: Option<f32>,
    minimum: &[f32],
) -> Vec<f32> {
    let (Some(viewport), Some(source), Some(target)) = (viewport, layout.viewport_width, measured)
    else {
        return layout.widths.clone();
    };
    if viewport.flexible >= layout.widths.len() || minimum.len() != layout.widths.len() {
        return layout.widths.clone();
    }
    let mut result = layout.widths.clone();
    let available = (result.iter().sum::<f32>() + target - source).max(0.);
    let floor = minimum.iter().sum::<f32>();
    if available < floor && floor > 0. {
        return minimum
            .iter()
            .map(|value| value * available / floor)
            .collect();
    }
    let index = viewport.flexible;
    result[index] = (result[index] + target - source).max(minimum[index]);
    let excess = (result.iter().sum::<f32>() - available).max(0.);
    if excess > 0. {
        let spare = result
            .iter()
            .zip(minimum)
            .map(|(value, floor)| (value - floor).max(0.))
            .sum::<f32>();
        if spare > 0. {
            for (value, floor) in result.iter_mut().zip(minimum) {
                *value -= (*value - floor).max(0.) * (excess / spare).min(1.);
            }
        }
    }
    result
}
/// Cached prefix positions make each bound text rectangle's projection constant time.
fn edges(origin: f32, widths: &[f32]) -> Vec<f32> {
    let mut result = vec![origin];
    for width in widths {
        result.push(result.last().unwrap() + width);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Repeated viewport changes keep the trailing cells anchored and restore uncompressed widths.
    #[test]
    fn viewport_fitting_is_reversible_and_preserves_native_release_intent() {
        let mut state = ColumnResize::default();
        let minimum = [104., 40., 24., 100.];
        let viewport = |width| Some(Viewport { width, flexible: 0 });
        state.configure(1, 0., &[300., 100., 80., 112.], &minimum, viewport(600.));
        for width in [560., 640., 590., 620.] {
            state.resize_viewport(width);
            let (x, w) = state.project(3, 484., 104.);
            assert_eq!(x + w, width - 12.);
            assert_eq!(state.span_width(600.), width);
        }
        state.resize_viewport(260.);
        assert!(state.widths.iter().all(|width| *width >= 0.));
        assert!((state.widths.iter().sum::<f32>() - 252.).abs() < 0.01);
        // A narrow reply keeps the tail's canonical preference instead of publishing fitted widths.
        state.configure(1, 0., &[104., 100., 80., 112.], &minimum, viewport(404.));
        state.resize_viewport(600.);
        assert_eq!(state.widths, [300., 100., 80., 112.]);
        state.resize_viewport(560.);
        assert!(state.begin(360.));
        assert!(state.move_to(380.));
        assert_eq!(state.finish().unwrap(), [260., 120., 60., 112.]);
        state.resize_viewport(600.);
        // Neither an old snapshot nor an acknowledgement rendered at the previous viewport can jump.
        state.configure(1, 0., &[300., 100., 80., 112.], &minimum, viewport(600.));
        assert_eq!(state.widths, [300., 120., 60., 112.]);
        state.configure(1, 0., &[260., 120., 60., 112.], &minimum, viewport(560.));
        assert_eq!(state.widths, [300., 120., 60., 112.]);
        state.configure(1, 0., &[300., 110., 70., 112.], &minimum, viewport(600.));
        assert_eq!(state.widths, [300., 110., 70., 112.]);
    }

    /// Mid-width compression is identical before/after a reply, including a user edit while narrow.
    #[test]
    fn canonical_narrow_reply_keeps_all_columns_and_accepts_narrow_drag() {
        let mut state = ColumnResize::default();
        let minimum = [104., 40., 24., 100.];
        let viewport = |width| Some(Viewport { width, flexible: 0 });
        state.configure(1, 0., &[300., 100., 80., 112.], &minimum, viewport(600.));
        state.resize_viewport(340.);
        assert_eq!(state.widths, [104., 70., 52., 106.]);
        state.configure(1, 0., &[104., 100., 80., 112.], &minimum, viewport(404.));
        assert_eq!(state.widths, [104., 70., 52., 106.]);
        assert!(state.begin(174.));
        assert!(state.move_to(184.));
        assert_eq!(state.finish().unwrap(), [104., 80., 42., 106.]);
        // The guest's Resize still says 600, but the changed tail carries the real native choice.
        state.configure(1, 0., &[364., 80., 42., 106.], &minimum, viewport(600.));
        assert_eq!(state.widths, [104., 80., 42., 106.]);
        assert!(state.pending.is_none());
        state.resize_viewport(600.);
        assert_eq!(state.widths, [364., 80., 42., 106.]);
    }

    /// A delayed viewport snapshot is not an external pointer move and cannot cancel a held divider.
    #[test]
    fn viewport_reply_during_internal_drag_preserves_capture_and_fixed_background() {
        let mut state = ColumnResize::default();
        let minimum = [104., 40., 24., 100.];
        state.configure(
            1,
            0.,
            &[300., 100., 80., 112.],
            &minimum,
            Some(Viewport {
                width: 600.,
                flexible: 0,
            }),
        );
        state.resize_viewport(560.);
        assert!(state.begin(360.));
        state.move_to(370.);
        state.configure(
            1,
            0.,
            &[260., 100., 80., 112.],
            &minimum,
            Some(Viewport {
                width: 560.,
                flexible: 0,
            }),
        );
        assert!(state.dragging());
        state.move_to(380.);
        assert_eq!(state.widths, [260., 120., 60., 112.]);
        assert_eq!(state.span_width(560.), 560.);
        state.cancel();
        assert_eq!(state.widths, [260., 100., 80., 112.]);
    }

    /// A worker may return old drawings for arbitrarily many frames without delaying native motion.
    #[test]
    fn adjacent_resize_is_immediate_reversible_and_waits_for_ack() {
        let mut state = ColumnResize::default();
        state.configure(
            1,
            20.,
            &[300., 100., 80., 170.],
            &[104., 40., 24., 100.],
            None,
        );
        assert!(state.begin(420.));
        for x in 421..=460 {
            assert!(state.move_to(x as f32));
        }
        assert_eq!(state.project(2, 424., 72.), (464., 32.));
        assert_eq!(state.project(3, 504., 162.), (504., 162.));
        let result = state.finish().unwrap();
        assert_eq!(result, [300., 140., 40., 170.]);
        state.configure(
            1,
            20.,
            &[300., 100., 80., 170.],
            &[104., 40., 24., 100.],
            None,
        );
        assert_eq!(state.project(2, 424., 72.), (464., 32.));
        state.configure(1, 20., &result, &[104., 40., 24., 100.], None);
        assert_eq!(state.project(2, 464., 32.), (464., 32.));
        assert!(state.begin(460.));
        state.move_to(10000.);
        state.move_to(450.);
        assert_eq!(state.finish().unwrap(), [300., 130., 50., 170.]);
        state.configure(
            2,
            20.,
            &[300., 100., 80., 170.],
            &[104., 40., 24., 100.],
            None,
        );
        assert!(!state.dragging());
        assert!(!state.move_to(500.));
    }

    /// Cancelling a newer preview must preserve the older release and still recognize its acknowledgement.
    #[test]
    fn cancelled_second_gesture_preserves_only_the_submitted_preview() {
        for acknowledge_before_cancel in [false, true] {
            let mut state = ColumnResize::default();
            state.configure(1, 0., &[200., 200.], &[80., 80.], None);
            assert!(state.begin(200.));
            assert!(state.move_to(240.));
            assert_eq!(state.finish().unwrap(), [240., 160.]);
            assert!(state.begin(240.));
            assert!(state.move_to(260.));
            if acknowledge_before_cancel {
                state.configure(1, 0., &[240., 160.], &[80., 80.], None);
            }
            state.cancel();
            assert_eq!(state.dividers().collect::<Vec<_>>(), [240.]);
            assert!(state.finish().is_none(), "cancelled preview has no release");
            if !acknowledge_before_cancel {
                state.configure(1, 0., &[240., 160.], &[80., 80.], None);
            }
            state.configure(1, 0., &[250., 150.], &[80., 80.], None);
            assert_eq!(
                state.dividers().collect::<Vec<_>>(),
                [250.],
                "acknowledged previews cannot hide subsequent provider updates"
            );
        }
    }
}
