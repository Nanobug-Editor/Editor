//! Alignment correspondence is rebuilt synchronously from native sources; layout is revision scoped.
use super::*;
use std::{cell::Cell, rc::Rc};

pub(super) struct GroupState {
    pub sources: Vec<AlignmentPane>,
    /// Immutable correspondence shared with paint closures instead of cloning source-row arrays.
    pub rows: Option<Rc<AlignedRows>>,
    pub measurements: Vec<Option<Measurement>>,
    pub frames: Vec<Option<geometry::Projection>>,
    /// Horizontal placement describes painted pixels even during native/independent scrolling.
    pub gutters: Vec<Option<gutter::Projection>>,
    /// Stable outer widths and native chrome, retained across folds and content-only rebuilds.
    pub columns: Vec<Option<scroll::ColumnMetrics>>,
    /// Comparison gutters face inward, preserve reading clearance and use one far-right y rail.
    pub inner_gutters: bool,
    /// Optional retained comparison choices; no folding policy is imposed on ordinary code groups.
    pub folding: Option<UnchangedFolds>,
    pub viewports: Vec<Option<Bounds<Pixels>>>,
    pub regions: Vec<PointerRegion>,
    pub layout: Option<AlignedLayout>,
    pub display_viewport: Option<geometry::DisplayViewport>,
    pub scroll: Rc<Cell<Pixels>>,
    pub status: AlignmentStatus,
    pub epoch: u64,
    pub observed_scroll: Pixels,
    contents: Vec<content::NativeContent>,
    observed_cursors: Vec<Option<(usize, usize)>>,
    pending_reveal: Option<usize>,
    reservation: Option<Reservation>,
    pub retired: bool,
    enabled: bool,
    native_overlays: bool,
    /// Source pane/byte, membership epoch and viewport anchor survive only until valid geometry.
    pending_location: Option<(usize, usize, u64, f32)>,
    // Whole physical pane bounds at a disjoint-layout refusal. Fallback removes local rails;
    // comparing these outer bounds avoids treating that internal change as a resize retry.
    disjoint_bounds: Option<Vec<Bounds<Pixels>>>,
}

/// Work held by a live row plan. Returning it is idempotent and follows the model lifetime,
/// including retained old frames; explicit retirement returns it immediately and prevents revival.
struct Reservation {
    ledger: Rc<Cell<usize>>,
    cells: usize,
}

impl Reservation {
    fn release(&mut self) {
        self.ledger
            .set(self.ledger.get().saturating_add(self.cells));
        self.cells = 0;
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        self.release();
    }
}

impl Default for GroupState {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            rows: None,
            measurements: Vec::new(),
            frames: Vec::new(),
            gutters: Vec::new(),
            columns: Vec::new(),
            inner_gutters: false,
            folding: None,
            viewports: Vec::new(),
            regions: Vec::new(),
            layout: None,
            display_viewport: None,
            scroll: Rc::new(Cell::new(px(0.))),
            status: AlignmentStatus::Measuring,
            epoch: 0,
            observed_scroll: px(0.),
            contents: Vec::new(),
            observed_cursors: Vec::new(),
            pending_reveal: None,
            reservation: None,
            retired: false,
            enabled: true,
            native_overlays: false,
            pending_location: None,
            disjoint_bounds: None,
        }
    }
}

impl GroupState {
    /// Offscreen locations need one complete native measurement before exact byte projection.
    pub(super) fn needs_location_geometry(&self, index: usize) -> bool {
        self.pending_location
            .is_some_and(|(pane, _, _, _)| pane == index)
    }

    pub fn same_members(&self, sources: &[AlignmentPane]) -> bool {
        self.sources.len() == sources.len()
            && self
                .sources
                .iter()
                .zip(sources)
                .all(|(old, new)| old.editor == new.editor)
    }

    pub fn same_revisions(&self, sources: &[AlignmentPane]) -> bool {
        self.sources
            .iter()
            .zip(sources)
            .all(|(old, new)| old.revision == new.revision && old.label == new.label)
    }

    pub fn configure(
        &mut self,
        sources: Vec<AlignmentPane>,
        ledger: Rc<Cell<usize>>,
        cx: &mut gpui_kit::Context<Self>,
    ) {
        if !self.same_members(&sources) {
            self.observed_cursors = vec![None; sources.len()];
            self.viewports = vec![None; sources.len()];
            self.gutters = vec![None; sources.len()];
            self.columns = vec![None; sources.len()];
            self.pending_reveal = None;
        }
        self.sources = sources;
        self.capture_contents(cx);
        self.adopt_ledger(ledger);
        self.invalidate(cx);
        self.rebuild(cx);
    }

