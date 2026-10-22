//! Optional drawing and input contracts, independent of terminal state and native form geometry.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Canvas {
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
    /// `ui.canvas` 1.2: range movement addressed to one still-current scroll content.
    /// Only this event can cross a drawing revision; it grants no pointer, key or text authority.
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
