//! Native semantic elements reuse the real target focus and the worker's clipped scene geometry.
use super::*;
use gpui_kit::{AccessibleAction as NativeAction, AnyElement, Role, accesskit};
use protocol::ui::{
    CanvasInputKind, CanvasSemanticAction as Action, CanvasSemanticNode,
    CanvasSemanticRole as SemanticRole,
};

impl CanvasView {
    /// Build only the declared visible forest; stable target handles supply actual native focus.
    pub(super) fn semantic_elements(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(scene) = &self.drawing.scene else {
            return Vec::new();
        };
        if !self.enabled {
            return Vec::new();
        }
        scene
            .semantics
            .iter()
            .filter(|node| node.parent.is_none())
            .filter_map(|node| self.semantic_element(node, point(px(0.), px(0.)), cx))
            .collect()
    }
    /// Place one clipped native semantic object relative to its semantic parent's projected origin.
    fn semantic_element(
        &self,
        node: &CanvasSemanticNode,
        parent: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let scene = self.drawing.scene.as_ref()?;
        let rect = self
            .scene_graphics
            .as_ref()?
            .targets
            .semantic_bounds_in(&node.id, self.target_content_viewport())?;
        let mut rect = self.project(rect, self.bounds);
        // Native accessibility boxes and keyboard targets must not remain outside the actual viewport.
        let right = (rect.x + rect.w).min(self.bounds.size.width / px(1.));
        let bottom = (rect.y + rect.h).min(self.bounds.size.height / px(1.));
        rect.x = rect.x.max(0.);
        rect.y = rect.y.max(0.);
        rect.w = right - rect.x;
        rect.h = bottom - rect.y;
        if rect.w <= 0. || rect.h <= 0. {
            return None;
        }
        let origin = point(px(rect.x), px(rect.y));
        let target = node
            .target
            .as_ref()
            .and_then(|id| scene.targets.iter().find(|target| target.id == *id));
        let epoch = target.map_or(0, |target| target.epoch);
        let mut element = div()
            .id(format!(
                "canvas-semantic:{}:{}:{}:{}",
                scene.identity,
                node.id.len(),
                node.id,
                epoch
            ))
            .accessibility_id(format!("canvas:{}:{}", scene.identity, node.id))
            .role(role(node.role))
            .aria_label(node.name.for_locale(&self.locale).to_owned())
            .absolute()
            .left(origin.x - parent.x)
            .top(origin.y - parent.y)
            .w(px(rect.w))
            .h(px(rect.h));
        let mut description = node
            .description
            .as_ref()
            .map(|text| text.for_locale(&self.locale).to_owned())
            .unwrap_or_default();
        if let (Some(rows), Some(columns)) = (node.row_count, node.column_count) {
            // The locked Windows AccessKit adapter has no GridPattern. Expose totals in native
            // localized descriptions as well as typed AccessKit properties so readers can announce them.
            let locale = if self.locale.is_empty() {
                "zh-CN"
            } else {
                &self.locale
            };
            let totals = rust_i18n::t!(
                "canvas.semantic_grid_size",
                locale = locale,
                rows = rows,
                columns = columns
            );
            if !description.is_empty() {
                description.push_str("; ");
            }
            description.push_str(&totals);
        }
        if !description.is_empty() {
            element = element.aria_description(description);
        }
        if let Some(target) = target {
            if let Some(handle) = self.target_focus(&(target.id.clone(), target.epoch)) {
                element = element.track_focus(&handle).key_context("CanvasTarget");
            }
            if !node.disabled {
                for action in &node.actions {
                    let target = (target.id.clone(), target.epoch);
                    let semantic = node.id.clone();
                    let action = *action;
                    let identity = scene.identity;
                    let gesture = self.action_gesture;
                    let owner = cx.entity().downgrade();
                    element =
                        element.on_a11y_action(native_action(action), move |data, window, cx| {
                            let value = if action == Action::SetValue {
                                match data {
                                    Some(accesskit::ActionData::Value(value))
                                        if value.len() <= 65536 =>
                                    {
                                        Some(value.to_string())
                                    }
                                    _ => return,
                                }
                            } else {
                                None
                            };
                            let _ = owner.update(cx, |view, cx| {
                                if view.action_gesture != gesture || view.drawing.scene.as_ref().is_none_or(|scene| scene.identity != identity) {
                                    return;
                                }
                                // Validate the live declaration and original shown-frame target before
                                // changing native focus; a queued old reader action has no side effects.
                                if !view.emit_target(target.clone(), CanvasInputKind::A11y { semantic: semantic.clone(), action, value }, cx) {
                                    return;
                                }
                                if action == Action::Focus {
                                    if let Some(handle) = view.target_focus(&target) {
                                        handle.focus(window, cx);
                                        if let Err(error) = editor_ui::window::focus_accessible_surface(window) {
                                            tracing::warn!(%error, "could not synchronize native accessibility focus");
                                        }
                                    }
                                }
                                cx.notify();
                            });
                        });
                }
            }
        }
        let properties = node.clone();
        element = element.a11y_synthetic_children(move |builder| {
            let native = builder.parent_node();
            if let Some(value) = properties.value {
                native.set_value(value);
            }
            if properties.disabled {
                native.set_disabled();
            }
            if properties.read_only {
                native.set_read_only();
            }
            if let Some(value) = properties.selected {
                native.set_selected(value);
            }
            if let Some(value) = properties.expanded {
                native.set_expanded(value);
            }
            if let Some(value) = properties.checked {
                native.set_toggled(if value {
                    accesskit::Toggled::True
                } else {
                    accesskit::Toggled::False
                });
            }
            if let Some(value) = properties.row {
                native.set_row_index((value - 1) as usize);
                native.set_row_index_text(value.to_string());
            }
            if let Some(value) = properties.column {
                native.set_column_index((value - 1) as usize);
                native.set_column_index_text(value.to_string());
            }
            if let Some(value) = properties.row_count {
                native.set_row_count(value as usize);
            }
            if let Some(value) = properties.column_count {
                native.set_column_count(value as usize);
            }
        });
        for child in scene
            .semantics
            .iter()
            .filter(|child| child.parent.as_deref() == Some(&node.id))
        {
            if let Some(child) = self.semantic_element(child, origin, cx) {
                element = element.child(child);
            }
        }
        Some(element.into_any_element())
    }
}
/// Portable roles map once onto the locked native accessibility vocabulary.
fn role(role: SemanticRole) -> Role {
    match role {
        SemanticRole::Group => Role::Group,
        SemanticRole::Label => Role::Label,
        SemanticRole::Button => Role::Button,
        SemanticRole::TextInput => Role::TextInput,
        SemanticRole::Grid => Role::Grid,
        SemanticRole::Row => Role::Row,
        SemanticRole::GridCell => Role::GridCell,
        SemanticRole::ColumnHeader => Role::ColumnHeader,
        SemanticRole::RowHeader => Role::RowHeader,
        SemanticRole::Image => Role::Image,
        SemanticRole::List => Role::List,
        SemanticRole::ListItem => Role::ListItem,
        SemanticRole::Slider => Role::Slider,
        SemanticRole::CheckBox => Role::CheckBox,
        SemanticRole::TreeItem => Role::TreeItem,
    }
}
/// Native actions stay separate from ordinary GPUI keymap actions and guest business commands.
fn native_action(action: Action) -> NativeAction {
    match action {
        Action::Focus => NativeAction::Focus,
        Action::Activate => NativeAction::Click,
        Action::SetValue => NativeAction::SetValue,
        Action::ScrollIntoView => NativeAction::ScrollIntoView,
        Action::Increment => NativeAction::Increment,
        Action::Decrement => NativeAction::Decrement,
    }
}
