//! Revision-scoped source positions and shared display edges never mutate native selection.
use super::*;

impl GroupState {
    /// Restore `offset` only after this source incarnation has a complete viewport; reject invalid
    /// input and retired/disabled groups. A later user navigation or content change supersedes it.
    pub(in crate::aligned_editor) fn restore_scroll_offset(
        &mut self,
        offset: Pixels,
        cx: &mut gpui_kit::Context<Self>,
    ) -> bool {
        if self.retired
            || self.sources.is_empty()
            || !self.enabled
            || self.native_overlays
            || matches!(self.status, AlignmentStatus::Unavailable(_))
            || !f32::from(offset).is_finite()
            || offset < px(0.)
        {
            return false;
        }
        self.pending_location = None;
        self.pending_edge = None;
        self.pending_reveal = None;
        self.pending_scroll = Some((offset, self.epoch));
        self.apply_location(cx);
        cx.notify();
        true
    }
    /// Use native row metadata only to position the small input window around a pending byte.
    /// `apply_location` still requires that byte's actual painted glyph bounds before scrolling.
    pub(in crate::aligned_editor) fn location_scroll_hint(
        &self,
        index: usize,
        cx: &App,
    ) -> Option<Pixels> {
        use gpui_base::input::RopeExt as _;
        let (pane, byte, epoch, _) = self.pending_location?;
        if pane != index || epoch != self.epoch {
            return None;
        }
        let source = self.sources.get(index)?.editor.read(cx);
        let rows = &self.rows.as_ref()?.source_rows[index];
        if !geometry_display_map::supported_rows(source, rows) {
            return None;
        }
        let row = source.text().offset_to_point(byte).row;
        let range = source.display_map().buffer_line_to_display_row_range(row)?;
        Some(source.line_height()? * range.start)
    }

    /// Shared display edges include inserted/deleted slots that may be absent from every chosen source byte.
    pub fn reveal_content_edge(&mut self, end: bool, cx: &mut gpui_kit::Context<Self>) -> bool {
        self.pending_scroll = None;
        if self.retired
            || self.sources.is_empty()
            || !self.enabled
            || self.native_overlays
            || matches!(self.status, AlignmentStatus::Unavailable(_))
        {
            return false;
        }
        let mut unfolded = false;
        if let (Some(folding), Some(rows)) = (&self.folding, &self.rows) {
            for (index, source) in self.sources.iter().enumerate() {
                let byte = if end {
                    source.editor.read(cx).text().len()
                } else {
                    0
                };
                unfolded |= folding.reveal(rows, index, byte);
            }
        }
        if unfolded {
            self.folds_changed(cx);
        }
        self.pending_location = None;
        self.pending_edge = Some((end, self.epoch));
        self.apply_location(cx);
        cx.notify();
        true
    }

    /// Wait for this membership's completed display geometry before applying its shared clamp.
    fn apply_edge(&mut self) -> bool {
        let Some((end, epoch)) = self.pending_edge else {
            return false;
        };
        if epoch != self.epoch {
            self.pending_edge = None;
            return false;
        }
        if self.layout.is_some() && self.display_viewport.is_some() {
            self.scroll
                .set(if end { self.max_scroll() } else { px(0.) });
            self.pending_edge = None;
        }
        true
    }

    /// Queue a checked source `byte` in pane `index` at viewport `anchor`, scoped to this epoch.
    pub fn reveal_offset(
        &mut self,
        index: usize,
        byte: usize,
        anchor: f32,
        cx: &mut gpui_kit::Context<Self>,
    ) -> bool {
        if !anchor.is_finite()
            || !(0. ..=1.).contains(&anchor)
            || self.retired
            || !self.enabled
            || self.native_overlays
            || matches!(self.status, AlignmentStatus::Unavailable(_))
        {
            return false;
        }
        let Some(source) = self.sources.get(index) else {
            return false;
        };
        let text = source.editor.read(cx).text();
        if byte > text.len() || !text.is_char_boundary(byte) {
            return false;
        }
        if let (Some(folding), Some(rows)) = (&self.folding, &self.rows)
            && folding.reveal(rows, index, byte)
        {
            self.folds_changed(cx);
        }
        self.pending_edge = None;
        self.pending_location = Some((index, byte, self.epoch, anchor));
        self.pending_scroll = None;
        self.apply_location(cx);
        cx.notify();
        true
    }

    pub(super) fn apply_location(&mut self, cx: &mut gpui_kit::Context<Self>) {
        if let Some((offset, epoch)) = self.pending_scroll {
            if epoch != self.epoch {
                self.pending_scroll = None;
            } else if self.layout.is_some() && self.display_viewport.is_some() {
                self.scroll.set(offset);
                self.clamp_scroll();
                self.pending_scroll = None;
                return;
            } else {
                return;
            }
        }
        if self.apply_edge() {
            return;
        }
        let Some((index, byte, epoch, anchor)) = self.pending_location else {
            return;
        };
        if epoch != self.epoch {
            self.pending_location = None;
            return;
        }
        let Some(measurement) = self.measurements.get(index).and_then(Option::as_ref) else {
            return;
        };
        let Some(layout) = self
            .layout
            .as_ref()
            .and_then(|layout| layout.panes.get(index))
        else {
            return;
        };
        let source = self.sources[index].editor.read(cx);
        // Base's public range lookup is defined only over laid-out logical lines. A byte before
        // the visible range can otherwise resolve to its first line through saturating offsets.
        use gpui_base::input::RopeExt as _;
        let row = source.text().offset_to_point(byte).row;
        if source
            .visible_row_range()
            .is_none_or(|visible| !visible.contains(&row))
        {
            return;
        }
        let Some(native) = source.range_to_bounds(&(byte..byte)) else {
            return;
        };
        if let Some(display) = layout.source_to_display(measurement.source_y(native)) {
            let Some(viewport) = self.display_viewport else {
                return;
            };
            // Use completed content bounds after headers, wrapping and folding; native text
            // and selections remain untouched, including when the target reaches a scroll clamp.
            let within_viewport = (viewport.height - native.size.height).max(px(0.)) * anchor;
            self.scroll.set((px(display) - within_viewport).max(px(0.)));
            self.clamp_scroll();
            self.pending_location = None;
        }
    }
}
