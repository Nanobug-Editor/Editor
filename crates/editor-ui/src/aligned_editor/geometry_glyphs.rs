//! Retain native glyphs and validate rows once through GPUI's public cached-view path.
use super::*;
use gpui_base::input::RopeExt as _;
use gpui_kit::{
    Context, Element, ElementId, EntityId, Focusable as _, GlobalElementId, InspectorElementId,
    LayoutId, MouseMoveEvent, ParentElement as _, Refineable as _, Render, StyleRefinement,
    Styled as _, TextStyle, WeakEntity, Window, div, size,
};
use std::cell::RefCell;
use std::rc::Weak;

/// Register outside cached dock panels, whose invalidation otherwise also discards glyph reuse.
pub(super) fn init(cx: &mut App) {
    gpui_base::Root::register_plugin::<Presentation>(cx, |_, _| {
        Presentation(Rc::new(RefCell::new(Vec::new())))
    });
}

/// Weak entries cannot keep a closed source, group or presentation alive.
type Registry = Rc<RefCell<Vec<WeakEntity<GlyphView>>>>;

/// A pane owns this view. Root presentation only borrows it while its real paint is still mounted.
#[derive(Clone)]
pub(super) struct Lease(Entity<GlyphView>);

impl Lease {
    /// The same original source will be supplied after its first verified geometry paint.
    pub fn new(cx: &mut App) -> Self {
        Self(cx.new(|_| GlyphView {
            props: None,
            output: Rc::new(RefCell::new(None)),
            measure: Cell::new(false),
            horizontal: Cell::new(None),
        }))
    }

    /// Unknown native notifications invalidate geometry and exact glyph runs together.
    /// The root prepares one shared batch pass before the real pane validates that same pass.
    pub fn invalidate(&self, cx: &mut App) {
        let view = self.0.read(cx);
        if view.props.is_none() {
            return;
        }
        // Cached dock content may render only during prepaint, after the root has visited its
        // leases. Record the request in the native observer, before this frame begins.
        view.measure.set(true);
        view.output.borrow_mut().take();
        cx.notify(self.0.entity_id());
    }

    /// Wrapped display-map admission needs no hidden glyph lease. Release the mounted cache
    /// and pending validation while keeping this pane's reusable lease entity for later nowrap.
    pub fn release(&self, cx: &mut App) {
        self.0.update(cx, |view, cx| {
            view.measure.set(false);
            view.horizontal.set(None);
            view.output.borrow_mut().take();
            if view.props.take().is_some() {
                cx.notify();
            }
        });
    }

    /// Request this frame's native validation at the existing root cache. Only readonly measured
    /// rows cross back to the pane; native elements retain their original GPUI path and arena.
    /// None leaves initial admission/retired roots with the pane's ordinary native batch path.
    pub fn measure(
        &self,
        horizontal: Option<Pixels>,
        cx: &App,
    ) -> Option<geometry_measurement::Output> {
        if !self
            .0
            .read(cx)
            .props
            .as_ref()
            .is_some_and(|props| props.mounted(cx))
        {
            return None;
        }
        let view = self.0.read(cx);
        view.horizontal.set(horizontal);
        view.output.borrow_mut().take();
        let output = view.output.clone();
        Some(output)
    }

    /// Keep `source` in `group` hot at its verified `viewport`/full native `height`. `owner` is the
    /// real pane; native notifications renew private styles absent from the public signature.
    /// A paired notification ensures that pane follows any rebuild and restores native scrolling.
    pub fn publish(
        &self,
        group: &Entity<GroupState>,
        source: &AlignmentPane,
        owner: EntityId,
        viewport: Bounds<Pixels>,
        height: Pixels,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(root) = window.root::<gpui_base::Root>().flatten() else {
            return;
        };
        let Some(plugin) = root.read(cx).plugin::<Presentation>() else {
            return;
        };
        let registry = plugin.read(cx).0.clone();
        let mount = Rc::new(());
        let style = window.text_style();
        let rem_size = window.rem_size();
        let scale = window.scale_factor();
        let epoch = group.read(cx).epoch;
        let warm_rows = glyph_scope(source.editor.read(cx), window, cx);
        let changed = self.0.update(cx, |view, _| {
            // The actual pane has now admitted this geometry. A stale initial/membership request
            // must not make the following warm-only glyph refresh query the same rows again.
            view.measure.set(false);
            let changed = view.props.as_ref().is_none_or(|old| {
                old.viewport != viewport
                    || old.height != height
                    || old.style != style
                    || old.rem_size != rem_size
                    || old.scale != scale
                    || old.epoch != epoch
                    || old.warm_rows != warm_rows
                    || old.source.editor != source.editor
                    || match (
                        &old.source.native_authority_factory,
                        &source.native_authority_factory,
                    ) {
                        (Some(old), Some(new)) => !Rc::ptr_eq(old, new),
                        (None, None) => false,
                        _ => true,
                    }
            });
            view.props = Some(Props {
                group: group.downgrade(),
                source: source.clone(),
                viewport,
                height,
                style,
                rem_size,
                scale,
                epoch,
                warm_rows,
                mount: Rc::downgrade(&mount),
            });
            changed
        });
        // GPUI preserves native listeners when reusing a cached pane, so this lifetime covers
        // both freshly painted and cached scenes. A removed/retired pane cannot renew it.
        window.on_mouse_event(move |_: &MouseMoveEvent, _, _, _| {
            let _ = &mount;
        });
        let mut entries = registry.borrow_mut();
        entries.retain(|entry| entry.is_upgradable());
        if !entries
            .iter()
            .any(|entry| entry.entity_id() == self.0.entity_id())
        {
            entries.push(self.0.downgrade());
        }
        if changed {
            let lease = self.0.downgrade();
            window.on_next_frame(move |_, cx| {
                if let Some(lease) = lease.upgrade() {
                    cx.notify(lease.entity_id());
                    cx.notify(owner);
                }
            });
        }
    }
}

