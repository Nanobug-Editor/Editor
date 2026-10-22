//! Scope native input scrollbar paint through Base's public theme, retaining its private metrics.

use super::{VIEWPORT_EDGE_INSET, motion, resolved_scrollbar_styles};
use gpui_base::input::{Editor, EditorState, Textarea, TextareaState};
use gpui_kit::{
    AnyElement, App, Bounds, Element, ElementId, Entity, EntityId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Point, Window,
};

/// Synchronous prepaint depth only; projected panes own separate shared rails outside this scope.
#[derive(Default)]
struct ExternalRails(usize);
impl gpui_kit::Global for ExternalRails {}

/// Prepaint `content` in `window` with embedded rails hidden, restoring all application theme state.
/// The original native input/handle remains mounted; its shared projected owner paints the rails.
pub(crate) fn with_external_scrollbars<R>(
    window: &mut Window,
    cx: &mut App,
    content: impl FnOnce(&mut Window, &mut App) -> R,
) -> R {
    let depth = cx.default_global::<ExternalRails>().0;
    cx.default_global::<ExternalRails>().0 = depth + 1;
    let original = gpui_base::Theme::global(cx).scrollbar.clone();
    gpui_base::Theme::global_mut(cx).scrollbar = original
        .clone()
        .with_mode(gpui_base::ScrollbarMode::Always)
        .with_motion(
            original
                .motion()
                .with_idle(std::time::Duration::ZERO)
                .with_expand(std::time::Duration::ZERO),
        )
        .with_styles(hidden_styles());
    let result = content(window, cx);
    gpui_base::Theme::global_mut(cx).scrollbar = original;
    cx.default_global::<ExternalRails>().0 = depth;
    result
}

/// Zero-width tracks also withdraw embedded rail hit targets; the outer shared rails own dragging.
fn hidden_styles() -> gpui_base::ScrollbarStyles {
    gpui_base::ScrollbarStyles::default()
        .track(|style| style.width(gpui_kit::px(0.)))
        .track_hover(|style| style.width(gpui_kit::px(0.)))
        .track_active(|style| style.width(gpui_kit::px(0.)))
        .thumb(|style| style.width(gpui_kit::px(0.)))
        .thumb_hover(|style| style.width(gpui_kit::px(0.)))
        .thumb_active(|style| style.width(gpui_kit::px(0.)))
}

/// Mount the original editor, with shared thickness motion and no replacement text/scroll model.
pub fn native_editor(editor: &Entity<EditorState>) -> AnyElement {
    let source = editor.clone();
    NativeScrollbars {
        id: editor.entity_id(),
        input: Editor::new(editor).into_any_element(),
        offset: Box::new(move |cx| source.read(cx).scroll_offset()),
    }
    .into_any_element()
}

/// Ordinary multiline fields use the same presentation while retaining Base's auto-grow layout.
pub fn native_textarea(textarea: &Entity<TextareaState>) -> AnyElement {
    let source = textarea.clone();
    NativeScrollbars {
        id: textarea.entity_id(),
        input: Textarea::new(textarea).into_any_element(),
        offset: Box::new(move |cx| source.read(cx).scroll_offset()),
    }
    .into_any_element()
}

/// This transparent wrapper changes only the native scrollbar's width target during prepaint.
struct NativeScrollbars {
    /// The source entity separates concurrent inputs and never aliases their activity leases.
    id: EntityId,
    /// Native input owns its original layout, accessibility and event handlers.
    input: AnyElement,
    /// Read-only access to the existing input offset; no layout or text is estimated locally.
    offset: Box<dyn Fn(&App) -> Point<Pixels>>,
}

impl IntoElement for NativeScrollbars {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for NativeScrollbars {
    type RequestLayoutState = ();
    type PrepaintState = (Option<motion::Visibility>, Bounds<Pixels>);
    fn id(&self) -> Option<ElementId> {
        Some(("input-scrollbar-motion", self.id).into())
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
        (self.input.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let external = cx
            .try_global::<ExternalRails>()
            .is_some_and(|scope| scope.0 > 0);
        let hovered = !external
            && motion::rail_hovered(
                bounds,
                gpui_base::ScrollbarAxis::Both,
                VIEWPORT_EDGE_INSET,
                window,
            );
        let visibility =
            (!external).then(|| motion::active((self.offset)(cx), hovered, window, cx));
        let original = gpui_base::Theme::global(cx).scrollbar;
        let styles = visibility
            .as_ref()
            .map_or_else(hidden_styles, |visibility| {
                resolved_scrollbar_styles(cx, VIEWPORT_EDGE_INSET, visibility.visible)
            });
        let scoped = original.clone().with_styles(styles);
        // The local activity lease already owns the idle hold. Base schedules an additional
        // idle notification even in Always mode; on an editor that wakes native layout consumers
        // and can force a whole-source comparison measurement mid-scroll. Suppress only that
        // redundant wake, retaining Base's width interpolation, drag handling and local hold.
        let scoped = if original.mode() == gpui_base::ScrollbarMode::Always {
            scoped.with_motion(original.motion().with_idle(std::time::Duration::ZERO))
        } else {
            scoped
        };
        // Base's embedded scrollbar has no instance style/factory seam. Its supported theme is
        // scoped synchronously to this input's prepaint, then restored before sibling elements.
        // Both scopes keep the same mode/motion; paint consumes Base's already-resolved geometry.
        gpui_base::Theme::global_mut(cx).scrollbar = scoped;
        self.input.prepaint(window, cx);
        gpui_base::Theme::global_mut(cx).scrollbar = original;
        (visibility, bounds)
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        state: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(visibility) = &state.0 {
            visibility.observe_input(
                state.1,
                gpui_base::ScrollbarAxis::Both,
                VIEWPORT_EDGE_INSET,
                window,
            );
        }
        self.input.paint(window, cx);
    }
}
