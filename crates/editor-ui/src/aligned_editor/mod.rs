//! Zero-byte visual alignment over existing native editors, without another document or Undo stack.
//!
//! Base paints clipped source intervals and a final transparent authority mount. Local code owns
//! only measured geometry, shared viewport position and the platform-input coordinate bridge.
//! Install `AlignedPointerRouter` around the complete window root before using these panes.

mod content;
mod folding;
#[cfg(test)]
mod folding_tests;
pub use folding::UnchangedFolds;
mod geometry;
mod geometry_authority;
mod geometry_cache;
mod geometry_display_map;
mod geometry_glyphs;
mod geometry_measurement;
mod gutter;
#[cfg(test)]
mod gutter_tests;
mod input;
mod native_viewport;
mod notifications;
mod pane;
mod scroll;
mod state;
#[cfg(test)]
mod tests;

use super::aligned_pointer::{AlignedPointerRouter, PointerRegion};
use super::aligned_rows::{AlignedLayout, AlignedRows};
use geometry::Measurement;
use gpui_base::input::EditorState;
use gpui_kit::{AnyElement, App, AppContext as _, Bounds, Entity, IntoElement, Pixels, px};
use pane::PaneView;
use state::GroupState;
use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;

/// Register the native glyph cache before creating any Base roots.
pub(crate) fn init(cx: &mut App) {
    geometry_glyphs::init(cx);
}

/// Optional host input/session bridge. Alignment changes geometry only, including when linked
/// input queues, clipboard policy or composition logic sit in front of the native EditorState.
pub type AlignmentInputFactory =
    Rc<dyn Fn(Bounds<Pixels>, &mut gpui_kit::Window, &mut App) -> Box<dyn gpui_kit::InputHandler>>;
/// An atomic native control factory; host source frames/popovers stay outside the aligned pane.
pub type AlignmentNativeFactory =
    Rc<dyn Fn(&Entity<EditorState>, &mut gpui_kit::Window, &mut App) -> AnyElement>;

/// An existing source identity, its owner's content revision and its accessible source name.
#[derive(Clone)]
pub struct AlignmentPane {
    pub(crate) editor: Entity<EditorState>,
    pub(crate) revision: u64,
    pub(crate) label: String,
    pub(crate) input_factory: Option<AlignmentInputFactory>,
    pub(crate) native_authority_factory: Option<AlignmentNativeFactory>,
}

impl AlignmentPane {
    /// Describe the original `editor`, its owner's `revision` and accessible source `label`.
    /// Native presentation and input bridges start unset and remain owned by the alignment group.
    pub fn new(editor: Entity<EditorState>, revision: u64, label: String) -> Self {
        Self {
            editor,
            revision,
            label,
            input_factory: None,
            native_authority_factory: None,
        }
    }
}

/// Preparation/geometry failure is observable by the owning surface; it can show its localized
/// independent-scroll fallback instead of silently claiming approximate source alignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignmentError {
    PaneCount,
    DuplicateSource,
    Budget,
    Geometry,
    ActiveGesture,
    Retired,
}

/// The owner can distinguish initial measurement from a completed alignment or explicit fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignmentStatus {
    Measuring,
    Ready,
    Unavailable(AlignmentError),
    Disabled,
    NativeOverlays,
}

/// A two/three-pane presentation group. It retains no mutable copy of any source text.
///
/// Before replacing its editor membership or dropping visible panes, call `retire`. The window
/// root router releases any native drag against the old rendered frame before replacing its input
/// listeners; retained presentation clones cannot revive the retired membership.
#[derive(Clone)]
pub struct AlignmentGroup {
    state: Entity<GroupState>,
    panes: Vec<Entity<PaneView>>,
    regions: Vec<PointerRegion>,
}

impl AlignmentGroup {
    /// Creates an empty group; configure it with exactly two or three distinct existing editors.
    pub fn new(cx: &mut App) -> Self {
        Self {
            state: cx.new(|_| GroupState::default()),
            panes: Vec::new(),
            regions: Vec::new(),
        }
    }

