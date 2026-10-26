//! Target focus uses stable native handles; source/presentation authority remains in the Canvas owner.
use super::*;
use protocol::ui::{CanvasInput, CanvasInputKind, CanvasKeyPhase, CanvasModifiers, SceneGeometry};
pub(super) mod frame;
pub(super) mod geometry;
mod lifetime;
#[cfg(all(test, target_os = "windows"))]
pub(super) mod native_text_tests;
mod pointer;
mod raster;
pub(super) mod text;

/// Per-application keymap installation; legacy canvases retain the ordinary Root bindings.
struct TargetKeymap {
    /// One App-native sequence survives widget recreation and orders all Canvas input without wrapping.
    sequence: Rc<Cell<u64>>,
}
impl gpui_kit::Global for TargetKeymap {}
/// Kit dispatches bindings before raw keys. Canvas targets own navigation and domain shortcuts.
pub(super) fn init(cx: &mut App) {
    if !cx.has_global::<TargetKeymap>() {
        cx.bind_keys(
            [
                "tab",
                "shift-tab",
                "backspace",
                "delete",
                "enter",
                "left",
                "right",
                "up",
                "down",
                "home",
                "end",
                "shift-left",
                "shift-right",
                "shift-up",
                "shift-down",
                "shift-home",
                "shift-end",
                "ctrl-a",
                "ctrl-z",
                "ctrl-shift-z",
                "ctrl-y",
            ]
            .map(|key| gpui_kit::KeyBinding::new(key, gpui_kit::NoAction, Some("CanvasTarget"))),
        );
        cx.set_global(TargetKeymap {
            sequence: Rc::new(Cell::new(0)),
        });
    }
}

