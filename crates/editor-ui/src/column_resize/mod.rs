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
    /// Validated lower bound for each adjacent column.
    minimum: Vec<f32>,
    /// Provider and local prefix positions keep each text projection constant time.
    base_edges: Vec<f32>,
    edges: Vec<f32>,
    /// Captured separator gesture; its starting widths stay fixed until release.
    drag: Option<Drag>,
    /// A released preview awaits matching provider widths.
    pending: bool,
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
    /// or an unacknowledged release; a new identity or changed outer geometry retires both.
    pub fn configure(&mut self, epoch: u64, origin: f32, widths: &[f32], minimum: &[f32]) {
        let reset = self.epoch != epoch
            || self.origin != origin
            || self.base.len() != widths.len()
            || (self.base.iter().sum::<f32>() - widths.iter().sum::<f32>()).abs() > 0.25
            || self.minimum != minimum;
        if reset {
            self.drag = None;
            self.pending = false;
        }
        if self.pending
            && self
                .widths
                .iter()
                .zip(widths)
                .all(|(a, b)| (a - b).abs() < 0.25)
        {
            self.pending = false;
        }
        if self.drag.is_none() && !self.pending {
            self.widths = widths.to_vec();
        }
        self.epoch = epoch;
        self.origin = origin;
        self.base = widths.to_vec();
        self.minimum = minimum.to_vec();
        self.base_edges = edges(origin, &self.base);
        self.edges = edges(origin, &self.widths);
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
        let delta = (x - drag.anchor).clamp(
            self.minimum[i] - drag.widths[i],
            drag.widths[i + 1] - self.minimum[i + 1],
        );
        let mut next = drag.widths.clone();
        next[i] += delta;
        next[i + 1] -= delta;
        if next == self.widths {
            return false;
        }
        self.widths = next;
        self.edges = edges(self.origin, &self.widths);
        true
    }
    /// Emit one final value for a real change. Keep it locally until the provider acknowledges it.
    pub fn finish(&mut self) -> Option<Vec<f32>> {
        let drag = self.drag.take()?;
        if drag.widths == self.widths {
            return None;
        }
        self.pending = true;
        Some(self.widths.clone())
    }
    /// Cancel unfinished presentation, leaving an already submitted release intact.
    pub fn cancel(&mut self) {
        self.drag = None;
        if !self.pending {
            self.widths.clone_from(&self.base);
            self.edges = edges(self.origin, &self.widths);
        }
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
    /// A worker may return old drawings for arbitrarily many frames without delaying native motion.
    #[test]
    fn adjacent_resize_is_immediate_reversible_and_waits_for_ack() {
        let mut state = ColumnResize::default();
        state.configure(1, 20., &[300., 100., 80., 170.], &[104., 40., 24., 100.]);
        assert!(state.begin(420.));
        for x in 421..=460 {
            assert!(state.move_to(x as f32));
        }
        assert_eq!(state.project(2, 424., 72.), (464., 32.));
        assert_eq!(state.project(3, 504., 162.), (504., 162.));
        let result = state.finish().unwrap();
        assert_eq!(result, [300., 140., 40., 170.]);
        state.configure(1, 20., &[300., 100., 80., 170.], &[104., 40., 24., 100.]);
        assert_eq!(state.project(2, 424., 72.), (464., 32.));
        state.configure(1, 20., &result, &[104., 40., 24., 100.]);
        assert_eq!(state.project(2, 464., 32.), (464., 32.));
        assert!(state.begin(460.));
        state.move_to(10000.);
        state.move_to(450.);
        assert_eq!(state.finish().unwrap(), [300., 130., 50., 170.]);
        state.configure(2, 20., &[300., 100., 80., 170.], &[104., 40., 24., 100.]);
        assert!(!state.dragging());
        assert!(!state.move_to(500.));
    }
}
