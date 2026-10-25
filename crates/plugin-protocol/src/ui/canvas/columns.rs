//! `ui.canvas 1.6` native text-column resize declarations and completion boundaries.
use super::*;

/// Native frames stretch bound TextBoxes while dragging; only release emits ColumnsResized.
/// Keep `epoch` through drawing-only revisions; replace it when column meaning changes (including ABA).
/// All values use canvas-local logical pixels. Grid/IME and transformed viewport canvases are excluded.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnResize {
    /// Nonzero column identity; drawing-only revisions retain it.
    pub epoch: u64,
    /// First column's left edge in untransformed canvas-local logical pixels.
    pub x: f32,
    /// Two to 32 adjacent widths with a fixed total; each divider affects only its adjacent pair.
    pub widths: Vec<f32>,
    /// One nonnegative lower bound per width; callers scale these for narrow viewports.
    pub minimum: Vec<f32>,
    /// Each paint index occurs once and must identify a TextBox in this exact drawing.
    pub bindings: Vec<ColumnBinding>,
}
/// The bound rectangle moves/stretches with its column, retaining its original internal padding.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnBinding {
    /// Index of one TextBox in the same canvas paint array.
    pub paint: usize,
    /// Zero-based column whose width and left-edge deltas apply to that rectangle.
    pub column: usize,
}

impl ColumnResize {
    /// An event may outlive a drawing revision, never its semantic identity or outer geometry.
    pub(in crate::ui) fn accepts(&self, epoch: u64, widths: &[f32]) -> bool {
        epoch == self.epoch
            && widths.len() == self.widths.len()
            && widths
                .iter()
                .zip(&self.minimum)
                .all(|(w, m)| w.is_finite() && *w >= *m - 0.01 && *w <= 1_000_000.)
            && (widths.iter().sum::<f32>() - self.widths.iter().sum::<f32>()).abs() < 0.25
    }
}
impl Canvas {
    pub(super) fn validate_columns(&self) -> Result<(), String> {
        let Some(columns) = &self.column_resize else {
            return Ok(());
        };
        let mut bound = std::collections::BTreeSet::new();
        if self.grid
            || self.caret.is_some()
            || columns.epoch == 0
            || !columns.x.is_finite()
            || columns.x < 0.
            || columns.x > 1_000_000.
            || !(2..=32).contains(&columns.widths.len())
            || columns.minimum.len() != columns.widths.len()
            || columns.bindings.len() > 32_000
            || !columns.widths.iter().zip(&columns.minimum).all(|(w, m)| {
                w.is_finite() && m.is_finite() && *m >= 0. && *w >= *m && *w <= 1_000_000.
            })
            || columns.x + columns.widths.iter().sum::<f32>() > 1_000_000.
            || columns.bindings.iter().any(|binding| {
                binding.column >= columns.widths.len()
                    || !bound.insert(binding.paint)
                    || !matches!(
                        self.paint.get(binding.paint),
                        Some(crate::Paint::TextBox { .. })
                    )
            })
        {
            return Err("Invalid native canvas columns".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{Action, Document, Kind, Node, UiEvent};
    /// Epoch, geometry and binding validation keep late releases confined to their source columns.
    #[test]
    fn native_columns_bound_events_and_advertise_their_capability() {
        let mut canvas = Canvas {
            column_resize: Some(ColumnResize {
                epoch: 3,
                x: 0.,
                widths: vec![200., 200.],
                minimum: vec![80., 80.],
                bindings: vec![],
            }),
            ..Default::default()
        };
        canvas.validate().unwrap();
        let mut document = Document::new(Node::new("grid", Kind::Canvas(canvas.clone())));
        document.revision = 4;
        assert_eq!(
            document.required_capabilities()["ui.canvas"],
            semver::Version::new(1, 6, 0)
        );
        let event = |epoch, widths| UiEvent {
            revision: 2,
            node: "grid".into(),
            action: Action::Canvas(CanvasEvent::ColumnsResized { epoch, widths }),
        };
        document
            .validate_event(&event(3, vec![220., 180.]))
            .unwrap();
        assert!(
            document
                .validate_event(&event(2, vec![220., 180.]))
                .is_err()
        );
        assert!(document.validate_event(&event(3, vec![60., 340.])).is_err());
        assert!(
            document
                .validate_event(&event(3, vec![220., 200.]))
                .is_err()
        );
        canvas
            .column_resize
            .as_mut()
            .unwrap()
            .bindings
            .push(ColumnBinding {
                paint: 0,
                column: 0,
            });
        assert!(
            canvas.validate().is_err(),
            "bindings cannot point outside paint or to non-TextBoxes"
        );
    }
}