/// Native focus handles outlive ordinary scene repaints but never a target epoch replacement.
#[derive(Default)]
pub(super) struct Targets {
    /// Target listeners survive paint-only press-ticket changes but never a cleared native owner.
    lifetime: u64,
    /// Private complex-layer drafts are coalesced separately from ordered native text observations.
    raster: raster::State,
    /// Only focused or still-unacknowledged plugin-domain text needs a temporary native draft.
    sessions: BTreeMap<(String, u64), text::TextInput>,
    /// Deduplicate the same recoverable native session failure across ordinary repaints.
    text_errors: BTreeMap<(String, u64), (protocol::api::ErrorCode, String)>,
    /// Accepted redraws cannot reinterpret input that was created against the previous shown frame.
    frame: Option<frame::InputFrame>,
    /// Hover and capture follow native pointer identity, independently of keyboard focus.
    hover: Option<(String, u64)>,
    /// Last sample and observed down/up button set; this never fabricates a full native button mask.
    captured: Option<((String, u64), protocol::ui::CanvasPointerInput, u8)>,
    /// Native target focus callbacks retire with their exact handles.
    subscriptions: BTreeMap<(String, u64), Vec<Subscription>>,
    /// Drawing IDs and array positions do not participate in native focus identity.
    focus: BTreeMap<(String, u64), FocusHandle>,
    /// Public focus observations follow window activation as well as logical GPUI focus.
    focused: std::collections::BTreeSet<(String, u64)>,
    /// Shared native event order; never wraps and never resets for ordinary drawing updates.
    sequence: Rc<Cell<u64>>,
}
impl Targets {
    /// A rendered target listener keeps this token until the complete native owner is withdrawn.
    pub(super) fn lifetime(&self) -> u64 {
        self.lifetime
    }
    /// Rebind a same-owner visibility ticket without inventing a new displayed revision.
    pub(super) fn rebind_gesture(&mut self, gesture: Option<action_gestures::ActionGesture>) {
        if let Some(frame) = &mut self.frame {
            frame.gesture = gesture;
        }
    }
    /// New widgets reuse the App's native order so recreation cannot replay an earlier Canvas sequence.
    pub(super) fn new(cx: &App) -> Self {
        Self {
            sequence: cx.global::<TargetKeymap>().sequence.clone(),
            ..Self::default()
        }
    }
    /// Drop native ownership on a Canvas/source retirement without retaining committed plugin text.
    pub(super) fn clear(&mut self) {
        self.lifetime = self
            .lifetime
            .checked_add(1)
            .expect("Canvas target lifetime exhausted");
        self.raster.retire();
        self.focus.clear();
        self.focused.clear();
        self.subscriptions.clear();
        self.hover = None;
        self.captured = None;
        self.frame = None;
        self.sessions.clear();
        self.text_errors.clear();
    }
}
impl CanvasView {
    /// Share the stable native target handle with the semantic adapter without exposing target maps.
    pub(super) fn target_focus(&self, key: &(String, u64)) -> Option<FocusHandle> {
        self.targets.focus.get(key).cloned()
    }
    /// Reconcile stable focus handles before native element construction.
    pub(super) fn sync_targets(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let targets = self
            .drawing
            .scene
            .as_ref()
            .map_or(&[][..], |scene| scene.targets.as_slice());
        self.targets.focus.retain(|(id, epoch), _| {
            targets
                .iter()
                .any(|target| target.focusable && target.id == *id && target.epoch == *epoch)
        });
        self.targets
            .subscriptions
            .retain(|key, _| self.targets.focus.contains_key(key));
        if self.enabled {
            for target in targets.iter().filter(|target| target.focusable) {
                let key = (target.id.clone(), target.epoch);
                if !self.targets.focus.contains_key(&key) {
                    let handle = cx.focus_handle();
                    let focused = key.clone();
                    let blurred = key.clone();
                    let subscriptions = vec![
                        cx.on_focus(&handle, window, move |this, window, cx| {
                            if window.is_window_active() {
                                this.emit_target(
                                    focused.clone(),
                                    CanvasInputKind::Focus { focused: true },
                                    cx,
                                );
                            }
                        }),
                        cx.on_blur(&handle, window, move |this, _, cx| {
                            this.cancel_target_text(&blurred, cx);
                            if this
                                .targets
                                .captured
                                .as_ref()
                                .is_some_and(|(key, _, _)| *key == blurred)
                            {
                                this.cancel_target_pointer(
                                    protocol::ui::CanvasCancelReason::FocusLost,
                                    cx,
                                );
                            }
                            this.emit_target(
                                blurred.clone(),
                                CanvasInputKind::Focus { focused: false },
                                cx,
                            );
                        }),
                    ];
                    self.targets.focus.insert(key.clone(), handle);
                    self.targets
                        .subscriptions
                        .insert(key.clone(), subscriptions);
                }
                if let Some(handle) = self.targets.focus.get_mut(&key) {
                    // Register only explicit Tab targets; actual clipped elements determine the
                    // native window's stops, shared with ordinary controls before/after the Canvas.
                    *handle = handle
                        .clone()
                        .tab_stop(target.tab_index.is_some())
                        .tab_index(
                            target
                                .tab_index
                                .map_or(0, |index| isize::try_from(index).unwrap_or(isize::MAX)),
                        );
                }
            }
        }
        self.sync_text_sessions(window, cx);
    }
    /// Install transparent native focus nodes at the same projected rectangles as their hit shapes.
    pub(super) fn target_elements(&self) -> Vec<gpui_kit::AnyElement> {
        let Some(scene) = &self.drawing.scene else {
            return Vec::new();
        };
        let semantic_targets = scene
            .semantics
            .iter()
            .filter_map(|node| node.target.as_deref())
            .collect::<std::collections::BTreeSet<_>>();
        let viewport = self.target_content_viewport();
        scene
            .targets
            .iter()
            .filter_map(|target| {
                if semantic_targets.contains(target.id.as_str()) {
                    return None;
                }
                let handle = self.targets.focus.get(&(target.id.clone(), target.epoch))?;
                let rect = self.scene_graphics.as_ref()?.targets.bounds_in(
                    &target.id,
                    target.epoch,
                    viewport,
                )?;
                let rect = self.project(rect, self.bounds);
                Some(
                    div()
                        .id(format!(
                            "canvas-target:{}:{}:{}",
                            target.id.len(),
                            target.id,
                            target.epoch
                        ))
                        .absolute()
                        .left(px(rect.x))
                        .top(px(rect.y))
                        .w(px(rect.w))
                        .h(px(rect.h))
                        .track_focus(handle)
                        .key_context("CanvasTarget")
                        .into_any_element(),
                )
            })
            .collect()
    }
    /// Native element construction clips the worker's fragments against the current layout viewport.
    pub(in crate::ui::plugin::canvas) fn target_content_viewport(&self) -> protocol::Rect {
        frame::clip_content(
            frame::content_viewport(
                self.bounds,
                self.project(
                    protocol::Rect {
                        x: 0.,
                        y: 0.,
                        w: 1.,
                        h: 1.,
                    },
                    self.bounds,
                ),
            ),
            self.retained_content_clip(),
        )
    }
    /// Emit only from a target in the exact successfully painted scene, with its original source.
    pub(super) fn emit_target(
        &mut self,
        target: (String, u64),
        kind: CanvasInputKind,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(frame) = &self.targets.frame else {
            return false;
        };
        let CanvasEvent::ScenePresented {
            identity,
            scene_revision,
            source,
            file,
            projection,
            error: None,
        } = &frame.receipt
        else {
            return false;
        };
        let cleanup = matches!(
            &kind,
            CanvasInputKind::Focus { focused: false }
                | CanvasInputKind::Composition(protocol::ui::CanvasComposition {
                    phase: protocol::ui::CanvasCompositionPhase::Cancel,
                    ..
                })
                | CanvasInputKind::Pointer(protocol::ui::CanvasPointerInput {
                    phase: protocol::ui::CanvasPointerPhase::Cancel,
                    ..
                })
        );
        if !cleanup
            && (!self.enabled
                || !self.drawing.scene.as_ref().is_some_and(|scene| {
                    *identity == scene.identity
                        && scene
                            .targets
                            .iter()
                            .any(|item| item.id == target.0 && item.epoch == target.1)
                })
                || frame.gesture != self.action_gesture)
        {
            return false;
        }
        if !cleanup
            && !frame
                .graphics
                .source
                .targets
                .iter()
                .any(|item| item.id == target.0 && item.epoch == target.1)
        {
            return false;
        }
        let Some(sequence) = self.targets.sequence.get().checked_add(1) else {
            self.targets.clear();
            return false;
        };
        let input = CanvasInput {
            identity: *identity,
            presented_revision: *scene_revision,
            target: target.0,
            epoch: target.1,
            sequence,
            source: source.clone(),
            file: file.clone(),
            projection: projection.clone(),
            kind,
        };
        if cleanup && input.cleanup_kind().is_none() {
            return false;
        }
        if !cleanup
            && !self
                .drawing
                .scene
                .as_ref()
                .is_some_and(|scene| scene.accepts_input(&input))
        {
            return false;
        }
        if let CanvasInputKind::Focus { focused } = &input.kind {
            let key = (input.target.clone(), input.epoch);
            let changed = if *focused {
                self.targets.focused.insert(key)
            } else {
                self.targets.focused.remove(&key)
            };
            if !changed {
                return false;
            }
        }
        self.targets.sequence.set(sequence);
        (self.sink)(
            CanvasEvent::Input { input },
            frame.revision,
            frame.gesture,
            cx,
        );
        true
    }
    /// Forward complete key transitions from the actual native focus; Tab changes native focus locally.
    pub(super) fn target_key(
        &mut self,
        key: &gpui_kit::Keystroke,
        phase: CanvasKeyPhase,
        repeat: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(target) = self
            .targets
            .focus
            .iter()
            .find(|(_, handle)| handle.is_focused(window))
            .map(|(key, _)| key.clone())
        else {
            return false;
        };
        let modifiers = CanvasModifiers {
            ctrl: key.modifiers.control,
            alt: key.modifiers.alt,
            shift: key.modifiers.shift,
            meta: key.modifiers.platform,
            function: key.modifiers.function,
        };
        if !self.emit_target(
            target.clone(),
            CanvasInputKind::Key {
                phase,
                key: key.key.clone(),
                code: None,
                repeat,
                modifiers,
            },
            cx,
        ) {
            return false;
        }
        if phase == CanvasKeyPhase::Down {
            if let Some(input) = self
                .targets
                .sessions
                .get(&target)
                .map(|input| input.state.clone())
            {
                if let Some(observation) =
                    input.update(cx, |input, cx| input.handle_key(key, window, cx))
                {
                    self.emit_target(target.clone(), observation, cx);
                }
            }
        }
        if phase == CanvasKeyPhase::Down
            && !repeat
            && key.key == "tab"
            && !modifiers.ctrl
            && !modifiers.alt
            && !modifiers.meta
        {
            if modifiers.shift {
                window.focus_prev(cx);
            } else {
                window.focus_next(cx);
            }
            cx.notify();
        }
        true
    }
}
