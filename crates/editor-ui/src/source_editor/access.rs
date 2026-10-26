//! Temporary input restrictions borrow the existing session and restore its owner's policy.
use gpui_base::input::EditorState;
use gpui_kit::{App, AppContext as _, Entity, EntityId, Global, WeakEntity};
use std::collections::BTreeMap;

/// Keep user input readonly while a unique source mount is displayed. Clones share one restriction;
/// dropping the last lease restores the captured owner policy without changing text or Undo.
#[derive(Clone)]
pub struct ReadOnlyLease {
    /// Shared lifecycle owner; the weak registry never prolongs this restriction.
    _restriction: Entity<Restriction>,
}

/// Weak registry prevents overlapping native owners from restoring editing before the last release.
#[derive(Default)]
struct Restrictions(BTreeMap<EntityId, Entry>);
impl Global for Restrictions {}

/// A pending final release retains the original policy until its replacement is established.
struct Entry {
    active: WeakEntity<Restriction>,
    original: bool,
}

/// The lifecycle entity owns only an input policy, never document bytes or editor history.
struct Restriction {
    editor: Entity<EditorState>,
    original: bool,
    token: EntityId,
}

impl ReadOnlyLease {
    /// Restrict `editor` without relaxing an already readonly owner. Callers retain this lease for
    /// the complete mount lifetime and release it on hide, source replacement or owner retirement.
    pub fn new(editor: &Entity<EditorState>, cx: &mut App) -> Self {
        let previous = cx
            .try_global::<Restrictions>()
            .and_then(|registry| registry.0.get(&editor.entity_id()))
            .map(|entry| (entry.active.upgrade(), entry.original));
        if let Some((Some(restriction), _)) = &previous {
            return Self {
                _restriction: restriction.clone(),
            };
        }
        let original = previous.map_or_else(
            || editor.read(cx).presentation().is_readonly(),
            |(_, original)| original,
        );
        editor.update(cx, |editor, cx| editor.set_readonly(true, cx));
        let source = editor.clone();
        let restriction = cx.new(|cx| {
            cx.on_release(|this: &mut Restriction, cx| {
                // A newly mounted replacement may exist before this deferred release is delivered.
                let current = cx
                    .try_global::<Restrictions>()
                    .and_then(|registry| registry.0.get(&this.editor.entity_id()))
                    .is_some_and(|entry| entry.active.entity_id() == this.token);
                if current {
                    cx.global_mut::<Restrictions>()
                        .0
                        .remove(&this.editor.entity_id());
                    this.editor
                        .update(cx, |editor, cx| editor.set_readonly(this.original, cx));
                }
            })
            .detach();
            Restriction {
                editor: source,
                original,
                token: cx.entity().entity_id(),
            }
        });
        if cx.try_global::<Restrictions>().is_none() {
            cx.set_global(Restrictions::default());
        }
        cx.global_mut::<Restrictions>().0.insert(
            editor.entity_id(),
            Entry {
                active: restriction.downgrade(),
                original,
            },
        );
        Self {
            _restriction: restriction,
        }
    }
}
