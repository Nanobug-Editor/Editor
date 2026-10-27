//! Foreground controls reuse ordinary node rendering and retain the document's native input states.
use super::*;
use gpui_kit::{AnyElement, InteractiveElement, ParentElement, Styled, div, px};

/// One ancestor's semantic lifetime; geometry and drawing revision intentionally do not retire inputs.
#[derive(Clone, PartialEq)]
struct OverlayKey {
    /// Public Canvas node owning this foreground layer.
    canvas: String,
    /// Stable foreground identity within that Canvas.
    overlay: String,
    /// Scene lifetime to which the control was attached.
    identity: u64,
    /// Interactive target lifetime, independent of drawing updates.
    target: ui::CanvasTargetRef,
}

/// Shared ownership chain also includes outer overlays when a native subtree contains another Canvas.
pub(super) struct OverlayScope {
    /// Outer-to-inner semantic owners; replacement of any ancestor withdraws native input.
    lineage: Vec<OverlayKey>,
    /// Unique view-local incarnation captured by ordinary native action handlers.
    pub(super) generation: u64,
    /// One native containment handle for the foreground subtree.
    focus: FocusHandle,
    /// Last accepted guest policy, still subject to successful display validation.
    focus_return: Option<ui::CanvasTargetRef>,
}

/// A withdrawn, actually focused native subtree may ask its Canvas to validate one return target.
pub(super) struct OverlayFocusReturn {
    /// Existing Canvas node; no new surface is created by this request.
    pub(super) canvas: String,
    /// Original scene lifetime; a replacement scene cannot inherit focus.
    pub(super) identity: u64,
    /// Declared native keyboard destination within that scene.
    pub(super) target: ui::CanvasTargetRef,
}

/// Stable native input identities are conditional on both source authority and complete target lineage.
#[derive(Default)]
pub(super) struct OverlayControls {
    /// Native descendants share an immutable ownership chain and incarnation.
    pub(super) scopes: BTreeMap<String, Rc<OverlayScope>>,
    /// Wake rendering must rebuild fields before ordinary leaf rendering can address them.
    pub(super) needs_sync: bool,
    /// App-only withdrawal defers only the focus check; editing subscriptions are already gone.
    pending_focus: Vec<FocusHandle>,
    /// Monotonic view-local lifetime allocator; never reset by hiding or redraws.
    sequence: u64,
    /// Exact editable source authority against which retained controls were last reconciled.
    source: Option<plugin_runtime::plugin_protocol::api::DocumentVersion>,
    /// Exact binary-file authority, independent of editable source.
    file: Option<plugin_runtime::plugin_protocol::api::FileVersion>,
}

impl OverlayControls {
    /// Reconcile active overlay descendants and return IDs whose old native ownership was withdrawn.
    fn reconcile(
        &mut self,
        document: &Document,
        cx: &mut App,
    ) -> (Vec<(String, Rc<OverlayScope>)>, bool) {
        let same_source = self.source == document.source && self.file == document.file;
        let mut next: BTreeMap<String, Rc<OverlayScope>> = BTreeMap::new();
        let mut visit = |node: &ui::Node| {
            let Kind::Canvas(canvas) = &node.kind else {
                return;
            };
            if document.active_node(&node.id).is_none() {
                return;
            }
            for overlay in &canvas.overlays {
                let mut lineage = next
                    .get(&node.id)
                    .map_or_else(Vec::new, |scope| scope.lineage.clone());
                lineage.push(OverlayKey {
                    canvas: node.id.clone(),
                    overlay: overlay.id.clone(),
                    identity: overlay.anchor.identity,
                    target: overlay.anchor.target.clone(),
                });
                let previous = self
                    .scopes
                    .get(&overlay.child.id)
                    .filter(|scope| same_source && scope.lineage == lineage);
                let (generation, focus) = if let Some(previous) = previous {
                    (previous.generation, previous.focus.clone())
                } else {
                    self.sequence = self
                        .sequence
                        .checked_add(1)
                        .expect("native overlay lifetime exhausted");
                    (self.sequence, cx.focus_handle())
                };
                let scope = Rc::new(OverlayScope {
                    lineage,
                    generation,
                    focus,
                    focus_return: overlay.focus_return.clone(),
                });
                overlay.child.visit(&mut |child| {
                    next.insert(child.id.clone(), scope.clone());
                });
            }
        };
        document.root.visit(&mut visit);
        if let Some(dialog) = &document.dialog {
            dialog.content.visit(&mut visit);
        }
        let retired = self
            .scopes
            .iter()
            .filter(|(id, scope)| {
                next.get(*id)
                    .is_none_or(|current| current.generation != scope.generation)
            })
            .map(|(id, scope)| (id.clone(), scope.clone()))
            .collect();
        self.scopes = next;
        self.source = document.source.clone();
        self.file = document.file.clone();
        (retired, same_source)
    }
}

