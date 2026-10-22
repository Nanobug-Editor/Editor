//! Constant-time identities for native document bytes, including notify-only IME preedit.
use gpui_base::input::Rope;

/// An immutable COW snapshot used only to revoke stale capabilities; never an editable document.
/// Retaining its shared root makes ordinary scroll/selection checks constant time without hashing
/// source text. EditorState and DocumentSession remain the sole mutable text and Undo owners.
#[derive(Clone)]
pub(crate) struct NativeTextIdentity(Rope);

impl NativeTextIdentity {
    /// Capture the current native instance in O(1), sharing rather than copying its content.
    pub(crate) fn new(text: &Rope) -> Self {
        Self(text.clone())
    }

    /// Different bytes always fail. Modified equal-byte instances may conservatively fail too;
    /// that only advances a capability or asks for a fresh proposal, never grants stale authority.
    /// There is no size-based shortcut because this check also guards action permissions.
    pub(crate) fn matches(&self, text: &Rope) -> bool {
        ropey::extra::esoterica::ropes_are_instances(&self.0, text)
    }

    /// Materialize a sealed snapshot only when its background save is ready to stage native bytes.
    pub(crate) fn contents(&self) -> String {
        self.0.to_string()
    }
}