    /// A correspondence map addresses one actual native-buffer generation. Public document
    /// revisions alone are insufficient because an uncommitted IME candidate can change bytes.
    pub fn invalidate(&mut self, cx: &mut gpui_kit::Context<Self>) {
        if self.retired {
            return;
        }
        self.epoch = self.epoch.wrapping_add(1);
        self.rows = None;
        self.layout = None;
        self.display_viewport = None;
        self.measurements = vec![None; self.sources.len()];
        self.frames = vec![None; self.sources.len()];
        self.status = AlignmentStatus::Measuring;
        self.pending_location = None;
        self.disjoint_bounds = None;
        cx.notify();
    }

    pub fn rebuild(&mut self, cx: &mut gpui_kit::Context<Self>) {
        // Native IME preedit only notifies its entity; it does not emit InputEvent::Change. Also
        // check at the render boundary so notification ordering cannot paint stale byte ranges.
        self.refresh_content(cx);
        if self.retired
            || self.rows.is_some()
            || matches!(self.status, AlignmentStatus::Unavailable(_))
        {
            return;
        }
        let Some(reservation) = self.reservation.as_mut() else {
            return;
        };
        reservation.release();
        let before = reservation.ledger.get();
        let mut budget = before;
        let sources = self
            .sources
            .iter()
            .map(|pane| pane.editor.read(cx))
            .collect::<Vec<_>>();
        if sources
            .iter()
            .any(|source| source.text().len() > MAX_SOURCE_BYTES)
        {
            self.status = AlignmentStatus::Unavailable(AlignmentError::Budget);
            return;
        }
        // These immutable admission snapshots die after constructing a byte-range/slot model.
        // All subsequent rendering, composition and document writes read the existing EditorState.
        let snapshots = sources
            .iter()
            .map(|source| source.value().to_string())
            .collect::<Vec<_>>();
        let borrowed = snapshots.iter().map(String::as_str).collect::<Vec<_>>();
        match AlignedRows::new_with_budget(&borrowed, &mut budget) {
            Ok(rows) => {
                if let Some(folding) = &self.folding {
                    folding.plan(&rows);
                }
                self.rows = Some(Rc::new(rows));
            }
            Err(_) => self.status = AlignmentStatus::Unavailable(AlignmentError::Budget),
        }
        reservation.ledger.set(budget);
        reservation.cells = before - budget;
    }

    /// Observe actual native-buffer identity, including same-length preedit replacement/cancel.
    /// Notifications for cursor, focus, layout or scrolling keep the existing row plan and epoch.
    pub fn refresh_content(&mut self, cx: &mut gpui_kit::Context<Self>) -> bool {
        if self.retired {
            return false;
        }
        let changed = self.contents.len() != self.sources.len()
            || self
                .sources
                .iter()
                .zip(&self.contents)
                .any(|(source, previous)| !previous.matches(source.editor.read(cx).text()));
        if changed {
            self.capture_contents(cx);
            self.invalidate(cx);
        }
        changed
    }

    fn capture_contents(&mut self, cx: &App) {
        self.contents = self
            .sources
            .iter()
            .map(|source| content::NativeContent::new(source.editor.read(cx).text()))
            .collect();
    }

    pub fn ledger(&self) -> Option<Rc<Cell<usize>>> {
        self.reservation
            .as_ref()
            .map(|reserved| reserved.ledger.clone())
    }

    pub fn adopt_ledger(&mut self, ledger: Rc<Cell<usize>>) {
        if self
            .reservation
            .as_ref()
            .is_some_and(|old| Rc::ptr_eq(&old.ledger, &ledger))
        {
            return;
        }
        self.reservation = Some(Reservation { ledger, cells: 0 });
    }

    pub fn effective_status(&self) -> AlignmentStatus {
        if self.retired {
            AlignmentStatus::Unavailable(AlignmentError::Retired)
        } else if !self.enabled {
            AlignmentStatus::Disabled
        } else if self.native_overlays {
            AlignmentStatus::NativeOverlays
        } else {
            self.status
        }
    }

    pub fn retire(&mut self, cx: &mut gpui_kit::Context<Self>) {
        if self.retired {
            return;
        }
        self.retired = true;
        self.epoch = self.epoch.wrapping_add(1);
        self.rows = None;
        self.layout = None;
        self.display_viewport = None;
        self.frames.fill(None);
        self.gutters.fill(None);
        self.reservation.take();
        self.contents.clear();
        self.pending_location = None;
        self.disjoint_bounds = None;
        self.status = AlignmentStatus::Unavailable(AlignmentError::Retired);
        cx.notify();
    }

