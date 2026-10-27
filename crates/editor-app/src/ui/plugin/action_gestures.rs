//! Source and retained-scene actions separate short-lived Base presses from persistent native focus.
use super::*;
use gpui_kit::SharedString;
use plugin_runtime::plugin_protocol::ui::Node;

/// Source changes and visibility withdrawal cannot reauthorize an earlier frame's press.
/// Source/visibility actions share a view nonce; overlay actions additionally follow their
/// semantic owner chain. None of these private native credentials grants document authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ActionGesture {
    /// Editor edge actions also capture the UI revision supplying their document-source binding.
    Source { incarnation: u64, revision: u64 },
    /// Literal Editor rails have exact UI ownership without a document-source credential.
    Snapshot { incarnation: u64, revision: u64 },
    /// Ordinary controls retained after hiding capture only their native scene lifetime.
    Visibility { incarnation: u64 },
    /// Native foreground controls retain their owning target lineage, independently of artwork revisions.
    Overlay {
        incarnation: u64,
        revision: Option<u64>,
    },
}

impl ActionGesture {
    /// Native collection adapters use only the incarnation to revoke their local pointer state.
    pub(super) fn incarnation(self) -> u64 {
        match self {
            Self::Source { incarnation, .. }
            | Self::Snapshot { incarnation, .. }
            | Self::Visibility { incarnation }
            | Self::Overlay { incarnation, .. } => incarnation,
        }
    }
}

/// Different controlled options retain focus by public identity, independently of their position.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum FocusTarget {
    Button(String),
    Choice(String, String),
}

/// Native image input has no Base click owner. The sequence prevents deferred outside-release
/// cleanup from discarding a subsequent fresh Down within the same admitted source scene.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct VisualPress {
    gesture: ActionGesture,
    sequence: u64,
}

#[derive(Default)]
pub(super) struct ActionGestures {
    incarnation: u64,
    /// After first withdrawal, ordinary frames need credentials too because their entities survive.
    pub(super) retained_visibility: bool,
    pub(super) focus: BTreeMap<FocusTarget, FocusHandle>,
    /// A deferred outside release may clear only the exact source-image Down it observed.
    visual_press_sequence: u64,
    visual_presses: BTreeMap<(String, u8), VisualPress>,
}

/// Base stores a pending press by element ID. Retained actions need a new ID after revocation;
/// the surrounding containers, native inputs and scroll state keep their existing identities.
pub(super) fn native_id(
    id: impl Into<SharedString>,
    gesture: Option<ActionGesture>,
) -> SharedString {
    let id = id.into();
    match gesture {
        Some(ActionGesture::Source { incarnation, .. }) => {
            format!("{id}-action-{incarnation}").into()
        }
        Some(ActionGesture::Snapshot { incarnation, .. }) => {
            format!("{id}-action-snapshot-{incarnation}").into()
        }
        Some(ActionGesture::Visibility { incarnation }) => {
            format!("{id}-action-visibility-{incarnation}").into()
        }
        Some(ActionGesture::Overlay {
            incarnation,
            revision: Some(revision),
        }) => format!("{id}-action-overlay-{incarnation}-{revision}").into(),
        Some(ActionGesture::Overlay {
            incarnation,
            revision: None,
        }) => format!("{id}-action-overlay-{incarnation}").into(),
        None => id,
    }
}

impl PluginView {
    /// Record ownership before forwarding Down; scene changes in the sink can revoke it.
    pub(super) fn press_visual(
        &mut self,
        node: &str,
        button: u8,
        gesture: ActionGesture,
    ) -> VisualPress {
        self.action_gestures.visual_press_sequence =
            self.action_gestures.visual_press_sequence.wrapping_add(1);
        let press = VisualPress {
            gesture,
            sequence: self.action_gestures.visual_press_sequence,
        };
        self.action_gestures
            .visual_presses
            .insert((node.to_owned(), button), press);
        press
    }

    /// A held move or release belongs to the exact source and button that supplied the Down.
    pub(super) fn visual_press_current(
        &self,
        node: &str,
        button: u8,
        gesture: ActionGesture,
    ) -> bool {
        self.action_gesture_current(node, Some(gesture))
            && self
                .action_gestures
                .visual_presses
                .get(&(node.to_owned(), button))
                .is_some_and(|press| press.gesture == gesture)
    }

    /// Lost/outside releases and rejected Down events withdraw only their original ticket.
    pub(super) fn forget_visual_press(&mut self, node: &str, button: u8, press: VisualPress) {
        let key = (node.to_owned(), button);
        if self.action_gestures.visual_presses.get(&key) == Some(&press) {
            self.action_gestures.visual_presses.remove(&key);
        }
    }

