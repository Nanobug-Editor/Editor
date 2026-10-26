//! Immutable Editor content retains native selection, focus and scrolling between publications.
use crate::row_backgrounds::RowBackgroundCollection;
use gpui_base::input::{EditorState, RangeDecoration};
use gpui_kit::{App, AppContext as _, Entity, Window};

/// A literal source owned by its native surface; it has no document/save or language-tool authority.
pub struct Snapshot {
    /// The sole native text/selection/layout state for this literal source.
    pub editor: Entity<EditorState>,
    /// Changes only when literal bytes change, invalidating retained alignment maps.
    pub generation: u64,
    /// Last immutable publication, used only to avoid resetting native selection on repaint.
    text: String,
    /// Decorations are instance-local and removed when the surface withdraws the source.
    marks: RowBackgroundCollection,
}

impl Snapshot {
    /// Create a readonly literal source with native code geometry and owned row decorations.
    pub fn new(
        text: String,
        marks: Vec<RangeDecoration>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("text")
                .line_number(true)
                .soft_wrap(false)
        });
        let marks = editor.update(cx, |editor, cx| {
            editor.set_value(text.clone(), window, cx);
            editor.set_readonly(true, cx);
            RowBackgroundCollection::new(&cx.entity(), marks, cx)
        });
        Self {
            editor,
            generation: 0,
            text,
            marks,
        }
    }

    /// Replace changed bytes without rebuilding the native entity; unchanged bytes retain selection.
    pub fn update(
        &mut self,
        text: &str,
        marks: Vec<RangeDecoration>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.text != text {
            self.editor.update(cx, |editor, cx| {
                editor.set_value(text.to_owned(), window, cx)
            });
            self.text = text.to_owned();
            self.generation = self.generation.wrapping_add(1);
        }
        self.marks.set(marks, cx);
    }

    /// Withdraw owned decoration resources before the containing surface releases its source.
    pub fn dispose(self, cx: &mut App) {
        self.marks.dispose(cx);
    }
}
