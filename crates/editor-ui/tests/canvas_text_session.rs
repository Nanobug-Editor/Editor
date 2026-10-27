//! Public native draft behavior preserves source graphemes and pending acknowledgement order.
use editor_ui::text::session::CanvasTextSession;
use plugin_runtime::plugin_protocol::ui::{CanvasTextRange, TextInputSession};

/// A preedit cannot leave a native marked boundary inside an adjacent committed grapheme.
#[test]
fn native_preedit_join_rejects_atomically() {
    let binding = TextInputSession {
        paragraph: "label".into(),
        layout_key: "one".into(),
        text_revision: 1,
        selection: CanvasTextRange { start: 1, end: 1 },
        acknowledged_sequence: 0,
        read_only: false,
        editing_keys: false,
    };
    let mut session = CanvasTextSession::new(&binding, "e").unwrap();
    assert!(session.mark(None, "\u{301}", None).is_err());
    assert_eq!(session.text(), "e");
    assert_eq!(session.selection_utf16(), 1..1);
    assert_eq!(session.marked_utf16(), None);
}

/// A guest acknowledgement cannot move a selection made after the acknowledged native commit.
#[test]
fn native_selection_after_commit_survives_its_late_acknowledgement() {
    let mut binding = TextInputSession {
        paragraph: "label".into(),
        layout_key: "one".into(),
        text_revision: 1,
        selection: CanvasTextRange { start: 3, end: 3 },
        acknowledged_sequence: 0,
        read_only: false,
        editing_keys: true,
    };
    let mut session = CanvasTextSession::new(&binding, "ABC").unwrap();
    session.commit(Some(2..3), "D", 7).unwrap();
    session.select(0..1);
    binding.text_revision = 2;
    binding.acknowledged_sequence = 7;
    session.reconcile(&binding, "ABD").unwrap();
    assert_eq!(session.selection_utf16(), 0..1);
    assert_eq!(session.text(), "ABD");
}
