//! Per-viewport inertial motion follows native vsync with one pending callback and no item scans.
use super::{
    inertia::Inertia,
    router,
    targets::{Metrics, Target},
};
use gpui_base::ScrollbarHandle;
use gpui_kit::{
    AnyElement, App, Element, ElementId, Entity, EntityId, GlobalElementId, Hitbox, HitboxBehavior,
    InspectorElementId, IntoElement, LayoutId, Pixels, Point, RenderOnce, ScrollWheelEvent, Window,
    point, px,
};
use std::{
    rc::{Rc, Weak},
    time::Instant,
};

/// Scroll axes supported by the original viewport.
#[derive(Clone, Copy)]
pub enum ScrollAxis {
    /// Vertical lists and ordinary panels.
    Vertical,
    /// Horizontal strips accept either wheel axis, including Shift-wheel.
    Horizontal,
    /// Editors retain independent horizontal and vertical movement.
    Both,
}

impl ScrollAxis {
    /// Convert a wheel to pixels using `line`; single-axis surfaces retain native axis fallback.
    pub(super) fn delta(self, event: &ScrollWheelEvent, line: Pixels) -> Point<Pixels> {
        let delta = event.delta.pixel_delta(line);
        match self {
            Self::Vertical => point(px(0.), if delta.y != px(0.) { delta.y } else { delta.x }),
            Self::Horizontal => point(if delta.x != px(0.) { delta.x } else { delta.y }, px(0.)),
            Self::Both if event.modifiers.shift && delta.x == px(0.) => point(delta.y, px(0.)),
            Self::Both => delta,
        }
    }
}

/// Add inertial wheel motion to `content`, retaining its native handle and virtual layout.
/// Call `editor_ui::init` before opening Base Root windows to install nested-viewport routing.
#[derive(IntoElement)]
pub struct SmoothScroll {
    /// Caller identity separates multiple scroll surfaces in the same native view.
    id: ElementId,
    /// The original element supplies all layout, input and accessibility behavior.
    content: AnyElement,
    /// Adapter to the existing offset authority.
    target: Target,
    /// Original viewport's supported scroll axes.
    axis: ScrollAxis,
    /// Externally drawn viewports route continuous input through their public handle too.
    projected: bool,
    /// Projected controls supply the native line height outside their hidden authority's style scope.
    line_height: Option<Pixels>,
    /// External surfaces can reserve domain-specific wheel gestures for their original handler.
    wheel_filter: Option<Rc<dyn Fn(&ScrollWheelEvent) -> bool>>,
}

impl SmoothScroll {
    /// Wrap `content` with the public `handle`; `id` must be stable per viewport and `axis`.
    pub fn new<H: ScrollbarHandle + Clone>(
        id: impl Into<ElementId>,
        handle: &H,
        axis: ScrollAxis,
        content: impl IntoElement,
    ) -> Self {
        Self::custom(id, Target::handle(handle), axis, content.into_any_element())
    }

    /// Internal adapters keep native input and projected-comparison ownership intact.
    pub(super) fn custom(
        id: impl Into<ElementId>,
        target: Target,
        axis: ScrollAxis,
        content: AnyElement,
    ) -> Self {
        Self {
            id: id.into(),
            content,
            target,
            axis,
            projected: false,
            line_height: None,
            wheel_filter: None,
        }
    }

    /// The shared comparison viewport owns both axes above its complete-source authority mounts.
    pub(crate) fn projected(mut self, line_height: Pixels) -> Self {
        self.projected = true;
        self.line_height = Some(line_height);
        self
    }

    /// Route both discrete and precise wheels through `handle` for externally drawn content.
    /// `line_height` is the surface's measured logical row height. No native overflow is added;
    /// the existing handle remains responsible for clamping and publishing offset changes.
    pub fn external_scroll(self, line_height: Pixels) -> Self {
        self.projected(line_height)
    }

    /// Leave rejected wheel gestures with the original content handler, including inside nested
    /// scroll surfaces. The predicate must be cheap; it runs once per physical wheel event.
    pub fn filter_wheel(mut self, accept: impl Fn(&ScrollWheelEvent) -> bool + 'static) -> Self {
        self.wheel_filter = Some(Rc::new(accept));
        self
    }
}

impl RenderOnce for SmoothScroll {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(
            ElementId::NamedChild(self.id.into(), "wheel-motion".into()),
            cx,
            |_, _| Motion::default(),
        );
        Surface {
            content: self.content,
            target: self.target,
            axis: self.axis,
            projected: self.projected,
            line_height: self.line_height,
            wheel_filter: self.wheel_filter,
            state,
            owner: window.current_view(),
        }
    }
}

