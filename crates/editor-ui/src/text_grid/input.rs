//! Native pointer capture and UTF-16 IME ranges for one retained fixed-cell surface.

use super::*;

impl TextGrid {
    /// Keep a drag owned by this grid until release, including movement beyond the original bounds.
    pub(super) fn capture_pointer(&self, window: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
            if !phase.capture() {
                return;
            }
            let _ = owner.update(cx, |grid, cx| {
                if grid.drag.is_some() && grid.drag == event.pressed_button {
                    grid.pointer(
                        GridPointerPhase::Move,
                        event.position,
                        button_index(grid.drag.unwrap()),
                        1,
                        event.modifiers,
                        cx,
                    );
                    cx.stop_propagation();
                }
            });
        });
        let owner = cx.entity().downgrade();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
            if !phase.capture() {
                return;
            }
            let _ = owner.update(cx, |grid, cx| {
                if grid.drag == Some(event.button) {
                    grid.drag = None;
                    grid.pointer(
                        GridPointerPhase::Up,
                        event.position,
                        button_index(event.button),
                        1,
                        event.modifiers,
                        cx,
                    );
                    cx.stop_propagation();
                }
            });
        });
    }

    /// Preserve full modifiers and local coordinates; the owner decides selection or mouse protocols.
    pub(super) fn pointer(
        &self,
        phase: GridPointerPhase,
        position: Point<Pixels>,
        button: u8,
        clicks: u8,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        let at = position - self.bounds.origin;
        self.emit(
            GridEvent::Pointer {
                phase,
                x: at.x / px(1.),
                y: at.y / px(1.),
                button,
                clicks,
                shift: modifiers.shift,
                modifiers,
            },
            cx,
        );
    }
}

impl EntityInputHandler for TextGrid {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let (bytes, units) = composition_range(&self.composition, range);
        *adjusted = Some(units);
        Some(self.composition[bytes].to_owned())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.selection.clone(),
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        (!self.composition.is_empty()).then(|| 0..self.composition.encode_utf16().count())
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.composition.clear();
        self.selection = 0..0;
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range.unwrap_or(0..self.composition.encode_utf16().count());
        let (bytes, _) = composition_range(&self.composition, range);
        self.composition.replace_range(bytes, text);
        let text = std::mem::take(&mut self.composition);
        self.selection = 0..0;
        self.emit(GridEvent::Text { text }, cx);
        cx.notify();
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range.unwrap_or(0..self.composition.encode_utf16().count());
        let (bytes, units) = composition_range(&self.composition, range);
        self.composition.replace_range(bytes, text);
        let end = text.encode_utf16().count();
        let selected = selected.unwrap_or(end..end);
        self.selection = composition_range(
            &self.composition,
            units.start + selected.start..units.start + selected.end,
        )
        .1;
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(rect_bounds(
            self.drawing.caret.unwrap_or(GridRect {
                w: 1.,
                h: self.drawing.font.size_px,
                ..Default::default()
            }),
            self.bounds.origin,
        ))
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(0)
    }
}

/// Convert native buttons to the grid's portable left/middle/right numbering.
fn button_index(button: MouseButton) -> u8 {
    match button {
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        _ => 0,
    }
}

/// Clamp an IME UTF-16 range to scalar boundaries without splitting a surrogate pair or UTF-8 codepoint.
pub(super) fn composition_range(text: &str, range: Range<usize>) -> (Range<usize>, Range<usize>) {
    let mut boundaries = vec![(0, 0)];
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        units += ch.len_utf16();
        boundaries.push((byte + ch.len_utf8(), units));
    }
    let start = boundaries
        .iter()
        .copied()
        .take_while(|(_, units)| *units <= range.start)
        .last()
        .unwrap();
    let end = boundaries
        .iter()
        .copied()
        .find(|(_, units)| *units >= range.end.max(start.1))
        .unwrap_or((text.len(), units));
    (start.0..end.0, start.1..end.1)
}