    pub fn set_enabled(&mut self, enabled: bool, cx: &mut gpui_kit::Context<Self>) {
        if self.retired || self.enabled == enabled {
            return;
        }
        let was_active = self.enabled && !self.native_overlays;
        self.enabled = enabled;
        self.pause_changed(was_active, cx);
    }

    pub fn refresh_native_overlays(&mut self, cx: &mut gpui_kit::Context<Self>) -> bool {
        if self.retired {
            return false;
        }
        let open = self.sources.iter().any(|source| {
            let editor = source.editor.read(cx);
            editor.search_session().open || editor.code_action_menu_state().open
        });
        if self.native_overlays != open {
            let was_active = self.enabled && !self.native_overlays;
            self.native_overlays = open;
            self.pause_changed(was_active, cx);
        }
        open
    }

    fn pause_changed(&mut self, was_active: bool, cx: &mut gpui_kit::Context<Self>) {
        let active = self.enabled && !self.native_overlays;
        if was_active == active {
            return;
        }
        for region in &self.regions {
            region.request_cancel();
        }
        if !active {
            // Preserve the visible source location when native search/menus take over the viewport.
            for (index, source) in self.sources.iter().enumerate() {
                let y = self.native_scroll(index);
                source.editor.update(cx, |editor, cx| {
                    editor.set_scroll_offset(gpui_kit::point(editor.scroll_offset().x, -y), cx)
                });
            }
        } else {
            self.epoch = self.epoch.wrapping_add(1);
            self.measurements.fill(None);
            self.frames.fill(None);
            self.layout = None;
            self.display_viewport = None;
            if !matches!(self.status, AlignmentStatus::Unavailable(_)) {
                self.status = AlignmentStatus::Measuring;
            }
        }
        cx.notify();
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
        self.pending_location = Some((index, byte, self.epoch, anchor));
        self.apply_location(cx);
        cx.notify();
        true
    }