    /// A replacement or foreign source viewport cannot consume another frame's pending Down.
    pub(super) fn accept_visual_release(
        &mut self,
        node: &str,
        button: u8,
        gesture: ActionGesture,
    ) -> bool {
        if !self.visual_press_current(node, button, gesture) {
            return false;
        }
        self.action_gestures
            .visual_presses
            .remove(&(node.to_owned(), button));
        true
    }

    /// Clear outside releases after the matching native Up has had its turn. Capture the exact
    /// ticket now: a fresh Down before deferred cleanup must retain its new ownership.
    pub(super) fn track_visual_releases(
        owner: gpui_kit::WeakEntity<Self>,
        node: String,
        gesture: ActionGesture,
        window: &mut Window,
    ) {
        window.on_mouse_event(move |event: &gpui_kit::MouseUpEvent, phase, _, cx| {
            if !phase.capture() {
                return;
            }
            let button = match event.button {
                gpui_kit::MouseButton::Left => 0,
                gpui_kit::MouseButton::Middle => 1,
                gpui_kit::MouseButton::Right => 2,
                _ => return,
            };
            let press = owner
                .update(cx, |view, _| {
                    view.action_gestures
                        .visual_presses
                        .get(&(node.clone(), button))
                        .copied()
                        .filter(|press| press.gesture == gesture)
                })
                .ok()
                .flatten();
            if let Some(press) = press {
                let owner = owner.clone();
                let node = node.clone();
                cx.defer(move |cx| {
                    let _ = owner.update(cx, |view, _| {
                        view.forget_visual_press(&node, button, press);
                    });
                });
            }
        });
    }

    /// Revoke old listeners even when false/true transitions occur before a replacement frame.
    /// This changes no text identity: only a newly bound complete native source token samples text.
    pub(super) fn invalidate_action_gestures(&mut self, cx: &mut Context<Self>) {
        self.action_gestures.incarnation = self.action_gestures.incarnation.wrapping_add(1);
        self.link_press = None;
        self.action_gestures.visual_presses.clear();
        // Keyed collection/canvas entities keep mutable native state across renders. Revoke their
        // source-owned presses now, including false/true transitions with no intervening frame.
        for (id, canvas) in &self.canvases {
            let gesture = self.action_gesture(id);
            canvas.update(cx, |view, cx| {
                if view.set_action_gesture(gesture) {
                    cx.notify();
                }
            });
        }
        for (id, collection) in &self.collections {
            let gesture = self.action_gesture(id);
            collection.update(cx, |view, cx| view.set_action_gesture(gesture, cx));
        }
        self.sync_tree_action_gestures(cx);
    }

    /// Source controls capture their UI revision. Once retained across hiding, ordinary controls
    /// also capture the shared nonce, with a distinct kind even when a source's revision is zero.
    pub(super) fn action_gesture(&self, id: &str) -> Option<ActionGesture> {
        if let Some(scope) = self.overlays.scopes.get(id) {
            return Some(ActionGesture::Overlay {
                incarnation: scope.generation,
                revision: self
                    .document
                    .is_editor_action(id)
                    .then_some(self.document.revision),
            });
        }
        if self.document.editor_action_source(id).is_some() {
            Some(ActionGesture::Source {
                incarnation: self.action_gestures.incarnation,
                revision: self.document.revision,
            })
        } else if self.document.is_editor_action(id) {
            Some(ActionGesture::Snapshot {
                incarnation: self.action_gestures.incarnation,
                revision: self.document.revision,
            })
        } else if self.action_gestures.retained_visibility {
            Some(ActionGesture::Visibility {
                incarnation: self.action_gestures.incarnation,
            })
        } else {
            None
        }
    }

    /// Check before forwarding through the existing IME, native identity and exact owner gates.
    /// A control changing between ordinary and source-owned cannot inherit the other's listener.
    pub(super) fn action_gesture_current(&self, id: &str, gesture: Option<ActionGesture>) -> bool {
        self.action_gesture(id) == gesture
    }

    /// Keep button/choice focus independent of revocable native press IDs, including ordinary
    /// controls before their first hide. Base still owns activation and traversal.
    pub(super) fn sync_action_focus(&mut self, nodes: &[Node], cx: &mut Context<Self>) {
        let mut targets = BTreeSet::new();
        for node in nodes {
            match &node.kind {
                Kind::Button { .. } => {
                    targets.insert(FocusTarget::Button(node.id.clone()));
                }
                Kind::Choice { options, .. } => {
                    for option in options {
                        targets.insert(FocusTarget::Choice(node.id.clone(), option.id.clone()));
                    }
                }
                _ => {}
            }
        }
        self.action_gestures
            .focus
            .retain(|key, _| targets.contains(key));
        for target in targets {
            self.action_gestures
                .focus
                .entry(target)
                .or_insert_with(|| cx.focus_handle());
        }
    }
}