/// Editable/focused sources retain complete native glyphs for edits, IME and selection. An
/// inactive snapshot prefetches neighboring 64-row blocks around its actual native envelope;
/// this performance-only scope never limits complete geometry validation after native changes.
fn glyph_scope(native: &EditorState, window: &Window, cx: &App) -> Range<usize> {
    let count = native.text().lines_len();
    if native.presentation().is_editable() || native.focus_handle(cx).is_focused(window) {
        return 0..count;
    }
    let visible = native.visible_row_range().unwrap_or(0..count);
    // Keep enough native rows ahead for a fold to jump beyond the physical viewport in one frame.
    let block = 64;
    visible.start.saturating_sub(block) / block * block
        ..(visible.end.div_ceil(block) + 1)
            .saturating_mul(block)
            .min(count)
}

/// Observe actual root leases without exposing any source/pane internals to production callers.
#[cfg(test)]
pub(super) fn live_count(window: &Window, cx: &App) -> usize {
    let Some(root) = window.root::<gpui_base::Root>().flatten() else {
        return 0;
    };
    let Some(plugin) = root.read(cx).plugin::<Presentation>() else {
        return 0;
    };
    plugin
        .read(cx)
        .0
        .borrow()
        .iter()
        .filter_map(WeakEntity::upgrade)
        .filter(|view| {
            view.read(cx)
                .props
                .as_ref()
                .is_some_and(|props| props.mounted(cx))
        })
        .count()
}

/// Only native geometry/style and a weak mounted-scene lease are retained; no text copy exists.
struct Props {
    /// Retirement/epoch checks prevent a retained frame from reviving replaced source membership.
    group: WeakEntity<GroupState>,
    /// Exact original entity and host atomic presentation; there is no shadow editing state.
    source: AlignmentPane,
    /// Last actual pane allocation, including native gutter and surrounding text style.
    viewport: Bounds<Pixels>,
    /// Existing conservative full-source geometry budget, in logical pixels.
    height: Pixels,
    /// Root-level warming must use the same inherited font runs as the source pane.
    style: TextStyle,
    /// Relative font resolution is not represented by TextStyle's length expressions alone.
    rem_size: Pixels,
    /// Renew retained glyphs when the window moves to another display scale.
    scale: f32,
    /// Content membership version for which these native font runs were admitted.
    epoch: u64,
    /// A performance-only native glyph window; original row geometry remains fully validated.
    warm_rows: Range<usize>,
    /// Only the actual pane's native listener keeps this mounted-scene lease alive.
    mount: Weak<()>,
}

impl Props {
    /// Native fallback/retired scenes cannot retain dispatch nodes or glyph work at the root.
    fn mounted(&self, cx: &App) -> bool {
        self.mount.upgrade().is_some()
            && self.group.upgrade().is_some_and(|group| {
                let group = group.read(cx);
                !group.retired
                    && group.epoch == self.epoch
                    && group.effective_status() == AlignmentStatus::Ready
            })
    }
}

