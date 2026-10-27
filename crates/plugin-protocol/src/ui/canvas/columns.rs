//! Native text-column gestures (1.6) and viewport fitting (1.11) share one drawing identity.
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
    /// Two to 32 adjacent snapshot widths; each divider affects only its adjacent pair.
    /// Their total stays fixed unless viewport fitting is explicitly enabled.
    pub widths: Vec<f32>,
    /// One nonnegative lower bound per width. With viewport fitting, keep these uncompressed;
    /// the native surface scales them only when the actual viewport is narrower than their sum.
    pub minimum: Vec<f32>,
    /// Each paint index occurs once and must identify a TextBox in this exact drawing.
    pub bindings: Vec<ColumnBinding>,
    /// `ui.canvas 1.11`: fit one flexible column to native width changes before a worker reply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewport: Option<ColumnViewport>,
}

/// A drawing reference width anchors the table's fixed outer margins during native resizing.
/// The flexible column absorbs available width first; below its minimum, other excess widths
/// shrink proportionally. Below the sum of minima, all minima scale to fit. Widening restores
/// the original preferred widths rather than accumulating rounded resize deltas.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnViewport {
    /// Finite drawing reference width, including both outer margins, in logical pixels.
    /// It may exceed actual bounds: publish uncompressed preferences, never fitted narrow widths.
    /// Bound text and span fills must use this same reference to avoid applying the delta twice.
    pub width: f32,
    /// Zero-based column which absorbs viewport growth and yields width first on shrink.
    pub flexible: usize,
    /// Unique Fill paint indices whose widths follow the total table width, not an inner divider.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub span_paints: Vec<usize>,
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
    /// Releases retain their semantic identity. Opted-in viewport fitting may precede the latest
    /// worker Resize reply; its total infers the actual viewport using the declared fixed margins.
    pub(in crate::ui) fn accepts(&self, epoch: u64, widths: &[f32]) -> bool {
        let total = widths.iter().sum::<f32>();
        let minimum_total = self.minimum.iter().sum::<f32>();
        let minimum_scale = if self.viewport.is_some() && minimum_total > 0. {
            (total / minimum_total).clamp(0., 1.)
        } else {
            1.
        };
        let geometry = if let Some(viewport) = &self.viewport {
            let width = viewport.width + total - self.widths.iter().sum::<f32>();
            width.is_finite() && (0. ..=1_000_000.).contains(&width)
        } else {
            (total - self.widths.iter().sum::<f32>()).abs() < 0.25
        };
        epoch == self.epoch
            && widths.len() == self.widths.len()
            && widths.iter().zip(&self.minimum).all(|(w, m)| {
                w.is_finite() && *w >= (*m * minimum_scale - 0.01).max(0.) && *w <= 1_000_000.
            })
            && geometry
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
        if let Some(viewport) = &columns.viewport {
            let mut spans = std::collections::BTreeSet::new();
            if !viewport.width.is_finite()
                || viewport.width <= 0.
                || viewport.width > 1_000_000.
                || columns.x + columns.widths.iter().sum::<f32>() > viewport.width + 0.25
                || viewport.flexible >= columns.widths.len()
                || viewport.span_paints.len() > 32_000
                || viewport.span_paints.iter().any(|index| {
                    !spans.insert(*index)
                        || !matches!(self.paint.get(*index), Some(crate::Paint::Fill { .. }))
                })
            {
                return Err("Invalid native column viewport".into());
            }
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
                viewport: None,
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

    /// Fitting is explicitly negotiated and cannot bind arbitrary paint or bypass epoch/minimum gates.
    #[test]
    fn fitted_columns_validate_capability_geometry_and_fill_bindings() {
        let mut canvas = Canvas {
            column_resize: Some(ColumnResize {
                epoch: 7,
                x: 0.,
                widths: vec![300., 100.],
                minimum: vec![80., 40.],
                bindings: vec![],
                viewport: Some(ColumnViewport {
                    width: 408.,
                    flexible: 0,
                    span_paints: vec![0],
                }),
            }),
            paint: vec![crate::Paint::Fill {
                rect: crate::Rect {
                    x: 0.,
                    y: 0.,
                    w: 408.,
                    h: 28.,
                },
                color: 0,
                extend_to_bottom: false,
            }],
            ..Default::default()
        };
        let document = Document::new(Node::new("table", Kind::Canvas(canvas.clone())));
        document.validate().unwrap();
        assert_eq!(
            document.required_capabilities()["ui.canvas"],
            semver::Version::new(1, 11, 0)
        );
        let columns = canvas.column_resize.as_ref().unwrap();
        assert!(columns.accepts(7, &[200., 100.]));
        assert!(columns.accepts(7, &[40., 20.]));
        assert!(!columns.accepts(6, &[200., 100.]));
        assert!(!columns.accepts(7, &[10., 100.]));
        assert!(!columns.accepts(7, &[f32::NAN, 100.]));
        for viewport in [
            ColumnViewport {
                width: 300.,
                flexible: 0,
                span_paints: vec![],
            },
            ColumnViewport {
                width: 408.,
                flexible: 2,
                span_paints: vec![],
            },
            ColumnViewport {
                width: 408.,
                flexible: 0,
                span_paints: vec![0, 0],
            },
            ColumnViewport {
                width: 408.,
                flexible: 0,
                span_paints: vec![1],
            },
        ] {
            canvas.column_resize.as_mut().unwrap().viewport = Some(viewport);
            assert!(canvas.validate().is_err());
        }
    }
}