    fn apply_location(&mut self, cx: &mut gpui_kit::Context<Self>) {
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

    /// Commit the current pane's actual input frame; return whether projected peers need repaint.
    /// Absolute translation/caret changes keep the existing row layout and never re-diff source.
    pub fn measured(
        &mut self,
        index: usize,
        epoch: u64,
        measured: Measurement,
        cx: &mut gpui_kit::Context<Self>,
    ) -> bool {
        if epoch != self.epoch || index >= self.sources.len() {
            return false;
        }
        let old = self.measurements[index].as_ref();
        let row_geometry_changed = old.is_none_or(|old| {
            old.line_height != measured.line_height
                || (!Rc::ptr_eq(&old.logical_rows, &measured.logical_rows)
                    && old.logical_rows != measured.logical_rows)
        });
        let observed = (measured.cursor, measured.text_len);
        if self.observed_cursors[index].is_some_and(|old| old != observed) {
            self.pending_reveal = Some(index);
            if let (Some(folding), Some(rows)) = (&self.folding, &self.rows)
                && folding.reveal(rows, index, measured.cursor)
            {
                self.layout = None;
            }
        }
        self.observed_cursors[index] = Some(observed);
        let changed = old != Some(&measured);
        if !changed {
            return false;
        }
        self.measurements[index] = Some(measured);
        if self.rows.is_none() || self.measurements.iter().any(Option::is_none) {
            return false;
        }
        let Some(display_viewport) =
            geometry::DisplayViewport::from_measurements(&self.measurements)
        else {
            let outer = self
                .measurements
                .iter()
                .enumerate()
                .filter_map(|(index, measure)| measure.as_ref().map(|measure| (index, measure)))
                .map(|(index, m)| {
                    let mut bounds = m.viewport;
                    bounds.size.width = self.independent_width(index, bounds.size.width);
                    // Fallback removes only the rail actually reserved by the aligned panes.
                    // A phantom rail would misclassify every fallback paint as a new resize.
                    bounds.size.height += self.horizontal_rail_height();
                    bounds
                })
                .collect();
            for region in &self.regions {
                region.request_cancel();
            }
            self.fail_geometry(index, epoch, cx);
            self.disjoint_bounds = Some(outer);
            return true;
        };
        let old_scroll = self.scroll.get();
        let old_status = self.status;
        let old_viewport = self.display_viewport;
        // Moving source pixels or a caret does not change native wrapping/alignment. In
        // particular, publishing a completed scroll frame must not schedule all peers again.
        let layout_changed =
            (row_geometry_changed || self.layout.is_none()) && self.measure_layout();
        if self.layout.is_some() && self.status == AlignmentStatus::Ready {
            self.display_viewport = Some(display_viewport);
            self.disjoint_bounds = None;
            self.clamp_scroll();
            if let Some(index) = self.pending_reveal.take() {
                self.reveal(index);
            }
            self.apply_location(cx);
        }
        let redraw = layout_changed
            || old_scroll != self.scroll.get()
            || old_status != self.status
            || old_viewport != self.display_viewport;
        if redraw {
            cx.notify();
        }
        redraw
    }

    /// Rebuild only source-relative alignment after actual row geometry or fold-plan changes.
    /// All native measurements must already exist; no absolute screen coordinate enters this map.
    fn measure_layout(&mut self) -> bool {
        #[cfg(test)]
        geometry::LAYOUT_BUILDS.with(|count| count.set(count.get() + 1));
        let rows = self.rows.as_ref().expect("admitted row plan");
        let all = self
            .measurements
            .iter()
            .flatten()
            .map(|m| m.logical_rows.to_vec())
            .collect::<Vec<_>>();
        let folds = self
            .folding
            .as_ref()
            .map_or_else(Vec::new, UnchangedFolds::collapsed);
        let line = self
            .measurements
            .iter()
            .flatten()
            .map(|measure| f32::from(measure.line_height))
            .fold(0_f32, f32::max);
        match rows.layout_with_folds(&all, &folds, line) {
            Ok(layout) => {
                let changed = self.layout.as_ref() != Some(&layout);
                self.layout = Some(layout);
                self.status = AlignmentStatus::Ready;
                changed
            }
            Err(_) => {
                self.status = AlignmentStatus::Unavailable(AlignmentError::Geometry);
                true
            }
        }
    }

    pub fn fail_geometry(&mut self, index: usize, epoch: u64, cx: &mut gpui_kit::Context<Self>) {
        if epoch != self.epoch || index >= self.measurements.len() {
            return;
        }
        self.status = AlignmentStatus::Unavailable(AlignmentError::Geometry);
        self.layout = None;
        self.display_viewport = None;
        self.frames.fill(None);
        self.disjoint_bounds = None;
        cx.notify();
    }

    /// Re-admit a previously disjoint group only after a real pane move/resize. Its admitted
    /// source correspondence stays valid, so geometry recovery neither copies text nor spends
    /// another diff reservation. A native independent drag is released before projection resumes.
    pub fn retry_disjoint_viewport(
        &mut self,
        index: usize,
        viewport: Bounds<Pixels>,
        cx: &mut gpui_kit::Context<Self>,
    ) -> bool {
        if self.effective_status() != AlignmentStatus::Unavailable(AlignmentError::Geometry)
            || !self
                .disjoint_bounds
                .as_ref()
                .and_then(|bounds| bounds.get(index))
                .is_some_and(|old| *old != viewport)
        {
            return false;
        }
        for region in &self.regions {
            region.request_cancel();
        }
        self.disjoint_bounds = None;
        self.epoch = self.epoch.wrapping_add(1);
        self.measurements.fill(None);
        self.frames.fill(None);
        self.layout = None;
        self.display_viewport = None;
        self.status = AlignmentStatus::Measuring;
        cx.notify();
        true
    }

    pub fn max_scroll(&self) -> Pixels {
        let height = self
            .layout
            .as_ref()
            .map_or(0., |layout| layout.display_height);
        let viewport = self.display_viewport.map_or(px(0.), |view| view.height);
        (px(height) - viewport).max(px(0.))
    }

    pub fn clamp_scroll(&self) {
        self.scroll
            .set(self.scroll.get().clamp(px(0.), self.max_scroll()));
    }

    pub fn scroll_by(&mut self, delta: Pixels, cx: &mut gpui_kit::Context<Self>) {
        self.scroll
            .set((self.scroll.get() + delta).clamp(px(0.), self.max_scroll()));
        self.observed_scroll = self.scroll.get();
        cx.notify();
    }

    /// Reveal the actual painted native caret in the projected viewport, retaining end-of-wrap
    /// affinity through complete admission and bounded scroll frames without guessing byte geometry.
    pub fn reveal(&mut self, index: usize) {
        let Some(measurement) = self.measurements.get(index).and_then(Option::as_ref) else {
            return;
        };
        let Some(caret) = measurement.caret else {
            return;
        };
        let Some(layout) = self
            .layout
            .as_ref()
            .and_then(|layout| layout.panes.get(index))
        else {
            return;
        };
        let Some(top) = layout.source_to_display(measurement.source_y(caret)) else {
            return;
        };
        let top = px(top);
        let bottom = top + caret.size.height;
        if top < self.scroll.get() {
            self.scroll.set(top);
        }
        let height = self.display_viewport.map_or(px(0.), |view| view.height);
        if bottom > self.scroll.get() + height {
            self.scroll.set(bottom - height);
        }
        self.clamp_scroll();
    }
}
