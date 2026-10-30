//! Shared, text-free unchanged-region choices and native disclosure buttons for aligned panes.
use super::*;
use crate::{Button, Icon, theme::ActiveTheme as _};
use gpui_kit::{InteractiveElement as _, ParentElement as _, Styled as _, div};
use rust_i18n::t;
use std::{cell::RefCell, sync::Arc};

/// A retained view owns this small presentation model; source text and Undo stay in EditorState.
#[derive(Clone)]
pub struct UnchangedFolds {
    /// Provider-selected context, checked against the public 1..=20 line bound.
    context: usize,
    /// Publishing instance's validated light/dark SVG bytes; no host asset path substitution.
    expand_icons: [Arc<[u8]>; 2],
    /// Clones share per-region choices across tab activation and group replacement.
    state: Rc<RefCell<Choices>>,
}

/// Plans contain aligned slot intervals only; content/revision rebuilds replace obsolete intervals.
#[derive(Default)]
struct Choices {
    /// The last all-regions action also governs newly discovered common regions after edits.
    collapse_new: bool,
    /// Each entry contains one common interval and whether its disclosure is currently collapsed.
    regions: Vec<(Range<usize>, bool)>,
}

impl UnchangedFolds {
    /// Choose the initial all-regions policy before mounting; this does not change native source text.
    pub fn initially_collapsed(self, collapsed: bool) -> Self {
        self.set_all(collapsed);
        self
    }

    /// Retain checked package artwork and an initially expanded comparison, without copying text.
    pub fn new(context: u8, expand_icons: [Arc<[u8]>; 2]) -> Self {
        Self {
            context: usize::from(context),
            expand_icons,
            state: Default::default(),
        }
    }

    /// Reuse manual choices only while the complete common-slot plan remains the same.
    pub(super) fn plan(&self, rows: &AlignedRows) {
        let regions = rows.unchanged_regions(self.context);
        let mut state = self.state.borrow_mut();
        if !state
            .regions
            .iter()
            .map(|(range, _)| range)
            .eq(regions.iter())
        {
            let collapsed = state.collapse_new;
            state.regions = regions
                .into_iter()
                .map(|range| (range, collapsed))
                .collect();
        }
    }

    /// Whether collapse/expand can change the current plan, respectively.
    pub fn available(&self) -> (bool, bool) {
        let state = self.state.borrow();
        (
            state.regions.iter().any(|(_, collapsed)| !collapsed),
            state.regions.iter().any(|(_, collapsed)| *collapsed),
        )
    }

    /// Apply one toolbar choice to existing and subsequently discovered unchanged regions.
    pub(super) fn set_all(&self, collapsed: bool) {
        let mut state = self.state.borrow_mut();
        state.collapse_new = collapsed;
        for (_, value) in &mut state.regions {
            *value = collapsed;
        }
    }

    /// Expand exactly the painted slot interval; stale disclosure callbacks have no effect.
    pub(super) fn expand(&self, expected: &Range<usize>) -> bool {
        let mut state = self.state.borrow_mut();
        let Some((_, collapsed)) = state
            .regions
            .iter_mut()
            .find(|(range, _)| range == expected)
        else {
            return false;
        };
        std::mem::replace(collapsed, false)
    }

    /// Return the currently hidden intervals for a shared two/three-pane projection.
    pub(super) fn collapsed(&self) -> Vec<Range<usize>> {
        self.state
            .borrow()
            .regions
            .iter()
            .filter(|(_, collapsed)| *collapsed)
            .map(|(range, _)| range.clone())
            .collect()
    }

    /// Reveal a source byte requested by navigation or a real native cursor move, in every peer.
    pub(super) fn reveal(&self, rows: &AlignedRows, pane: usize, byte: usize) -> bool {
        let Some(ranges) = rows.source_rows.get(pane) else {
            return false;
        };
        let row = ranges
            .partition_point(|range| range.end <= byte && !range.is_empty())
            .min(ranges.len().saturating_sub(1));
        let Some(slot) = rows.slots[pane]
            .iter()
            .position(|value| *value == Some(row))
        else {
            return false;
        };
        let target = self
            .collapsed()
            .into_iter()
            .find(|range| range.contains(&slot));
        target.is_some_and(|range| self.expand(&range))
    }
}

impl GroupState {
    /// Invalidate only projected geometry after a fold choice; no diff work or source state changes.
    pub(super) fn folds_changed(&mut self, cx: &mut gpui_kit::Context<Self>) {
        for region in &self.regions {
            region.request_cancel();
        }
        self.epoch = self.epoch.wrapping_add(1);
        self.measurements.fill(None);
        self.frames.fill(None);
        self.layout = None;
        self.status = AlignmentStatus::Measuring;
        cx.notify();
    }

    /// Change all disclosures together; hidden/retired groups cannot execute stale controls.
    pub(super) fn set_folds(&mut self, collapsed: bool, cx: &mut gpui_kit::Context<Self>) {
        if self.retired {
            return;
        }
        if let Some(folding) = &self.folding {
            folding.set_all(collapsed);
            self.folds_changed(cx);
        }
    }
}

/// Overlay controls are siblings above the native pointer surface, just like the scroll rails.
/// Their hitboxes prevent the root router from translating a disclosure click into source input.
pub(super) fn disclosures(
    group: &Entity<GroupState>,
    index: usize,
    cx: &mut App,
) -> Vec<AnyElement> {
    let state = group.read(cx);
    if !matches!(state.effective_status(), AlignmentStatus::Ready) {
        return Vec::new();
    }
    let (Some(folding), Some(layout), Some(view)) =
        (&state.folding, &state.layout, state.display_viewport)
    else {
        return Vec::new();
    };
    let Some(measured) = state.measurements[index].as_ref() else {
        return Vec::new();
    };
    let inset = view.pane(measured).inset_y;
    let scroll = state.scroll.get();
    let right_inset = state.source_right_inset(index);
    let epoch = state.epoch;
    let icon = folding.expand_icons[usize::from(cx.theme().is_dark())].clone();
    layout
        .folds
        .iter()
        .filter_map(|(range, y)| {
            let top = px(y.start) - scroll;
            let height = px(y.end - y.start);
            if top < px(0.) || top + height > view.height {
                return None;
            }
            let expected = range.clone();
            let owner = group.clone();
            let selector = format!("diff-unchanged-{index}-{}", range.start);
            let label = t!("editor.diff_expand_region", count = range.len()).to_string();
            Some(
                div()
                    .absolute()
                    // Protect native text clicks while retaining the shared viewport's wheel
                    // hitbox; full occlusion would trap scrolling whenever a bar passes the mouse.
                    .block_mouse_except_scroll()
                    .top(inset + top)
                    .left_0()
                    .right(right_inset)
                    .h(height)
                    .bg(cx.theme().muted)
                    .child(
                        Button::new(("diff-unchanged", range.start))
                            .debug_selector(move || selector.clone())
                            .icon(Icon::default().data(&icon))
                            .label(label)
                            .small()
                            .ghost()
                            .w_full()
                            .h(height)
                            .on_click(move |_, _, cx| {
                                owner.update(cx, |state, cx| {
                                    if state.retired || state.epoch != epoch {
                                        return;
                                    }
                                    if state
                                        .folding
                                        .as_ref()
                                        .is_some_and(|folding| folding.expand(&expected))
                                    {
                                        state.folds_changed(cx);
                                    }
                                });
                            }),
                    )
                    .into_any_element(),
            )
        })
        .collect()
}
