//! First-level group rotation shares immutable child weights between preview and the final Base edit.
use super::*;

impl Snapshot {
    /// Rotate only `group`'s first split to `stacking`; selected nested cells retain their own axes.
    /// `members` selects visible moving panels, and `cx` resolves their existing Base view handles.
    pub(super) fn group_layout(
        &self,
        group: &PaneNode,
        members: &[PanelId],
        stacking: Axis,
        cx: &App,
    ) -> Option<DockLayout> {
        if matches!(group.kind(), PaneRef::Tabs { .. }) {
            return self.layout(group, None, Some(members), cx);
        }
        let mut layout = if stacking == Axis::Horizontal {
            DockLayout::h_split()
        } else {
            DockLayout::v_split()
        };
        for (child, weight) in self.group_slots(group, members, cx) {
            layout = layout.child(self.layout(child, None, Some(members), cx)?, Some(weight));
        }
        Some(layout)
    }

    /// Project selected leaves into `destination`; only the outer split receives the optional new axis.
    /// Nested rectangles follow original child ratios, matching `group_layout` without flattening them.
    pub(in super::super) fn group_rectangles(
        &self,
        group: &PaneNode,
        members: &[PanelId],
        destination: Bounds<Pixels>,
        stacking: Option<Axis>,
        cx: &App,
    ) -> Vec<(PanelId, Bounds<Pixels>)> {
        let PaneRef::Split { axis, .. } = group.kind() else {
            let PaneRef::Tabs { panels, .. } = group.kind() else {
                return Vec::new();
            };
            return panels
                .iter()
                .filter(|id| members.contains(id))
                .map(|id| (*id, destination))
                .collect();
        };
        let slots = self.group_slots(group, members, cx);
        let total = slots
            .iter()
            .fold(px(0.), |total, (_, weight)| total + *weight);
        let mut offset = px(0.);
        let mut rectangles = Vec::new();
        for (child, weight) in slots {
            let mut bounds = destination;
            if stacking.unwrap_or(axis) == Axis::Horizontal {
                bounds.origin.x += offset;
                bounds.size.width *= weight / total;
                offset += bounds.size.width;
            } else {
                bounds.origin.y += offset;
                bounds.size.height *= weight / total;
                offset += bounds.size.height;
            }
            rectangles.extend(self.group_rectangles(child, members, bounds, None, cx));
        }
        rectangles
    }

    /// Keep first-level cells intact and weight them along their original axis before any rotation.
    /// Missing paint data falls back to Base's preference or one pixel, never dropping registered panels.
    fn group_slots<'a>(
        &self,
        group: &'a PaneNode,
        members: &[PanelId],
        cx: &App,
    ) -> Vec<(&'a PaneNode, Pixels)> {
        let PaneRef::Split {
            axis,
            children,
            sizes,
        } = group.kind()
        else {
            return Vec::new();
        };
        children
            .iter()
            .enumerate()
            .filter_map(|(index, child)| {
                let mut selected = false;
                child.walk(&mut |node| {
                    if let PaneRef::Tabs { panels, .. } = node.kind() {
                        selected |= panels.iter().any(|id| members.contains(id));
                    }
                });
                selected.then(|| {
                    let weight = self
                        .bounds(child, cx)
                        .map(|bounds| {
                            if axis == Axis::Horizontal {
                                bounds.size.width
                            } else {
                                bounds.size.height
                            }
                        })
                        .or_else(|| sizes.get(index).copied().flatten())
                        .unwrap_or(px(1.))
                        .max(px(1.));
                    (child, weight)
                })
            })
            .collect()
    }
}