    /// Rebuilds source-row correspondence from the current native texts at these owner revisions.
    /// Same-editor revisions retain focus, native gestures and pane entities. Replacing membership
    /// during an active gesture returns `ActiveGesture`; retire the old regions before retrying.
    pub fn configure(
        &mut self,
        panes: Vec<AlignmentPane>,
        router: &AlignedPointerRouter,
        cx: &mut App,
    ) -> Result<(), AlignmentError> {
        let ledger = self
            .state
            .read(cx)
            .ledger()
            .unwrap_or_else(|| Rc::new(Cell::new(super::aligned_rows::MAX_COMPARISON_ROWS)));
        self.configure_shared(panes, router, ledger, cx)
    }

    /// Shares a scene's remaining input-row budget with other groups. Unchanged cached source
    /// revisions do not charge again; a native Change invalidates that cache before rebuilding.
    pub fn configure_bounded(
        &mut self,
        panes: Vec<AlignmentPane>,
        router: &AlignedPointerRouter,
        budget: &mut usize,
        cx: &mut App,
    ) -> Result<(), AlignmentError> {
        let ledger = Rc::new(Cell::new(*budget));
        let result = self.configure_shared(panes, router, ledger.clone(), cx);
        *budget = ledger.get();
        result
    }

    /// Shares one persistent work reservation ledger across all live groups in a scene. Rebuilds
    /// return their old reservation first, and retirement/drop returns it exactly once.
    pub fn configure_shared(
        &mut self,
        panes: Vec<AlignmentPane>,
        router: &AlignedPointerRouter,
        ledger: Rc<Cell<usize>>,
        cx: &mut App,
    ) -> Result<(), AlignmentError> {
        if self.state.read(cx).retired {
            return Err(AlignmentError::Retired);
        }
        if !(2..=3).contains(&panes.len()) {
            return Err(AlignmentError::PaneCount);
        }
        for (index, pane) in panes.iter().enumerate() {
            if panes[..index].iter().any(|old| old.editor == pane.editor) {
                return Err(AlignmentError::DuplicateSource);
            }
        }
        let same_members = self.state.read(cx).same_members(&panes);
        if same_members
            && self.state.read(cx).same_revisions(&panes)
            && self.state.read(cx).rows.is_some()
        {
            // Session bridges can change while the document content revision stays the same.
            self.state.update(cx, |state, _| {
                state.sources = panes;
                state.adopt_ledger(ledger);
            });
            return Ok(());
        }
        if !same_members && self.regions.iter().any(|region| !region.clear()) {
            return Err(AlignmentError::ActiveGesture);
        }
        self.state
            .update(cx, |state, cx| state.configure(panes, ledger, cx));
        if !same_members {
            self.panes.clear();
            self.regions.clear();
            for index in 0..self.state.read(cx).sources.len() {
                let region = router.register();
                let state = self.state.clone();
                let own_region = region.clone();
                self.panes
                    .push(cx.new(|cx| PaneView::new(state, index, own_region, cx)));
                self.regions.push(region);
            }
            self.state
                .update(cx, |state, _| state.regions = self.regions.clone());
        }
        match self.status(cx) {
            AlignmentStatus::Unavailable(error) => Err(error),
            _ => Ok(()),
        }
    }

    /// Returns the persistent pane view, which owns the native composition/geometry bridge.
    pub fn pane(&self, index: usize) -> Option<AnyElement> {
        self.panes
            .get(index)
            .map(|pane| pane.clone().into_any_element())
    }

    /// Regions are root-owned lifecycle seams. Never discard an active lease without `retire`.
    pub fn regions(&self) -> &[PointerRegion] {
        &self.regions
    }

    /// Permanently revoke this membership before removing it from the scene. The window-root
    /// router releases any drag against the still-rendered old native frame in its next prepaint.
    /// Retained overlay/frame clones cannot revive a retired group or spend its returned budget.
    pub fn retire(&self, cx: &mut App) {
        for region in &self.regions {
            region.request_cancel();
        }
        self.state.update(cx, |state, cx| state.retire(cx));
    }

