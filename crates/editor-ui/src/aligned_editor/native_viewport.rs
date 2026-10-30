//! Apply each clipped copy's public native scroll request before Base chooses its visible rows.
use super::*;
use gpui_kit::InteractiveElement as _;
use gpui_kit::{
    Element, ElementId, GlobalElementId, InspectorElementId, LayoutId, ParentElement as _,
    Styled as _, Window, div, point, size,
};

/// Current-frame native minimum x, or None if the finite probe did not reach the native boundary.
pub(super) type WidthOutput = Rc<Cell<Option<Pixels>>>;

/// Optional per-mount diagnostics show native preparation cost without exposing source contents.
struct PaintTiming(Option<std::time::Instant>, bool);
impl PaintTiming {
    fn new(probe: bool) -> Self {
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        Self(
            ENABLED
                .get_or_init(|| std::env::var_os("NANOBUG_TRACE_DIFF").is_some())
                .then(std::time::Instant::now),
            probe,
        )
    }
}
impl Drop for PaintTiming {
    fn drop(&mut self) {
        if let Some(started) = self.0 {
            let ms = started.elapsed().as_secs_f64() * 1000.;
            if ms >= 1. {
                eprintln!("diff-stage native-prepaint probe={} {ms:.3}ms", self.1);
            }
        }
    }
}

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
    /// Present only on visible copies; the final input window must not admit hidden source rows.
    ranges: Option<native_ranges::NativeRanges>,
    /// Invisible width sampling restores native scrolling through a second original control.
    extent: Option<(WidthOutput, AnyElement)>,
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
            ranges: None,
            extent: None,
        }
    }

    /// Consume deferred caller scrolling before source copies, with no offset override or rails.
    pub fn primer(editor: &Entity<EditorState>) -> Self {
        Self {
            editor: editor.clone(),
            y: None,
            horizontal: None,
            content: crate::scrollbar::native_editor(editor),
            ranges: None,
            extent: None,
        }
    }

    /// After a primer consumes caller scrolling, sample native x clamping and restore it before
    /// visible copies prepaint. `factory` preserves the source's exact native typography and
    /// padding. Cold sources are safe; an existing captured/changed selection is excluded upstream.
    pub fn width_probe(
        editor: &Entity<EditorState>,
        output: WidthOutput,
        factory: Option<&AlignmentNativeFactory>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let mut native = || {
            factory.map_or_else(
                || crate::scrollbar::native_editor(editor),
                |factory| factory(editor, window, cx),
            )
        };
        Self {
            editor: editor.clone(),
            y: None,
            horizontal: None,
            content: div()
                .id("native-width-sample")
                .size_full()
                .child(native())
                .into_any_element(),
            ranges: None,
            extent: Some((
                output,
                div()
                    .id("native-width-restore")
                    .size_full()
                    .child(native())
                    .into_any_element(),
            )),
        }
    }

    /// Retain native carets in `ranges` only when this visible copy lies outside the input window.
    pub fn capture(mut self, ranges: Option<&native_ranges::NativeRanges>) -> Self {
        self.ranges = ranges.cloned();
        self
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
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let _timing = PaintTiming::new(self.extent.is_some());
        if let Some((output, restore)) = &mut self.extent {
            let original = self.editor.read(cx).scroll_offset();
            let request = px(-MAX_LAYOUT_HEIGHT);
            self.editor.update(cx, |editor, cx| {
                editor.set_scroll_offset(point(request, original.y), cx)
            });
            crate::scrollbar::with_external_scrollbars(window, cx, |window, cx| {
                self.content.prepaint(window, cx)
            });
            // Base's scrollbar prepaint applies the current frame's exact native clamp. A source
            // wider than our finite request uses complete validation instead of guessed width.
            let minimum = self.editor.read(cx).scroll_offset().x;
            output.set(
                (minimum > request + px(1.) && minimum <= px(0.) && f32::from(minimum).is_finite())
                    .then_some(minimum),
            );
            self.editor
                .update(cx, |editor, cx| editor.set_scroll_offset(original, cx));
            crate::scrollbar::with_external_scrollbars(window, cx, |window, cx| {
                restore.prepaint_as_root(
                    bounds.origin,
                    size(bounds.size.width.into(), bounds.size.height.into()),
                    window,
                    cx,
                );
            });
            return;
        }
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
        if self.extent.is_some() {
            return;
        }
        self.content.paint(window, cx);
        if let Some(ranges) = &self.ranges {
            ranges.capture(self.editor.read(cx), window);
        }
    }
}
