//! Optional drawing and input contracts, independent of terminal state and native form geometry.
use serde::{Deserialize, Serialize};
mod text_measure;
pub use text_measure::*;
mod columns;
pub use columns::*;
mod scene;
pub use scene::*;
mod paragraph;
pub use paragraph::*;
mod text_layout;
pub use text_layout::*;
mod targets;
pub use targets::*;
mod text_index;
pub use text_index::*;
mod pointer;
pub use pointer::*;
mod text_input;
pub use text_input::*;
mod semantics;
pub use semantics::*;
mod placement;
pub use placement::*;
mod overlay;
pub use overlay::*;
#[cfg(test)]
mod overlay_tests;
#[cfg(test)]
mod paragraph_tests;
#[cfg(test)]
mod scene_tests;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Canvas {
    /// `ui.canvas 1.9`: ordinary native children, in foreground order, anchored to live scene targets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overlays: Vec<CanvasOverlay>,
    /// `ui.canvas 1.8`: bounded native point queries for named paragraphs in this exact scene.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text_queries: Vec<CanvasTextQuery>,
    /// `ui.canvas 1.7`: structured artwork; mutually exclusive with nonempty legacy `paint`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene: Option<CanvasScene>,
    /// `ui.canvas 1.6`: transparent native column handles and frame-local TextBox resizing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column_resize: Option<ColumnResize>,
    /// Optional typography for custom grids; omitted fields inherit the selected theme role.
    #[serde(default)]
    pub font: crate::FontStyle,
    /// Native scrollbar range. Content remains guest-owned; scrolling emits an absolute pixel offset.
    #[serde(default)]
    pub scroll: Option<ScrollRange>,
    /// `ui.canvas` 1.2: nonzero identity of the current scroll content. Keep it through
    /// drawing-only repaints; change it when content/order/meaning changes, including A → B → A.
    /// Opted-in deltas and scrollbar positions survive older UI revisions with this same identity.
    /// Other input remains scene-bound. Omission preserves the original exact-revision contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scroll_epoch: Option<u64>,
    /// `ui.canvas` 1.3: retained content interval, in absolute vertical content pixels.
    /// Paint coordinates still refer to `scroll.offset`. The host translates them each native
    /// frame while a replacement drawing is in flight, and maps pointer coordinates back.
    /// Supply overscan around the viewport; motion beyond this interval waits for new artwork.
    /// Requires a scroll epoch, excludes grid/IME canvases and transformed visual viewports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scroll_buffer: Option<ScrollBuffer>,
    /// `ui.canvas` 1.5: bounded sample groups whose widest native text advance is returned as
    /// [`CanvasEvent::TextMeasured`]. Results use untransformed logical pixels and this scene's
    /// revision; keep IDs stable only while their text and typography retain the same meaning.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub measure_text: Vec<TextMeasure>,
    pub paint: Vec<crate::Paint>,
    /// Ordinary image surfaces do not claim text focus or IME; interactive canvases explicitly opt in.
    #[serde(default)]
    pub focusable: bool,
    /// Requires ui.grid negotiation; without it Resize contains no character-cell metrics.
    #[serde(default)]
    pub grid: bool,
    /// Optional IME anchor in canvas-local coordinates. It does not imply an editable document.
    #[serde(default)]
    pub caret: Option<crate::Rect>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScrollRange {
    pub content: f32,
    pub offset: f32,
}

/// The complete vertical interval painted by a virtualized canvas, including its overscan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScrollBuffer {
    /// First covered content pixel, inclusive.
    pub start: f32,
    /// Last covered content pixel, exclusive; must not exceed the scroll content extent.
    pub end: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GridMetrics {
    pub cell_width: f32,
    pub cell_height: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointerPhase {
    Down,
    Move,
    Up,
}

/// Events target their enclosing UiEvent node and revision, never a global plugin input stream.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CanvasEvent {
    /// `ui.canvas 1.8`: stable target input bound to an actual native presentation.
    Input {
        input: CanvasInput,
    },
    /// Native point-query results from successfully painted artwork; ordinary exact UI revision applies.
    TextLayout {
        /// Scene identity and drawing version actually queried.
        identity: u64,
        /// Exact scene revision, independent of the enclosing UI revision.
        scene_revision: u64,
        /// Original text authority; never borrowed from a replacement source.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<crate::api::DocumentVersion>,
        /// Original binary authority when the view is file-bound.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        file: Option<crate::api::FileVersion>,
        /// The visual viewport used by the native layout.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        projection: Option<super::VisualViewport>,
        /// One ordered result per declared query, with a native font binding or explicit error.
        results: Vec<CanvasTextResult>,
    },
    /// `ui.canvas 1.7`: the native paint reached this exact scene; an error reports failed preparation.
    ScenePresented {
        /// Current nonzero scene identity, never a replacement or retired owner.
        identity: u64,
        /// Scene drawing version actually painted, independent of the outer UI revision.
        scene_revision: u64,
        /// Text source authority at actual preparation time; retained only through unrelated repaint.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<crate::api::DocumentVersion>,
        /// Binary source authority is independent of optional editable text.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        file: Option<crate::api::FileVersion>,
        /// Viewport declaration actually painted; a changed projection needs a new display receipt.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        projection: Option<super::VisualViewport>,
        /// Bounded preparation diagnostic; absent means artwork was submitted to GPUI.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// `ui.canvas 1.6`: final widths after a native gesture, matched by epoch and fixed total width.
    ColumnsResized {
        epoch: u64,
        widths: Vec<f32>,
    },
    /// `ui.canvas` 1.5: one width for every requested sample group, in request order.
    /// This is an exact-scene geometry reply, never an exception to ordinary revision checks.
    TextMeasured {
        widths: Vec<TextWidth>,
    },
    /// `ui.canvas` 1.2: range movement addressed to one still-current scroll content.
    /// A matching content epoch can cross a drawing revision; it grants no pointer, key or text authority.
    ScrollInput {
        epoch: u64,
        change: ScrollChange,
    },
    Scroll {
        offset: f32,
    },
    Resize {
        width: f32,
        height: f32,
        grid: Option<GridMetrics>,
    },
    Focus {
        focused: bool,
    },
    Key {
        key: String,
        ctrl: bool,
        alt: bool,
        shift: bool,
    },
    Text {
        text: String,
    },
    Pointer {
        phase: PointerPhase,
        x: f32,
        y: f32,
        button: u8,
        clicks: u8,
        shift: bool,
    },
    Wheel {
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
        shift: bool,
    },
}

/// A range movement uses logical pixels; apply each delta in order and clamp to the native viewport.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScrollChange {
    /// Native wheel convention: positive moves toward the start, negative toward the end.
    Delta { delta_y: f32 },
    /// A native scrollbar's absolute, nonnegative content offset.
    To { offset: f32 },
}

impl Canvas {
    /// Validate geometry and bound expensive vectors before allocating host rendering resources.
    pub fn validate(&self) -> Result<(), String> {
        self.validate_overlays()?;
        if let Some(scene) = &self.scene {
            if !self.paint.is_empty() || self.column_resize.is_some() || self.grid {
                return Err(
                    "A structured scene excludes legacy paint, columns and character grids".into(),
                );
            }
            scene.validate()?;
        }
        self.validate_measurements()?;
        self.validate_text_queries()?;
        self.validate_columns()?;
        if let Some(buffer) = &self.scroll_buffer {
            let valid = self.scroll_epoch.is_some()
                && !self.grid
                && self.caret.is_none()
                && self.scroll.as_ref().is_some_and(|range| {
                    buffer.start.is_finite()
                        && buffer.end.is_finite()
                        && buffer.start >= 0.
                        && buffer.start <= range.offset
                        && buffer.end >= range.offset
                        && buffer.end <= range.content
                });
            if !valid {
                return Err("Invalid retained canvas scroll buffer".into());
            }
        }
        if self
            .scroll_epoch
            .is_some_and(|epoch| epoch == 0 || self.scroll.is_none())
            || self
                .font
                .family
                .as_ref()
                .is_some_and(|font| font.is_empty() || font.len() > 256)
            || self
                .font
                .size_px
                .is_some_and(|size| !size.is_finite() || !(1. ..=128.).contains(&size))
            || self.scroll.as_ref().is_some_and(|range| {
                !range.content.is_finite()
                    || !range.offset.is_finite()
                    || range.content < 0.
                    || range.content > 100_000_000.
                    || range.offset < 0.
                    || range.offset > range.content
            })
        {
            return Err("Invalid canvas typography or scroll range".into());
        }
        let coordinate = |value: f32| value.is_finite() && value.abs() <= 1_000_000.;
        let rect = |rect: &crate::Rect| {
            coordinate(rect.x)
                && coordinate(rect.y)
                && coordinate(rect.w)
                && coordinate(rect.h)
                && rect.w >= 0.
                && rect.h >= 0.
        };
        if self.paint.len() > 32_000 || self.caret.as_ref().is_some_and(|value| !rect(value)) {
            return Err("Invalid canvas geometry or drawing quota".into());
        }
        let mut vectors = 0;
        let mut bytes = 0;
        for paint in &self.paint {
            let valid = match paint {
                crate::Paint::Fill { rect: value, .. } => rect(value),
                crate::Paint::Text {
                    x,
                    y,
                    text,
                    size,
                    font,
                    ..
                } => {
                    bytes += text.len();
                    coordinate(*x)
                        && coordinate(*y)
                        && (1. ..=128.).contains(size)
                        && text.len() <= 65536
                        && font
                            .as_ref()
                            .is_none_or(|font| !font.is_empty() && font.len() <= 256)
                }
                crate::Paint::TextBox {
                    rect: value,
                    text,
                    size,
                    font,
                    ..
                } => {
                    bytes += text.len();
                    rect(value)
                        && (1. ..=128.).contains(size)
                        && text.len() <= 65536
                        && !text.contains(['\r', '\n'])
                        && font
                            .as_ref()
                            .is_none_or(|font| !font.is_empty() && font.len() <= 256)
                }
                crate::Paint::Svg {
                    rect: value,
                    clip,
                    source,
                } => {
                    vectors += 1;
                    bytes += source.len();
                    rect(value) && rect(clip) && !source.is_empty() && source.len() <= 1024 * 1024
                }
            };
            if !valid || vectors > 16 || bytes > 2 * 1024 * 1024 {
                return Err("Invalid canvas paint or content quota".into());
            }
        }
        Ok(())
    }
}