    /// Refresh only the current host source frame's input/control adapters. The expected native
    /// identity rejects late source-frame work; correspondence, revisions and region leases stay put.
    pub fn set_presentation(
        &self,
        index: usize,
        expected_editor: &Entity<EditorState>,
        input: Option<AlignmentInputFactory>,
        native: Option<AlignmentNativeFactory>,
        cx: &mut App,
    ) -> bool {
        let changed = self.state.update(cx, |group, _| {
            let Some(source) = group.sources.get_mut(index) else {
                return false;
            };
            if source.editor != *expected_editor {
                return false;
            }
            source.input_factory = input;
            source.native_authority_factory = native;
            true
        });
        // Called while the owner composes this same frame. Factories are presentation props;
        // notifying unconditionally here would turn fresh per-render closures into idle work.
        changed
    }

    /// Convert a current original-native rectangle for completion, hover or diagnostic overlays.
    pub fn project_bounds(
        &self,
        index: usize,
        native: Bounds<Pixels>,
        cx: &App,
    ) -> Option<Bounds<Pixels>> {
        self.state.read(cx).project(index, native)
    }

    /// A source-owned callback may outlive its frame. Capture only a weak group so an editor's
    /// persistent context-menu callback cannot keep its own source/group membership alive.
    pub fn bounds_projector(
        &self,
        index: usize,
    ) -> Rc<dyn Fn(Bounds<Pixels>, &App) -> Option<Bounds<Pixels>>> {
        let state = self.state.downgrade();
        Rc::new(move |native, cx| {
            let state = state.upgrade()?;
            let group = state.read(cx);
            (!group.retired)
                .then(|| group.project(index, native))
                .flatten()
        })
    }

    /// Project a native mouse anchor, whose gutter segment has a different x translation than text.
    pub fn project_pointer_bounds(
        &self,
        index: usize,
        native: Bounds<Pixels>,
        cx: &App,
    ) -> Option<Bounds<Pixels>> {
        self.state.read(cx).project_pointer(index, native)
    }

    /// Weak source-menu callback; it cannot retain a closed source through its own native state.
    pub fn pointer_projector(
        &self,
        index: usize,
    ) -> Rc<dyn Fn(Bounds<Pixels>, &App) -> Option<Bounds<Pixels>>> {
        let state = self.state.downgrade();
        Rc::new(move |native, cx| state.upgrade()?.read(cx).project_pointer(index, native))
    }

    /// The actual physical pane rectangle from its most recent completed native paint.
    pub fn viewport(&self, index: usize, cx: &App) -> Option<Bounds<Pixels>> {
        self.state.read(cx).viewports.get(index)?.as_ref().copied()
    }

    /// Reports the real bounded model/measurement state for localized fallback presentation.
    pub fn status(&self, cx: &App) -> AlignmentStatus {
        self.state.read(cx).effective_status()
    }

    /// A user sync toggle pauses the complete group and preserves every native session/history.
    pub fn set_enabled(&self, enabled: bool, cx: &mut App) {
        self.state
            .update(cx, |state, cx| state.set_enabled(enabled, cx));
    }

    /// Use inward comparison gutters, the first source's reading clearance and one far-right
    /// shared vertical rail. Native input widths balance those insets before wrapping text.
    /// This changes presentation only; native overlay/independent fallback behavior remains intact.
    pub fn set_inner_gutters(&self, enabled: bool, cx: &mut App) {
        self.state.update(cx, |state, cx| {
            if !state.retired && state.inner_gutters != enabled {
                state.inner_gutters = enabled;
                cx.notify();
            }
        });
    }

    /// Attach the retained view's shared unchanged-region choices to this native membership.
    pub fn set_folding(&self, folding: UnchangedFolds, cx: &mut App) {
        self.state.update(cx, |state, cx| {
            if state.retired {
                return;
            }
            if let Some(rows) = &state.rows {
                folding.plan(rows);
            }
            state.folding = Some(folding);
            state.folds_changed(cx);
        });
    }

    /// Collapse or expand every unchanged region without mutating native text or selection.
    pub fn fold_unchanged(&self, collapsed: bool, cx: &mut App) {
        self.state
            .update(cx, |state, cx| state.set_folds(collapsed, cx));
    }

