//! A bounded activity lease selects width targets; Base owns interpolation and pointer capture.

use super::resolved_scrollbar_styles;
use gpui_base::{ScrollbarAxis, ScrollbarHandle, ScrollbarMode};
use gpui_kit::{
    AnyElement, App, Bounds, Element, ElementId, Entity, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseExitEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, Position, Style, Task, Window, px, relative,
};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

/// All scroll surfaces retain a revealed thumb for the same interval after movement.
pub(super) const IDLE: Duration = Duration::from_secs(2);
/// Normal, hover, drag and hidden width targets share Base's reversible easing.
pub(super) const TRANSITION: Duration = Duration::from_millis(180);

/// Only offsets and one finite wake-up are retained; content and input stay with the scroll owner.
struct Activity {
    /// Last observed owner offset, including keyboard and programmatic scrolling.
    offset: Point<Pixels>,
    /// Activity time is independent of palette repaints and adopts the current idle policy.
    last_activity: Instant,
    /// Hover holds visibility without scheduling a repeating idle timer.
    hovered: bool,
    /// One weakly captured timer is enough even during continuous scrolling.
    wake: Option<Task<()>>,
    /// Presentation hold only; Base continues to own pointer capture and offset changes.
    dragging: bool,
    /// A window exit must release cosmetic hover even while its last position is inside a rail.
    pointer_inside: bool,
}

/// A weakly observed presentation lease, with no authority to edit content or move its viewport.
/// Its type is public only because native Element prepaint state crosses the component boundary.
pub struct Visibility {
    /// Current normal thumb target; hover and captured dragging remain Base style states.
    pub(super) visible: bool,
    /// The keyed element owns all cosmetic lifetime and timer resources.
    lease: Entity<Rc<RefCell<Activity>>>,
}

impl Visibility {
    /// Observe original rail capture and window entry/exit without consuming native input.
    /// `bounds`, `axis` and `inset` identify the fixed strip; only a prior rail press observes release.
    pub fn observe_input(
        &self,
        bounds: Bounds<Pixels>,
        axis: ScrollbarAxis,
        inset: Pixels,
        window: &mut Window,
    ) {
        let entered = self.lease.downgrade();
        window.on_mouse_event(move |_: &MouseMoveEvent, phase, _, cx| {
            if phase.capture() {
                let _ = entered.update(cx, |state, cx| {
                    let mut state = state.borrow_mut();
                    if !state.pointer_inside {
                        state.pointer_inside = true;
                        cx.notify();
                    }
                });
            }
        });
        let exited = self.lease.downgrade();
        window.on_mouse_event(move |_: &MouseExitEvent, phase, _, cx| {
            if phase.capture() {
                let _ = exited.update(cx, |state, cx| {
                    let mut state = state.borrow_mut();
                    state.pointer_inside = false;
                    cx.notify();
                });
            }
        });
        let down = self.lease.downgrade();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, _, cx| {
            if phase.capture()
                && event.button == MouseButton::Left
                && rail_contains(bounds, axis, inset, event.position)
            {
                let _ = down.update(cx, |state, cx| {
                    let mut state = state.borrow_mut();
                    state.dragging = true;
                    cx.notify();
                });
            }
        });
        let up = self.lease.downgrade();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
            if phase.capture() && event.button == MouseButton::Left {
                let _ = up.update(cx, |state, cx| {
                    let mut state = state.borrow_mut();
                    if state.dragging {
                        state.dragging = false;
                        state.last_activity = Instant::now();
                        cx.notify();
                    }
                });
            }
        });
    }
}

/// Return the width target for `offset` and wake its current view when the idle lease expires.
/// The keyed element owns the lease; dropping it cancels its timer without retaining the viewport.
pub(super) fn active(
    offset: Point<Pixels>,
    hovered: bool,
    window: &mut Window,
    cx: &mut App,
) -> Visibility {
    let idle = gpui_base::Theme::global(cx).scrollbar.motion().idle();
    let lease = window.use_state(cx, |_, _| {
        Rc::new(RefCell::new(Activity {
            offset,
            last_activity: Instant::now(),
            hovered: false,
            wake: None,
            dragging: false,
            pointer_inside: true,
        }))
    });
    let state = lease.read(cx).clone();
    let now = Instant::now();
    let mut activity = state.borrow_mut();
    let hovered = hovered && activity.pointer_inside;
    if activity.offset != offset || activity.hovered != hovered {
        activity.offset = offset;
        activity.last_activity = now;
        activity.hovered = hovered;
    }
    let elapsed = now.saturating_duration_since(activity.last_activity);
    let visible = activity.dragging || hovered || elapsed < idle;
    if !hovered && !activity.dragging && visible && activity.wake.is_none() {
        let delay = idle.saturating_sub(elapsed);
        let weak = Rc::downgrade(&state);
        let changed = lease.downgrade();
        activity.wake = Some(window.spawn(cx, async move |cx| {
            cx.background_executor().timer(delay).await;
            if let Some(state) = weak.upgrade() {
                state.borrow_mut().wake = None;
                // Notify the keyed lease so Base's registered observer invalidates the actual
                // enclosing view even when it lives under a cached dock presentation.
                let _ = changed.update(cx, |_, cx| cx.notify());
            }
        }));
    }
    Visibility { visible, lease }
}

