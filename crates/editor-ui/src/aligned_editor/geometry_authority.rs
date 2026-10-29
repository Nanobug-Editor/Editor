//! Bound the original input mount independently of distant, collapsed source intervals.
use super::*;

/// Only the invisible input window's native scroll changes; source selection and text do not.
#[derive(Clone, Default)]
pub(super) struct Position(Rc<Cell<Option<(u64, Pixels, Pixels, Pixels)>>>);

impl Position {
    /// Return the prepared offset for content `epoch`, display `scroll` and default `native` offset.
    /// A different frame key retires the prepared window and returns `native`.
    pub fn read(&self, epoch: u64, scroll: Pixels, native: Pixels) -> Pixels {
        self.0
            .get()
            .filter(|old| (old.0, old.1, old.2) == (epoch, scroll, native))
            .map_or(native, |old| old.3)
    }

    /// Prepare the source at `offset`, scoped to content `epoch`, display `scroll` and default
    /// `native` offset, before dispatching a native pointer gesture.
    pub fn set(&self, epoch: u64, scroll: Pixels, native: Pixels, offset: Pixels) {
        self.0.set(Some((epoch, scroll, native, offset)));
    }
}

/// Size the native input window from the physical `viewport` plus `line` overscan. Hidden source
/// rows must never enlarge it; the pointer router prepares another source window before dispatch.
pub(super) fn height(viewport: Option<geometry::PaneViewport>, line: Pixels) -> Pixels {
    let Some(viewport) = viewport else {
        return line * 2.;
    };
    viewport.height + viewport.native_inset_y + line * 2.
}

/// Use `pane`, physical `viewport` and the last `measured` caret to keep input near a visible caret.
/// `native` is the default source offset for display `scroll`; incomplete geometry keeps that default.
pub(super) fn caret_scroll(
    pane: Option<&crate::aligned_rows::PaneLayout>,
    viewport: Option<geometry::PaneViewport>,
    measured: Option<&Measurement>,
    scroll: Pixels,
    native: Pixels,
) -> Pixels {
    let (Some(pane), Some(viewport), Some(measured)) = (pane, viewport, measured) else {
        return native;
    };
    let Some(caret) = measured.caret else {
        return native;
    };
    let source = measured.source_y(caret);
    let Some(display) = pane.source_to_display(source).map(px) else {
        return native;
    };
    if display + measured.line_height <= scroll || display >= scroll + viewport.height {
        return native;
    }
    (px(source) - (display - scroll)).max(px(0.))
}
