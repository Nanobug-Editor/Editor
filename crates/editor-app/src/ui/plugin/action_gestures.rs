//! Source actions keep short-lived Base gesture identities apart from persistent native focus.
use super::*;
use gpui_kit::SharedString;
use plugin_runtime::plugin_protocol::ui::Node;

/// A native source token change or withdrawal cannot reauthorize a callback from an older frame.
/// The full source token is owned by CodeSync; its changes advance this view-local incarnation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct ActionGesture {
    incarnation: u64,
    revision: u64,
}

impl ActionGesture {
    /// Native collection adapters use only the incarnation to revoke their local pointer state.
    pub(super) fn incarnation(self) -> u64 {
        self.incarnation
    }
}

/// Different controlled options retain focus by public identity, independently of their position.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum FocusTarget {
    Button(String),
    Choice(String, String),
}

#[derive(Default)]
pub(super) struct ActionGestures {
    incarnation: u64,
    pub(super) focus: BTreeMap<FocusTarget, FocusHandle>,
}

/// Base stores a pending press by element ID. Only source actions need a new ID after revocation;
/// the surrounding containers, native inputs and scroll state keep their existing identities.
pub(super) fn native_id(
    id: impl Into<SharedString>,
    gesture: Option<ActionGesture>,
) -> SharedString {
    let id = id.into();
    match gesture {
        Some(gesture) => format!("{id}-action-{}", gesture.incarnation).into(),
        None => id,
    }
}

impl PluginView {
    /// Revoke old listeners even when false/true transitions occur before a replacement frame.
    /// This changes no text identity: only a newly bound complete native source token samples text.
    pub(super) fn invalidate_action_gestures(&mut self, cx: &mut Context<Self>) {
        self.action_gestures.incarnation = self.action_gestures.incarnation.wrapping_add(1);
        self.link_press = None;
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
    }

    /// Ordinary controls retain their existing IDs and event semantics. A source control captures
    /// both its UI revision and the incarnation shared by source-token changes and withdrawals.
    pub(super) fn action_gesture(&self, id: &str) -> Option<ActionGesture> {
        self.document
            .editor_action_source(id)
            .map(|_| ActionGesture {
                incarnation: self.action_gestures.incarnation,
                revision: self.document.revision,
            })
    }

    /// Check before forwarding through the existing IME, native identity and exact owner gates.
    /// A control changing between ordinary and source-owned cannot inherit the other's listener.
    pub(super) fn action_gesture_current(&self, id: &str, gesture: Option<ActionGesture>) -> bool {
        self.action_gesture(id) == gesture
    }

    /// Retain only focus resources for currently declared source controls. Base still owns their
    /// activation and traversal; tabs have no native focus state in the upstream primitive.
    pub(super) fn sync_action_focus(&mut self, nodes: &[Node], cx: &mut Context<Self>) {
        let mut targets = BTreeSet::new();
        for node in nodes {
            if self.document.editor_action_source(&node.id).is_none() {
                continue;
            }
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
