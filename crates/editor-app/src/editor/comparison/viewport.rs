//! Retained tabs remember display positions without retaining another editable document or live input.
use super::*;

/// One comparison's last display offset and independent native horizontal/unified positions.
#[derive(Clone)]
pub(super) struct Viewport {
    /// Positive shared vertical position, applied after alignment/folding finishes admission.
    display: Pixels,
    /// Original source scroll positions, in left/result/optional-third order.
    sources: Vec<gpui_kit::Point<Pixels>>,
    /// Unified mode has its own derived editor and native viewport.
    unified: Option<gpui_kit::Point<Pixels>>,
}

impl EditorApp {
    /// Capture before disposing presentation, including when an ordinary document becomes active.
    pub(super) fn remember_comparison_viewport(&mut self, cx: &App) {
        let Some(comparison) = &self.document_comparison else {
            return;
        };
        let Some(id) = comparison.persistent.as_ref() else {
            return;
        };
        let snapshot = Viewport {
            display: comparison.alignment.scroll_offset(cx),
            sources: [&comparison.left_editor, &comparison.right_editor]
                .into_iter()
                .chain(comparison.third.as_ref().map(|third| &third.editor))
                .map(|editor| editor.read(cx).scroll_offset())
                .collect(),
            unified: comparison
                .unified_editor
                .as_ref()
                .map(|editor| editor.read(cx).scroll_offset()),
        };
        if let Some(view) = self.comparison_views.iter_mut().find(|view| &view.id == id) {
            view.viewport = Some(snapshot);
        }
    }

    /// Restore native x offsets immediately and defer shared y until the new exact layout is known.
    pub(super) fn restore_comparison_viewport(&self, viewport: &Viewport, cx: &mut App) {
        let Some(comparison) = &self.document_comparison else {
            return;
        };
        for (editor, offset) in [&comparison.left_editor, &comparison.right_editor]
            .into_iter()
            .chain(comparison.third.as_ref().map(|third| &third.editor))
            .zip(&viewport.sources)
        {
            editor.update(cx, |editor, cx| editor.set_scroll_offset(*offset, cx));
        }
        comparison
            .alignment
            .restore_scroll_offset(viewport.display, cx);
        if let (Some(editor), Some(offset)) = (&comparison.unified_editor, viewport.unified) {
            editor.update(cx, |editor, cx| editor.set_scroll_offset(offset, cx));
        }
    }
}
