//! A sealed native Canvas text endpoint owns only preedit/pending drafts, never a domain undo stack.
use super::{
    paragraph::{CanvasParagraph, CanvasTextLayout},
    session::CanvasTextSession,
};
use gpui_kit::{
    App, Bounds, Context, EntityInputHandler, FocusHandle, Pixels, Point, UTF16Selection, Window,
    fill, px, rgba,
};
use plugin_runtime::plugin_protocol::{
    api::Failure,
    ui::{CanvasInputKind, CanvasTextRange, TextInputSession, canvas_text_range_from_utf16},
};
use std::{cell::Cell, ops::Range, rc::Rc};
mod keys;

/// Immutable native presentation data for a worker; it grants no source edit or undo authority.
#[derive(Clone, PartialEq)]
pub struct CanvasTextPresentation {
    /// Bounded temporary native text including preedit and pending commits.
    pub text: String,
    /// Original UTF-8 selection and optional preedit span in that same text.
    pub selection: CanvasTextRange,
    pub marked: Option<CanvasTextRange>,
    /// Temporary plaintext does not reinterpret the guest's rich-style source ranges.
    pub transient: bool,
}

/// The native owner alone may revoke a text endpoint; stale platform handlers retain only its lease.
pub struct CanvasInputOwner(Rc<Cell<bool>>);
/// Read-only lifetime evidence, separate from focus and target business identity.
#[derive(Clone)]
pub struct CanvasInputLease(Rc<Cell<bool>>);
impl CanvasInputOwner {
    /// Create a live endpoint lifetime for one exact target epoch.
    pub fn new() -> Self {
        Self(Rc::new(Cell::new(true)))
    }
    /// Lend revocable evidence without allowing a native handler to revive itself.
    pub fn lease(&self) -> CanvasInputLease {
        CanvasInputLease(self.0.clone())
    }
}
impl Default for CanvasInputOwner {
    fn default() -> Self {
        Self::new()
    }
}
impl Drop for CanvasInputOwner {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

/// Native text callbacks carry the target epoch through this independently revocable entity.
pub struct CanvasInputState {
    lease: CanvasInputLease,
    focus: FocusHandle,
    model: CanvasTextSession,
    layout: Option<CanvasTextLayout>,
    /// Shared native input sequence; the host sink performs the actual scoped emission.
    sequence: Rc<Cell<u64>>,
    sink: Rc<dyn Fn(CanvasInputKind, &mut Window, &mut App) -> bool>,
    /// Repeated native failures do not create an unbounded observation loop.
    error: Option<(plugin_runtime::plugin_protocol::api::ErrorCode, String)>,
    /// Shift navigation retains the selection's active end without changing original-byte ranges.
    reversed: bool,
}
impl CanvasInputState {
    /// Create a native endpoint from the declared committed source and an exact target lifetime.
    /// The host `sink` must preserve frame/instance/panel authority and advance the shared sequence.
    /// Invalid selection or exhausted draft budgets fail before native state is installed.
    pub fn new(
        binding: &TextInputSession,
        text: &str,
        focus: FocusHandle,
        lease: CanvasInputLease,
        sequence: Rc<Cell<u64>>,
        sink: impl Fn(CanvasInputKind, &mut Window, &mut App) -> bool + 'static,
    ) -> Result<Self, Failure> {
        Ok(Self {
            lease,
            focus,
            model: CanvasTextSession::new(binding, text)?,
            layout: None,
            sequence,
            sink: Rc::new(sink),
            error: None,
            reversed: false,
        })
    }
    /// Reconcile committed source/acknowledgements without replacing later native drafts or preedit.
    pub fn reconcile(&mut self, binding: &TextInputSession, text: &str) -> Result<(), Failure> {
        let selection = self.model.selection();
        self.model.reconcile(binding, text)?;
        if self.model.selection() != selection {
            self.reversed = false;
        }
        Ok(())
    }
    /// Current declaration for host layout association, without exposing mutable native text.
    pub fn binding(&self) -> &TextInputSession {
        self.model.binding()
    }
    /// Current native draft, selection and marked source are read-only presentation data.
    pub fn text(&self) -> &str {
        self.model.text()
    }
    /// Snapshot only temporary native presentation for bounded affine raster preparation.
    pub fn presentation(&self) -> CanvasTextPresentation {
        CanvasTextPresentation {
            text: self.model.text().to_owned(),
            selection: self.model.selection(),
            marked: self.model.marked_range(),
            transient: self.model.is_transient(),
        }
    }
    /// Native input ownership is independent of the enclosing Canvas's legacy focus option.
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }
    /// An epoch/source retirement revokes even a platform handler retained by the previous frame.
    pub fn active(&self, window: &Window) -> bool {
        self.lease.0.get() && window.is_window_active() && self.focus.is_focused(window)
    }
    /// Resolve actual native geometry once at paint, invalidating OS candidate coordinates on change.
    pub fn set_layout(&mut self, layout: CanvasTextLayout, window: &Window) {
        if self.layout.as_ref() != Some(&layout) {
            self.layout = Some(layout);
            window.invalidate_character_coordinates();
        }
    }
    /// Paint the current native draft with the same glyph layout used by all OS geometry queries.
    /// Selection/preedit/caret are presentation only; no domain edit or undo is synthesized here.
    pub fn paint(&self, window: &mut Window, cx: &mut App) -> Result<(), String> {
        let layout = self.shaped(window)?;
        let selection = self.model.selection();
        for rect in layout.selection_rects(selection.start..selection.end)? {
            window.paint_quad(fill(rect, rgba(0x3388ff44)));
        }
        layout.paint(window, cx)?;
        if let Some(marked) = self.model.marked_range() {
            for mut rect in layout.selection_rects(marked.start..marked.end)? {
                rect.origin.y += rect.size.height - px(2.);
                rect.size.height = px(1.);
                window.paint_quad(fill(rect, self.layout.as_ref().unwrap().color));
            }
        }
        if selection.start == selection.end {
            if let Some(rect) = layout.caret_bounds(selection.end, true) {
                window.paint_quad(fill(rect, self.layout.as_ref().unwrap().color));
            }
        }
        Ok(())
    }
    /// Cancel only native preedit; the owner emits the returned scoped lifecycle observation.
    pub fn cancel(&mut self) -> Option<CanvasInputKind> {
        self.model.cancel().map(CanvasInputKind::Composition)
    }
    /// Shape against the actual installed native projection, retaining original source offsets.
    fn shaped(&self, window: &Window) -> Result<CanvasParagraph, String> {
        let context = self
            .layout
            .as_ref()
            .ok_or("Native Canvas text has no shown layout")?;
        let mut style = context.style.clone();
        // Rich visual spans remain guest-owned. Temporary plain-text drafts inherit paragraph style;
        // acknowledged source restores the guest's newly published style ranges.
        if self.model.is_transient() {
            style.runs.clear();
        }
        CanvasParagraph::new(
            self.model.text().to_owned().into(),
            context.font.clone(),
            context.font_size,
            context.color,
            context.bounds,
            &style,
            window,
        )
    }
    /// Forward only from a live endpoint; failed routing never changes another target's text.
    fn emit(&self, kind: CanvasInputKind, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.lease.0.get() && (self.sink)(kind, window, cx)
    }
    /// Emit a recoverable native error once while preserving the current draft on failure.
    fn fail(&mut self, error: Failure, window: &mut Window, cx: &mut Context<Self>) {
        let message = error.message.chars().take(128).collect::<String>();
        let next = (error.code, message.clone());
        if self.error.as_ref() != Some(&next) {
            self.emit(
                CanvasInputKind::TextError {
                    code: error.code,
                    message,
                },
                window,
                cx,
            );
            self.error = Some(next);
        }
    }
    /// Commit independently from keys; an empty marked result is native cancellation.
    fn replace(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.active(window) || self.model.binding().read_only {
            return;
        }
        if text.is_empty() && range.is_none() && self.model.marked_range().is_some() {
            if let Some(event) = self.cancel() {
                self.emit(event, window, cx);
            }
            cx.notify();
            window.invalidate_character_coordinates();
            return;
        }
        let Some(sequence) = self.sequence.get().checked_add(1) else {
            return;
        };
        match self.model.commit(range, text, sequence) {
            Ok((edit, ended)) => {
                self.reversed = false;
                self.emit(CanvasInputKind::Text(edit), window, cx);
                if let Some(ended) = ended {
                    self.emit(CanvasInputKind::Composition(ended), window, cx);
                }
                self.error = None;
                cx.notify();
                window.invalidate_character_coordinates();
            }
            Err(error) => self.fail(error, window, cx),
        }
    }
}
impl EntityInputHandler for CanvasInputState {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        if !self.active(window) {
            return None;
        }
        let (text, units) = self.model.text_for_range(range);
        *adjusted = Some(units);
        Some(text)
    }
    fn selected_text_range(
        &mut self,
        _ignore: bool,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        self.active(window).then(|| UTF16Selection {
            range: self.model.selection_utf16(),
            reversed: self.reversed,
        })
    }
    fn marked_text_range(
        &self,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.active(window)
            .then(|| self.model.marked_utf16())
            .flatten()
    }
    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.active(window) {
            return;
        }
        if let Some(range) = self.model.marked_range() {
            let text = self.model.text()[range.start..range.end].to_owned();
            self.replace(None, &text, window, cx);
        }
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace(range, text, window, cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.active(window) || self.model.binding().read_only {
            return;
        }
        match self.model.mark(range, text, selected) {
            Ok(Some(event)) => {
                self.reversed = false;
                self.emit(CanvasInputKind::Composition(event), window, cx);
                self.error = None;
                cx.notify();
                window.invalidate_character_coordinates();
            }
            Ok(None) => {}
            Err(error) => self.fail(error, window, cx),
        }
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _bounds: Bounds<Pixels>,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        if !self.active(window) {
            return None;
        }
        let range = canvas_text_range_from_utf16(self.model.text(), range);
        let rect = self.shaped(window).ok()?.caret_bounds(range.end, true)?;
        Some(self.layout.as_ref()?.window_rect(rect))
    }
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        if !self.active(window) {
            return None;
        }
        let point = self.layout.as_ref()?.local_point(point)?;
        let byte = self.shaped(window).ok()?.byte_for_point(point);
        CanvasTextRange {
            start: byte,
            end: byte,
        }
        .to_utf16(self.model.text())
        .ok()
        .map(|range| range.start)
    }
    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.active(window) {
            return;
        }
        let range = self.model.select(range);
        self.reversed = false;
        self.emit(
            CanvasInputKind::Selection {
                text_revision: self.model.binding().text_revision,
                base_sequence: self.model.base_sequence(),
                range,
            },
            window,
            cx,
        );
        cx.notify();
        window.invalidate_character_coordinates();
    }
    fn accepts_text_input(&self, window: &mut Window, _cx: &mut Context<Self>) -> bool {
        self.active(window) && !self.model.binding().read_only
    }
    fn text_length_utf16(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> Option<usize> {
        self.active(window)
            .then(|| self.model.text().encode_utf16().count())
    }
    fn text_input_editable_range(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.text_length_utf16(window, cx).map(|len| 0..len)
    }
}