/// Warm-only caches never paint. Requested validation paints before the real source authority,
/// which restores the original last_layout, focus/action dispatch and host-owned input delegate.
struct GlyphView {
    /// Unset until this pane has completed a valid geometry paint.
    props: Option<Props>,
    /// One current validation result; it contains only readonly native geometry.
    output: geometry_measurement::Output,
    /// The actual pane requests validation before root prepaint begins.
    measure: Cell<bool>,
    /// Preserve a caller's explicit local horizontal request in all original measurement mounts.
    horizontal: Cell<Option<Pixels>>,
}
impl Render for GlyphView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(props) = &self.props else {
            return div().into_any_element();
        };
        let Some(group) = props.group.upgrade() else {
            return div().into_any_element();
        };
        let group = group.read(cx);
        let Some(index) = group
            .sources
            .iter()
            .position(|source| source.editor == props.source.editor)
        else {
            return div().into_any_element();
        };
        let Some(rows) = group.rows.clone() else {
            return div().into_any_element();
        };
        let measured = group.measurements[index].as_ref();
        let line = props
            .source
            .editor
            .read(cx)
            .line_height()
            .unwrap_or(window.line_height());
        let batches = geometry_measurement::Batches::new(
            props.source.clone(),
            rows,
            index,
            measured.map(|measured| measured.logical_rows.clone()),
            props.height,
            line,
            measured.map_or(px(0.), |measured| measured.native_inset_y),
            self.horizontal.get(),
            self.output.clone(),
        );
        let batches = if self.measure.get() {
            batches
        } else {
            batches.prefetch(props.warm_rows.clone())
        };
        div()
            .relative()
            .size_full()
            .overflow_hidden()
            .opacity(0.)
            .child(div().absolute().top_0().left_0().w_full().h(px(48.)).child(
                native_viewport::NativeViewport::primer(&props.source.editor),
            ))
            .child(batches)
            .into_any_element()
    }
}

/// Base owns registration and teardown together with the native window root.
struct Presentation(Registry);
impl Render for Presentation {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
impl gpui_base::RootPlugin for Presentation {
    fn decorate(
        &self,
        content: AnyElement,
        _: &gpui_base::Root,
        _: &mut Window,
        _: &mut App,
    ) -> impl IntoElement {
        Root {
            content,
            registry: self.0.clone(),
        }
    }
}

/// Prepaint leases first and paint requested validation before the real source subtree.
struct Root {
    /// Original source views, overlays and accessibility tree retain their native ordering.
    content: AnyElement,
    /// Weak pane-owned caches, reused only while their actual painted scenes remain mounted.
    registry: Registry,
}
impl IntoElement for Root {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for Root {
    type RequestLayoutState = ();
    type PrepaintState = Vec<(AnyElement, TextStyle)>;
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
    ) -> Vec<(AnyElement, TextStyle)> {
        let entries = {
            let mut entries = self.registry.borrow_mut();
            entries.retain(|entry| {
                entry.upgrade().is_some_and(|view| {
                    view.read(cx)
                        .props
                        .as_ref()
                        .is_some_and(|props| props.mounted(cx))
                })
            });
            entries.clone()
        };
        let mut validations = Vec::new();
        for entry in entries {
            let Some(view) = entry.upgrade() else {
                continue;
            };
            let Some(props) = &view.read(cx).props else {
                continue;
            };
            let viewport = props.viewport;
            let full_style = props.style.clone();
            let style = props.style.subtract(&Default::default());
            let measure = view.read(cx).measure.get();
            window.with_text_style(Some(style), |window| {
                crate::row_backgrounds::with_geometry_only(window, cx, |window, cx| {
                    let mut element = view
                        .clone()
                        .cached(StyleRefinement::default().size_full())
                        .into_any_element();
                    element.prepaint_as_root(
                        viewport.origin,
                        size(viewport.size.width.into(), viewport.size.height.into()),
                        window,
                        cx,
                    );
                    if measure {
                        validations.push((element, full_style));
                    }
                    // Warm-only elements drop after prepaint; GPUI retains their exact native
                    // font-run range. Requested validation keeps its original element path and
                    // arena, publishes readonly rows, then yields to the real pane's inputs.
                });
            });
            view.read(cx).measure.set(false);
        }
        self.content.prepaint(window, cx);
        validations
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        validations: &mut Vec<(AnyElement, TextStyle)>,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Validation paints at its original root path before the actual pane restores native
        // last_layout, focus, actions and the host input delegate. Warm-only leases never paint.
        for (validation, style) in validations {
            window.with_text_style(Some(style.subtract(&Default::default())), |window| {
                crate::row_backgrounds::with_geometry_only(window, cx, |window, cx| {
                    window.with_content_mask(Some(gpui_kit::ContentMask::default()), |window| {
                        validation.paint(window, cx)
                    });
                });
            });
        }
        self.content.paint(window, cx);
    }
}