impl PluginView {
    /// Revoke foreground producers synchronously, including a false/true handover without a paint.
    pub(super) fn suspend_overlay_controls(&mut self, cx: &mut Context<Self>) {
        let scopes = std::mem::take(&mut self.overlays.scopes);
        if scopes.is_empty() {
            return;
        }
        self.overlays.needs_sync = true;
        let mut generations = BTreeSet::new();
        let mut canvases = Vec::new();
        for (id, scope) in scopes {
            if generations.insert(scope.generation) {
                self.overlays.pending_focus.push(scope.focus.clone());
            }
            self.forms.cancel(&id);
            if let Some(input) = self.inputs.remove(&id) {
                input
                    .state
                    .update(cx, |state, cx| state.set_disabled(true, cx));
            }
            if let Some(input) = self.textareas.remove(&id) {
                input
                    .state
                    .update(cx, |state, cx| state.set_disabled(true, cx));
            }
            if let Some(canvas) = self.canvases.remove(&id) {
                // Stop producer effects immediately; the deferred Window cleanup still needs
                // the original frame to report cancellation under its original authority.
                canvas.update(cx, |canvas, cx| {
                    canvas.enabled = false;
                    cx.notify();
                });
                canvases.push(canvas);
            }
            self.collections.remove(&id);
            if let Some(tree) = self.trees.remove(&id) {
                tree.read(cx).resource.suspend();
            }
            if let Some(snapshot) = self.snapshots.remove(&id) {
                snapshot.dispose(cx);
            }
            self.checkbox_focus.remove(&id);
        }
        if !canvases.is_empty() {
            let target = self.window;
            cx.defer(move |cx| {
                let _ = target.update(cx, |_, window, cx| {
                    for canvas in canvases {
                        canvas.update(cx, |canvas, cx| {
                            canvas.reconcile_target_lifetimes(
                                None,
                                false,
                                ui::CanvasCancelReason::Retired,
                                window,
                                cx,
                            )
                        });
                    }
                });
            });
        }
    }

