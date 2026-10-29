//! Project platform geometry while preserving the caller's existing native/linked input pipeline.
use super::*;
use gpui_base::input::RopeExt as _;
use gpui_kit::{
    ClipboardItem, InputHandler, Point, TextInputConfiguration, UTF16Selection, Window,
};

/// A per-painted-frame input adapter. Text, marked ranges, paste and selection are delegated
/// intact; neither document mutations nor a second edit/Undo implementation live here.
pub(super) struct AlignedInputHandler {
    pub delegate: Box<dyn InputHandler>,
    pub editor: Entity<EditorState>,
    pub projection: Option<geometry::Projection>,
    /// Horizontal gutter placement remains active while vertical alignment is paused.
    pub horizontal: Option<gutter::Projection>,
    pub suppress_geometry: bool,
}

impl InputHandler for AlignedInputHandler {
    fn selected_text_range(
        &mut self,
        ignore: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<UTF16Selection> {
        self.delegate.selected_text_range(ignore, window, cx)
    }

    fn marked_text_range(&mut self, window: &mut Window, cx: &mut App) -> Option<Range<usize>> {
        self.delegate.marked_text_range(window, cx)
    }

    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<String> {
        self.delegate.text_for_range(range, adjusted, window, cx)
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.delegate.replace_text_in_range(range, text, window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.delegate
            .replace_and_mark_text_in_range(range, text, selected, window, cx);
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut App) {
        self.delegate.unmark_text(window, cx);
    }

    fn paste(&mut self, item: ClipboardItem, window: &mut Window, cx: &mut App) {
        self.delegate.paste(item, window, cx);
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Bounds<Pixels>> {
        if self.suppress_geometry {
            return None;
        }
        let delegated = self.delegate.bounds_for_range(range.clone(), window, cx);
        let editor = self.editor.read(cx);
        if self
            .projection
            .as_ref()
            .is_some_and(|projection| !projection.ranges.matches(editor))
        {
            return None;
        }
        let text = editor.text();
        if range.start > range.end || range.end > text.len_utf16() {
            return None;
        }
        let bytes =
            text.offset_utf16_to_offset(range.start)..text.offset_utf16_to_offset(range.end);
        // Base's last input window covers one source interval. Other painted pieces still own
        // exact native geometry, rather than the saturating offscreen lookup of that window.
        let outside = editor.visible_row_range().is_none_or(|visible| {
            !visible.contains(&text.offset_to_point(bytes.start).row)
                || !visible.contains(&text.offset_to_point(bytes.end).row)
        });
        let native = if let Some(projection) = self.projection.as_ref().filter(|_| outside) {
            projection
                .ranges
                .bounds(&bytes, &projection.measured, editor)
        } else {
            delegated
        }?;
        let projected = self
            .projection
            .as_ref()
            .map_or(Some(native), |projection| projection.project(native))?;
        Some(
            self.horizontal
                .map_or(projected, |gutter| gutter.text_bounds(projected)),
        )
    }

    fn character_index_for_point(
        &mut self,
        physical: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<usize> {
        if self.suppress_geometry {
            return None;
        }
        let physical = self
            .horizontal
            .map_or(physical, |gutter| gutter.native_point(physical, None));
        let Some(projection) = self.projection.as_ref() else {
            return self
                .delegate
                .character_index_for_point(physical, window, cx);
        };
        let native = projection.native_point(physical)?;
        let editor = self.editor.read(cx);
        if !projection.ranges.matches(editor) {
            return None;
        }
        let text = editor.text();
        let line = projection.measured.line_height;
        let first_y = projection.measured.source_origin.y;
        let row_y = first_y + ((native.y - first_y) / line).floor() * line;
        // Public caret geometry is monotone in source y. Binary search reaches the visual row
        // without scanning a megabyte-long wrapped logical line or copying Base hit-testing.
        let visible = editor.visible_row_range()?;
        if native.y < editor.input_bounds().top() || native.y >= editor.input_bounds().bottom() {
            return projection
                .ranges
                .byte_for_point(native, &projection.measured, editor)
                .map(|byte| text.offset_to_offset_utf16(byte));
        }
        // Offscreen bytes are outside Base's range lookup contract. Restrict the search to its
        // laid-out logical lines, including the complete wrapped line crossing either clip edge.
        let first_byte = text.line_start_offset(visible.start);
        let end_byte = text.line_start_offset(visible.end).min(text.len());
        let mut low = text.byte_to_char_idx(first_byte);
        let mut high = text.byte_to_char_idx(end_byte);
        let last_character = high;
        while low < high {
            let middle = (low + high) / 2;
            let byte = text.char_to_byte_idx(middle);
            let bounds = editor.range_to_bounds(&(byte..byte))?;
            if bounds.top() + px(0.25) < row_y {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        let mut best = None;
        for character in low..=last_character {
            let byte = text.char_to_byte_idx(character);
            let Some(bounds) = editor.range_to_bounds(&(byte..byte)) else {
                break;
            };
            if bounds.top() >= row_y + line - px(0.25) {
                break;
            }
            let distance = (native.x - bounds.left()).abs();
            if best.is_none_or(|(_, before)| distance < before) {
                best = Some((byte, distance));
            }
        }
        best.map(|(byte, _)| text.offset_to_offset_utf16(byte))
    }

    fn set_selected_text_range(&mut self, range: Range<usize>, window: &mut Window, cx: &mut App) {
        self.delegate.set_selected_text_range(range, window, cx);
    }

    fn element_bounds(&mut self, window: &mut Window, cx: &mut App) -> Option<Bounds<Pixels>> {
        if self.suppress_geometry {
            return None;
        }
        self.projection
            .as_ref()
            .map(|projection| projection.viewport)
            .or_else(|| self.horizontal.map(|gutter| gutter.viewport))
            .or_else(|| self.delegate.element_bounds(window, cx))
    }

    fn text_length_utf16(&mut self, window: &mut Window, cx: &mut App) -> Option<usize> {
        self.delegate.text_length_utf16(window, cx)
    }

    fn apple_press_and_hold_enabled(&mut self) -> bool {
        self.delegate.apple_press_and_hold_enabled()
    }

    fn accepts_text_input(&mut self, window: &mut Window, cx: &mut App) -> bool {
        self.delegate.accepts_text_input(window, cx)
    }

    fn text_input_editable_range(
        &mut self,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Range<usize>> {
        self.delegate.text_input_editable_range(window, cx)
    }

    fn prefers_ime_for_printable_keys(&mut self, window: &mut Window, cx: &mut App) -> bool {
        self.delegate.prefers_ime_for_printable_keys(window, cx)
    }

    fn text_input_configuration(
        &mut self,
        window: &mut Window,
        cx: &mut App,
    ) -> TextInputConfiguration {
        self.delegate.text_input_configuration(window, cx)
    }
}

/// Every painted native input frame keeps the original host pipeline, including the finite
/// measurement phase after an edit. Only unavailable geometry is suppressed during that phase.
pub(super) fn install(
    group: &Entity<GroupState>,
    index: usize,
    editor: Entity<EditorState>,
    projection: Option<geometry::Projection>,
    suppress_geometry: bool,
    window: &mut Window,
    cx: &mut App,
) {
    use gpui_kit::{ElementInputHandler, Focusable as _};
    let Some(source) = group.read(cx).sources.get(index) else {
        return;
    };
    if source.editor != editor || group.read(cx).retired {
        return;
    }
    let factory = source.input_factory.clone();
    let horizontal = group.read(cx).gutters.get(index).copied().flatten();
    let native = editor.read(cx);
    let bounds = native.text_bounds().unwrap_or(native.input_bounds());
    let delegate = factory.map_or_else(
        || Box::new(ElementInputHandler::new(bounds, editor.clone())) as Box<dyn InputHandler>,
        |factory| factory(bounds, window, cx),
    );
    window.handle_input(
        &editor.focus_handle(cx),
        AlignedInputHandler {
            delegate,
            editor,
            projection,
            horizontal,
            suppress_geometry,
        },
        cx,
    );
}
