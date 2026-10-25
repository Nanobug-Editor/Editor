//! VT effects are values returned to the host; this library never owns a PTY or OS event loop.

use crate::screen::ClipboardType;
use std::{fmt, sync::Arc};
use vte::ansi::Rgb;

/// Effect produced while parsing a session's bytes; the owner checks authority before applying it.
#[derive(Clone)]
pub enum Event {
    /// Mouse-reporting modes changed; the view may need a new pointer cursor.
    MouseCursorDirty,
    /// Replace the terminal-supplied title, independently from its user-assigned tab name.
    Title(String),
    /// Clear the terminal-supplied title.
    ResetTitle,
    /// OSC52 clipboard write request; task output alone does not grant clipboard authority.
    ClipboardStore(ClipboardType, String),
    /// Clipboard read request and encoder; the host can decline it under its existing policy.
    ClipboardLoad(
        ClipboardType,
        Arc<dyn Fn(&str) -> String + Sync + Send + 'static>,
    ),
    /// Dynamic color query: palette index and formatter for the owner's resolved RGB color.
    ColorRequest(usize, Arc<dyn Fn(Rgb) -> String + Sync + Send + 'static>),
    /// Reply bytes for the same owned process that supplied the parsed stream.
    PtyWrite(String),
    /// Size query formatted from the requesting session's actual native geometry.
    TextAreaSizeRequest(Arc<dyn Fn(WindowSize) -> String + Sync + Send + 'static>),
    /// The application's explicit cursor blink policy changed.
    CursorBlinkingChange,
    /// Visual/audible bell intent; the owner applies the user's notification settings.
    Bell,
}

impl fmt::Debug for Event {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MouseCursorDirty => output.write_str("MouseCursorDirty"),
            Self::Title(title) => output.debug_tuple("Title").field(title).finish(),
            Self::ResetTitle => output.write_str("ResetTitle"),
            Self::ClipboardStore(kind, text) => output
                .debug_tuple("ClipboardStore")
                .field(kind)
                .field(text)
                .finish(),
            Self::ClipboardLoad(kind, _) => {
                output.debug_tuple("ClipboardLoad").field(kind).finish()
            }
            Self::ColorRequest(index, _) => {
                output.debug_tuple("ColorRequest").field(index).finish()
            }
            Self::PtyWrite(bytes) => output.debug_tuple("PtyWrite").field(bytes).finish(),
            Self::TextAreaSizeRequest(_) => output.write_str("TextAreaSizeRequest"),
            Self::CursorBlinkingChange => output.write_str("CursorBlinkingChange"),
            Self::Bell => output.write_str("Bell"),
        }
    }
}

/// Current viewport dimensions used only to encode a VT size reply.
#[derive(Copy, Clone, Debug)]
pub struct WindowSize {
    /// Visible rows, excluding scrollback.
    pub num_lines: u16,
    /// Visible character columns.
    pub num_cols: u16,
    /// Measured glyph-cell width in logical pixels.
    pub cell_width: u16,
    /// Measured glyph-cell height in logical pixels.
    pub cell_height: u16,
}

/// Internal effect sink connects the screen to its owning Engine's per-parse reply collection.
pub(crate) trait EventListener {
    fn send_event(&self, event: Event);
}

/// Algorithm fixtures intentionally discard host effects instead of creating process resources.
#[cfg(test)]
pub(crate) struct VoidListener;

#[cfg(test)]
impl EventListener for VoidListener {
    fn send_event(&self, _: Event) {}
}