/// Only animation coordinates are retained; every frame reads the authoritative native offset.
#[derive(Default)]
pub(super) struct Motion {
    /// Upstream analytic physics retain velocity across same-direction wheel events.
    inertia: Inertia,
    /// Last requested native position; a different observed value cancels stale motion.
    written: Option<Point<Pixels>>,
    /// Extra invalidations before vsync must not queue duplicate animation callbacks.
    frame_pending: bool,
    /// A native painted-listener lease distinguishes cached reuse from a removed/remounted surface.
    mounted: Weak<()>,
}

impl Motion {
    /// Relinquish pending wheel movement without changing the upstream offset.
    pub(super) fn cancel(&mut self) {
        self.inertia.cancel();
        self.written = None;
    }

    /// Merge `delta` at the physical event time while respecting the observed native `metrics`.
    pub(super) fn push(&mut self, metrics: Metrics, delta: Point<Pixels>, now: Instant) -> bool {
        self.observe(metrics.offset);
        self.inertia.push(metrics, delta, now)
    }

    /// Native reveal, thumb dragging and post-layout clamping take precedence over wheel targets.
    fn observe(&mut self, actual: Point<Pixels>) {
        if self.written.is_some_and(|last| {
            (last.x - actual.x).abs() > px(0.5) || (last.y - actual.y).abs() > px(0.5)
        }) {
            self.cancel();
        }
    }
}

/// Transparent element preserves the exact upstream layout and publishes one clipped input region.
struct Surface {
    /// Native subtree remains the only layout, text and input authority.
    content: AnyElement,
    /// Original scroll state, read once per frame in constant time.
    target: Target,
    /// The viewport's existing scroll directions.
    axis: ScrollAxis,
    /// External handles take precedence over hidden native mounts inside their painted content.
    projected: bool,
    /// Comparison's actual native line height, when outside the source style scope.
    line_height: Option<Pixels>,
    /// Reserved domain gestures must reach the original native content handler.
    wheel_filter: Option<Rc<dyn Fn(&ScrollWheelEvent) -> bool>>,
    /// Per-surface endpoint and last applied animation coordinate.
    state: Entity<Motion>,
    /// Existing rendered view to notify on a physical wheel event.
    owner: EntityId,
}

impl IntoElement for Surface {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for Surface {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;
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
        bounds: gpui_kit::Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Hitbox {
        if let Some(metrics) = (self.target.read)(cx) {
            let now = cx.background_executor().now();
            let next = self.state.update(cx, |state, cx| {
                if cx.reduce_motion()
                    || (state.inertia.active() && state.mounted.upgrade().is_none())
                {
                    state.cancel();
                }
                state.observe(metrics.offset);
                if !state.inertia.active() {
                    return None;
                }
                let next = state.inertia.advance(metrics, now);
                state.written = Some(next);
                Some(next)
            });
            if let Some(next) = next {
                if next != metrics.offset {
                    (self.target.write)(next, window, cx);
                }
            }
            let request = self.state.update(cx, |state, _| {
                if !state.inertia.active() || state.frame_pending {
                    return false;
                }
                state.frame_pending = true;
                true
            });
            if request {
                let state = self.state.downgrade();
                let owner = self.owner;
                // GPUI schedules this on native vsync, including 120/144/240 Hz displays.
                // One weak callback covers both axes; idle and removed surfaces stop demand.
                window.on_next_frame(move |_, cx| {
                    let running = state
                        .update(cx, |state, _| {
                            state.frame_pending = false;
                            if state.mounted.upgrade().is_none() {
                                state.cancel();
                            }
                            state.inertia.active()
                        })
                        .unwrap_or(false);
                    if running {
                        cx.notify(owner);
                    }
                });
            }
        }
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        self.content.prepaint(window, cx);
        hitbox
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: gpui_kit::Bounds<Pixels>,
        _: &mut (),
        hitbox: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(router) = router::for_window(window, cx) {
            let lifetime = Rc::new(());
            self.state
                .update(cx, |state, _| state.mounted = Rc::downgrade(&lifetime));
            let region = super::router::Region {
                _lifetime: lifetime,
                hitbox: hitbox.clone(),
                target: self.target.clone(),
                axis: self.axis,
                projected: self.projected,
                state: self.state.clone(),
                owner: self.owner,
                line_height: self.line_height.unwrap_or(window.line_height()),
                wheel_filter: self.wheel_filter.clone(),
            };
            if self.projected {
                // Only these hidden full-source descendants yield to their shared viewport.
                // Later overlays still register later and keep normal native precedence.
                self.content.paint(window, cx);
                router.listen(region, window);
            } else {
                router.listen(region, window);
                self.content.paint(window, cx);
            }
        } else {
            self.content.paint(window, cx);
        }
    }
}
