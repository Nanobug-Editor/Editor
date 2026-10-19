//! Convert the historical center sidebar into Base's fixed-width peripheral dock at restoration.
use super::*;
use gpui_base::dock::DockPlacement;
use gpui_kit::Axis;

#[cfg(test)]
mod tests;

/// Move left-of-editor subtrees from `layout.center`, preserving their order and pixel preferences.
/// `fallback` is the legacy Explorer width when a hidden or unmeasured band has no usable size.
/// Returns whether the persisted structure changed; right/bottom docks and bound views are untouched.
pub(super) fn separate_left(layout: &mut DockAreaState, fallback: Pixels) -> bool {
    let mut changed = false;
    // Nested horizontal ancestors can contain several old left columns. Promote each
    // outer-to-inner band so no residual sidebar still scales with the center.
    while let Some((sidebar, width)) = take_left(&mut layout.center, fallback) {
        let (panel, width) = match layout.left_dock.take() {
            Some(previous) if has_panels(previous.panel()) => (
                stack(
                    vec![previous.panel().clone(), sidebar],
                    vec![previous.size(), width],
                ),
                if previous.open() {
                    previous.size() + width
                } else {
                    width
                },
            ),
            _ => (sidebar, width),
        };
        layout.left_dock = Some(DockState::new(panel, DockPlacement::Left, width, true));
        changed = true;
    }
    changed
}

/// A closed old outer column becomes hidden individual panels when joining the open fixed sidebar.
/// This is read before native view construction, so no hidden resource or focus is briefly mounted.
pub(super) fn closed_left_panels(layout: &DockAreaState) -> Vec<String> {
    let Some(previous) = layout.left_dock.as_ref().filter(|dock| !dock.open()) else {
        return Vec::new();
    };
    if take_left(&mut layout.center.clone(), px(1.)).is_none() {
        return Vec::new();
    }
    let mut names = Vec::new();
    collect_names(previous.panel(), &mut names);
    names
}

/// Collect stable leaf identities only; containers have no independent visibility intent.
fn collect_names(state: &PanelState, names: &mut Vec<String>) {
    if matches!(&state.info, PanelInfo::Panel(_)) && !state.panel_name.is_empty() {
        names.push(state.panel_name.clone());
    }
    for child in &state.children {
        collect_names(child, names);
    }
}

/// An emptied legacy dock keeps metadata but must not reserve a second, invisible column.
fn has_panels(state: &PanelState) -> bool {
    matches!(&state.info, PanelInfo::Panel(_)) && !state.panel_name.is_empty()
        || state.children.iter().any(has_panels)
}

/// Only the horizontal sidebar ancestry is promoted; editor-local top/bottom arrangements stay intact.
fn take_left(state: &mut PanelState, fallback: Pixels) -> Option<(PanelState, Pixels)> {
    if state.info.axis() != Some(Axis::Horizontal) {
        return None;
    }
    let index = state
        .children
        .iter()
        .position(|child| contains_panel(child, "Editor"))?;
    if index == 0 {
        return take_left(&mut state.children[0], fallback);
    }
    let PanelInfo::Stack { sizes, .. } = &mut state.info else {
        return None;
    };
    let widths: Vec<_> = (0..index)
        .map(|ix| {
            sizes
                .get(ix)
                .copied()
                .filter(|size| size.as_f32().is_finite() && *size > px(0.))
                .unwrap_or(fallback)
        })
        .collect();
    let width = widths
        .iter()
        .copied()
        .fold(px(0.), |sum, width| sum + width);
    let children = state.children.drain(..index).collect::<Vec<_>>();
    sizes.drain(..index.min(sizes.len()));
    let sidebar = if children.len() == 1 {
        children.into_iter().next().unwrap()
    } else {
        stack(children, widths)
    };
    if state.children.len() == 1 {
        *state = state.children.remove(0);
    }
    Some((sidebar, width))
}

/// Compose existing columns using the public persisted schema; Base normalizes the result on load.
fn stack(children: Vec<PanelState>, sizes: Vec<Pixels>) -> PanelState {
    PanelState {
        panel_name: "StackPanel".into(),
        children,
        info: PanelInfo::stack(sizes, Axis::Horizontal),
    }
}