/// Reveal anywhere along a fixed `axis` rail in `bounds`, including a fully contracted thumb.
/// `inset` is the established edge clearance; pointer ownership and dragging still belong to Base.
pub(super) fn rail_hovered(
    bounds: Bounds<Pixels>,
    axis: ScrollbarAxis,
    inset: Pixels,
    window: &Window,
) -> bool {
    rail_contains(bounds, axis, inset, window.mouse_position())
}

/// Test the existing rail strip against a physical `at` position without taking input ownership.
fn rail_contains(
    bounds: Bounds<Pixels>,
    axis: ScrollbarAxis,
    inset: Pixels,
    at: Point<Pixels>,
) -> bool {
    bounds.contains(&at)
        && ((axis != ScrollbarAxis::Horizontal && at.x >= bounds.right() - inset - px(8.))
            || (axis != ScrollbarAxis::Vertical && at.y >= bounds.bottom() - inset - px(8.)))
}

/// The product scrollbar changes paint width, leaving Base's geometry, drag and wheel behavior intact.
pub struct Scrollbar {
    /// Base element is consumed once during native prepaint after current metrics are available.
    native: Option<gpui_base::Scrollbar>,
    /// Read the original handle late, after a preceding canvas or list has measured its viewport.
    offset: Rc<dyn Fn() -> Point<Pixels>>,
    /// Original handle bounds are used for lists; canvases choose the measured overlay layout.
    viewport: Option<Rc<dyn Fn() -> Bounds<Pixels>>>,
    /// Only the matching rail can extend this element's activity lease.
    axis: ScrollbarAxis,
    /// Fixed clearance is independent of animated width, preventing content reflow.
    inset: Pixels,
    /// Per-call-site identity preserves Base's animation through host or guest rerenders.
    id: ElementId,
}

impl Scrollbar {
    /// Bind the original `handle`; `inset` clears the edge and `layout` selects canvas bounds.
    #[track_caller]
    pub(super) fn new<H: ScrollbarHandle + Clone>(handle: &H, inset: Pixels, layout: bool) -> Self {
        let caller = std::panic::Location::caller();
        let offset = handle.clone();
        let viewport = handle.clone();
        let native = gpui_base::Scrollbar::vertical(handle).mode(ScrollbarMode::Always);
        Self {
            native: Some(if layout {
                native.viewport_from_layout()
            } else {
                native
            }),
            offset: Rc::new(move || offset.offset()),
            viewport: (!layout).then(|| {
                Rc::new(move || viewport.viewport_bounds()) as Rc<dyn Fn() -> Bounds<Pixels>>
            }),
            axis: ScrollbarAxis::Vertical,
            inset,
            id: format!(
                "scrollbar-motion:{}:{}:{}",
                caller.file(),
                caller.line(),
                caller.column()
            )
            .into(),
        }
    }

    /// Horizontal scrollbars animate their thickness using the same timing and fixed edge.
    pub fn axis(mut self, axis: ScrollbarAxis) -> Self {
        self.native = self.native.take().map(|native| native.axis(axis));
        self.axis = axis;
        self
    }
}

impl IntoElement for Scrollbar {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for Scrollbar {
    type RequestLayoutState = ();
    type PrepaintState = (AnyElement, Visibility, Bounds<Pixels>);

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
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
        let mut style = Style::default();
        style.position = Position::Absolute;
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        // Auto insets would place a trailing overlay below a full-height canvas sibling.
        style.inset.top = px(0.).into();
        style.inset.left = px(0.).into();
        (window.request_layout(style, [], cx), ())
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
        let viewport = self.viewport.as_ref().map_or(bounds, |read| read());
        let hovered = rail_hovered(viewport, self.axis, self.inset, window);
        let visibility = active((self.offset)(), hovered, window, cx);
        let styles = resolved_scrollbar_styles(cx, self.inset, visibility.visible);
        let mut native = self
            .native
            .take()
            .expect("one native prepaint per scrollbar")
            .styles(|_| styles)
            .into_any_element();
        native.prepaint_as_root(bounds.origin, bounds.size.into(), window, cx);
        (native, visibility, viewport)
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
        state
            .1
            .observe_input(state.2, self.axis, self.inset, window);
        state.0.paint(window, cx);
    }
}
