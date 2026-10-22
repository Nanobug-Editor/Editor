//! Bound the original input mount to the source envelope needed by the painted viewport.
use super::*;
use crate::aligned_rows::PaneLayout;

/// Only the invisible input mount moves; display slices and the original editor state do not.
#[derive(Clone, Default)]
pub(super) struct Position(Rc<Cell<Option<(u64, Pixels, Pixels, Pixels)>>>);

impl Position {
    /// A different content epoch, displayed scroll or native source envelope retires the shift.
    pub fn read(&self, epoch: u64, scroll: Pixels, native: Pixels) -> Pixels {
        self.0
            .get()
            .filter(|old| (old.0, old.1, old.2) == (epoch, scroll, native))
            .map_or(px(0.), |old| old.3)
    }

    /// Preserve one geometry-only translation for the current painted source envelope.
    pub fn set(&self, epoch: u64, scroll: Pixels, native: Pixels, translation: Pixels) {
        self.0.set(Some((epoch, scroll, native, translation)));
    }
}

/// Cover every actually visible source interval, including discontinuous intervals separated by
/// collapsed unchanged rows. Offscreen source prefixes/suffixes never enlarge the native mount.
/// Keeping the complete intervening envelope preserves Base's original hit testing and IME;
/// a viewport crossing a distant fold can therefore still require a larger native allocation.
pub(super) fn height(
    pane: Option<&PaneLayout>,
    viewport: Option<geometry::PaneViewport>,
    scroll: Pixels,
    native_scroll: Pixels,
    line: Pixels,
) -> Pixels {
    let Some(viewport) = viewport else {
        return line * 2.;
    };
    let top = f32::from(scroll);
    let bottom = top + f32::from(viewport.height);
    let end = pane
        .into_iter()
        .flat_map(|pane| &pane.slices)
        .filter(|slice| slice.display.end > top && slice.display.start < bottom)
        .map(|slice| slice.source.start + (slice.display.end.min(bottom) - slice.display.start))
        .fold(f32::from(native_scroll + viewport.height), f32::max);
    (px(end) - native_scroll).max(viewport.height) + viewport.native_inset_y + line * 2.
}