    /// Detach old editing subscriptions before a reused child ID can create a new native control.
    pub(super) fn reconcile_overlay_controls(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<OverlayFocusReturn> {
        self.overlays.needs_sync = false;
        let pending = std::mem::take(&mut self.overlays.pending_focus);
        if pending
            .iter()
            .any(|focus| focus.contains_focused(window, cx))
        {
            window.blur(cx);
        }
        let (retired, same_source) = self.overlays.reconcile(&self.document, cx);
        let mut focused = BTreeMap::new();
        for (_, scope) in &retired {
            if scope.focus.contains_focused(window, cx) {
                focused.insert(scope.generation, scope.clone());
            }
        }
        let mut returns = Vec::new();
        let blur = !focused.is_empty();
        for scope in focused.into_values() {
            if same_source && let Some(target) = &scope.focus_return {
                let owner = scope
                    .lineage
                    .last()
                    .expect("overlay scope has an owning Canvas");
                let prefix = &scope.lineage[..scope.lineage.len() - 1];
                let current_parent = self
                    .overlays
                    .scopes
                    .get(&owner.canvas)
                    .map_or(&[][..], |parent| parent.lineage.as_slice());
                // A replaced outer overlay cannot transfer inner focus to its newly created Canvas.
                if prefix == current_parent {
                    returns.push(OverlayFocusReturn {
                        canvas: owner.canvas.clone(),
                        identity: owner.identity,
                        target: target.clone(),
                    });
                }
            }
        }
        for (id, _) in retired {
            self.forms.cancel(&id);
            if let Some(input) = self.inputs.remove(&id) {
                input
                    .state
                    .update(cx, |state, cx| state.set_disabled(true, cx));
            }
            if let Some(input) = self.textareas.remove(&id) {
                input
                    .state
                    .update(cx, |state, cx| state.set_disabled(true, cx));
            }
            if let Some(canvas) = self.canvases.remove(&id) {
                canvas.update(cx, |canvas, cx| {
                    canvas.reconcile_target_lifetimes(
                        None,
                        false,
                        ui::CanvasCancelReason::TargetRemoved,
                        window,
                        cx,
                    )
                });
            }
            self.collections.remove(&id);
            if let Some(tree) = self.trees.remove(&id) {
                tree.read(cx).resource.suspend();
            }
            if let Some(snapshot) = self.snapshots.remove(&id) {
                snapshot.dispose(cx);
            }
            self.checkbox_focus.remove(&id);
        }
        // End platform composition only after detached old subscriptions can no longer
        // reinterpret a blur-time commit as input for a replacement control.
        if blur {
            window.blur(cx);
        }
        returns
    }

    /// Render `overlay` only under its current Canvas/target ownership, at projected logical `rect`.
    pub(super) fn render_canvas_overlay(
        &mut self,
        canvas: &str,
        overlay: &ui::CanvasOverlay,
        layout: canvas::OverlayLayout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let canvas::OverlayLayout { rect, clip, scale } = layout;
        let current=self.scene_current.get() && self.document.active_node(canvas).is_some_and(|node| {
            matches!(&node.kind,Kind::Canvas(canvas) if canvas.overlays.iter().any(|item|item==overlay))
        });
        if !current || !scale.is_finite() || scale <= 0. {
            return div().into_any_element();
        }
        let Some(scope) = self.overlays.scopes.get(&overlay.child.id) else {
            return div().into_any_element();
        };
        let focus = scope.focus.clone();
        let mut child = (*overlay.child).clone();
        child.layout.width = Some(rect.w / scale);
        child.layout.height = Some(rect.h / scale);
        let previous = self.native_scale;
        self.native_scale = scale;
        let content = self.node(&child, false, window, cx);
        self.native_scale = previous;
        let mut element = div()
            .id(format!("canvas-overlay:{}:{}", canvas, overlay.id))
            .track_focus(&focus)
            .absolute()
            .left(px(rect.x - clip.x))
            .top(px(rect.y - clip.y))
            .w(px(rect.w))
            .h(px(rect.h))
            .flex()
            .flex_col()
            .overflow_hidden()
            .occlude()
            // The native subtree owns pointer input inside its rectangular foreground box.
            .on_any_mouse_down(|_, _, cx| cx.stop_propagation());
        for button in [
            gpui_kit::MouseButton::Left,
            gpui_kit::MouseButton::Middle,
            gpui_kit::MouseButton::Right,
            gpui_kit::MouseButton::Navigate(gpui_kit::NavigationDirection::Back),
            gpui_kit::MouseButton::Navigate(gpui_kit::NavigationDirection::Forward),
        ] {
            element = element.on_mouse_up(button, |_, _, cx| cx.stop_propagation());
        }
        div()
            .absolute()
            .left(px(clip.x))
            .top(px(clip.y))
            .w(px(clip.w))
            .h(px(clip.h))
            .overflow_hidden()
            .child(element.child(content))
            .into_any_element()
    }
}