    /// Native search/code-action panels live in the original atomic control. Pause every pane
    /// while any such panel is open so those controls remain visible and keyboard accessible.
    pub fn refresh_native_overlays(&self, cx: &mut App) -> bool {
        self.state
            .update(cx, |state, cx| state.refresh_native_overlays(cx))
    }

    /// Reveal a current source byte without moving its native selection or taking keyboard focus.
    /// Before layout it remains a revision-scoped pending location rather than guessed pixels.
    pub fn reveal_offset(&self, index: usize, byte: usize, cx: &mut App) -> bool {
        self.reveal_offset_at(index, byte, 0., cx)
    }

    /// Place source `byte` from pane `index` at `anchor` of the measured content viewport.
    /// Zero anchors its row at the top and 0.5 centers it; finite anchors in 0..=1 are accepted.
    /// Returns false for invalid/stale sources or unavailable alignment; native scroll limits apply.
    pub fn reveal_offset_at(&self, index: usize, byte: usize, anchor: f32, cx: &mut App) -> bool {
        self.state
            .update(cx, |state, cx| state.reveal_offset(index, byte, anchor, cx))
    }

    /// Reveal the shared display's start (`end=false`) or end, including rows absent from some panes.
    /// Queues against the current source epoch until layout is ready, without changing selection/focus.
    /// Returns false when retired, disabled or in native/independent fallback, like `reveal_offset_at`.
    pub fn reveal_content_edge(&self, end: bool, cx: &mut App) -> bool {
        self.state
            .update(cx, |state, cx| state.reveal_content_edge(end, cx))
    }

    /// Project a current UTF-8 source range for rails, decorations or source-owned controls.
    /// Hidden/offscreen/stale measurements return `None`, never coordinates from another pane.
    pub fn project_range(
        &self,
        index: usize,
        range: Range<usize>,
        cx: &App,
    ) -> Option<Bounds<Pixels>> {
        let group = self.state.read(cx);
        let source = group.sources.get(index)?;
        let native = source.editor.read(cx);
        if range.end > native.text().len()
            || range.start > range.end
            || !native.text().is_char_boundary(range.start)
            || !native.text().is_char_boundary(range.end)
        {
            return None;
        }
        use gpui_base::input::RopeExt as _;
        let visible = native.visible_row_range()?;
        if !visible.contains(&native.text().offset_to_point(range.start).row)
            || !visible.contains(&native.text().offset_to_point(range.end).row)
        {
            return None;
        }
        let bounds = native.range_to_bounds(&range)?;
        let projected = group.project(index, bounds)?;
        let viewport = if matches!(group.effective_status(), AlignmentStatus::Ready) {
            // Headers/native padding can leave zero-byte space above or below the shared source
            // window. Rails must not appear there merely because the outer pane contains it.
            group.frames.get(index)?.as_ref()?.viewport
        } else {
            *group.viewports.get(index)?.as_ref()?
        };
        // A moved line-start caret sits exactly on the pane's left edge. Strict rectangle
        // intersection rejects its zero width; use a subpixel visibility probe, not a text offset.
        let mut visibility = projected;
        visibility.size.width = visibility.size.width.max(px(0.5));
        viewport.intersects(&visibility).then_some(projected)
    }

    /// Shared positive vertical viewport position; Base's original source ScrollHandle stays native.
    pub fn scroll_offset(&self, cx: &App) -> Pixels {
        self.state.read(cx).scroll.get()
    }
}

/// Limit full native layout independently from line correspondence and raw source-byte budgets.
/// The old 240,000 px ceiling rejected ordinary 10k-line lockfiles after comparison admission.
/// Stay below 2^23 so f32 coordinates retain at least half-pixel precision; byte/row limits still
/// bound metadata independently. Excessive wrapping/zoom receives the existing explicit fallback.
const MAX_LAYOUT_HEIGHT: f32 = 8_000_000.;
const MAX_SOURCE_BYTES: usize = 1024 * 1024;
