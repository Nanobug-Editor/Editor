//! Apply each clipped copy's public native scroll request before Base chooses its visible rows.
use super::*;
use gpui_kit::{Element, ElementId, GlobalElementId, InspectorElementId, LayoutId, Window};

/// A layout-transparent native mount; all text, input and wrapping still belong to `editor`.
pub(super) struct NativeViewport {
    /// The original source identity shared by visual copies and the last input authority.
    editor: Entity<EditorState>,
    /// Native y for this copy; the primer leaves it unset to consume the caller's deferred request.
    y: Option<Pixels>,
    /// A local thumb request overrides x; otherwise preserve native deferred requests after priming.
    horizontal: Option<Pixels>,
    /// Original control, including source-owned atomic factories and row backgrounds.
    content: AnyElement,
}

impl NativeViewport {
    /// Mount original `editor`/`content` at native `y`. `horizontal` applies a local thumb request;
    /// None preserves x after native deferred scrolling is consumed by the preceding primer.
    pub fn new(
        editor: &Entity<EditorState>,
        horizontal: Option<Pixels>,
        y: Pixels,
        content: AnyElement,
    ) -> Self {
        Self {
            editor: editor.clone(),
            y: Some(y),
            horizontal,
            content,
        }
    }

    /// Consume deferred caller scrolling before source copies, with no offset override or rails.
    pub fn primer(editor: &Entity<EditorState>) -> Self {
        Self {
            editor: editor.clone(),
            y: None,
            horizontal: None,
            content: crate::scrollbar::native_editor(editor),
        }
    }
}

impl IntoElement for NativeViewport {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for NativeViewport {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.content.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        // Base reads deferred scrolling when selecting visible logical lines. Prepainting all
        // copies first is safe: each retains its own native layout and applied offset for paint.
        if let Some(y) = self.y {
            self.editor.update(cx, |editor, cx| {
                let x = self.horizontal.unwrap_or(editor.scroll_offset().x);
                editor.set_scroll_offset(gpui_kit::point(x, y), cx);
            });
        }
        crate::scrollbar::with_external_scrollbars(window, cx, |window, cx| {
            self.content.prepaint(window, cx);
        });
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.content.paint(window, cx);
    }
}
