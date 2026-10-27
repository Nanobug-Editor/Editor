//! One window-owned wheel router chooses the innermost live viewport after native capture observers.
use super::{
    smooth::{Motion, ScrollAxis},
    targets::Target,
};
use gpui_kit::{
    AnyElement, App, Bounds, Context, Element, ElementId, Entity, EntityId, GlobalElementId,
    Hitbox, InspectorElementId, IntoElement, KeyDownEvent, LayoutId, MouseDownEvent,
    MouseExitEvent, Pixels, Render, ScrollEvent, WeakEntity, Window, div,
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

/// Install before creating Root windows; registration is idempotent in Base's plugin registry.
pub(crate) fn init(cx: &mut App) {
    gpui_base::Root::register_plugin::<Presentation>(cx, |_, _| Presentation(Router::default()));
}

/// Retrieve only this native window's presentation router.
pub(super) fn for_window(window: &Window, cx: &App) -> Option<Router> {
    let root = window.root::<gpui_base::Root>().flatten()?;
    let plugin = root.read(cx).plugin::<Presentation>()?;
    Some(plugin.read(cx).0.clone())
}

/// A native listener retains this scope exactly as long as its painted or cached subtree.
#[derive(Clone)]
pub(super) struct Region {
    /// Native cached event listeners keep the mount alive without a background task.
    pub _lifetime: Rc<()>,
    /// Original clipped hitbox preserves occlusion and nested scroll precedence.
    pub hitbox: Hitbox,
    /// Reads and writes the viewport's original upstream state.
    pub target: Target,
    /// Native scroll axes and single-axis wheel fallback.
    pub axis: ScrollAxis,
    /// Externally drawn content applies precise deltas through its handle without native overflow.
    pub projected: bool,
    /// Stable keyed coordinates used only while easing a wheel gesture.
    pub state: Entity<Motion>,
    /// Only this rendered native view is invalidated when its wheel target changes.
    pub owner: EntityId,
    /// Style-resolved logical line height for discrete wheel input.
    pub line_height: Pixels,
    /// The innermost surface may reserve a wheel gesture for its original content handler.
    pub wheel_filter: Option<Rc<dyn Fn(&ScrollEvent) -> bool>>,
}

/// Only the currently painted surfaces are inspected when an actual input event arrives.
#[derive(Clone, Default)]
pub(super) struct Router(Rc<Routes>);

/// Candidates exist only during one physical event; the active animation is held weakly.
#[derive(Default)]
struct Routes {
    /// Reused allocation, populated by native capture order even for cached subviews.
    candidates: RefCell<Vec<Weak<Region>>>,
    /// Closing a viewport cannot be prevented by the window's input router.
    active: RefCell<Option<WeakEntity<Motion>>>,
}

impl Router {
    /// Record input through native listeners, which GPUI retains when reusing cached paint.
    /// Parents listen before children; projected sources listen after their hidden authorities.
    pub fn listen(&self, region: Region, window: &mut Window) {
        let router = self.clone();
        let region = Rc::new(region);
        window.on_mouse_event(move |_: &ScrollEvent, phase, window, _| {
            if phase.capture() && region.hitbox.should_handle_scroll(window) {
                router
                    .0
                    .candidates
                    .borrow_mut()
                    .push(Rc::downgrade(&region));
            }
        });
    }

    /// Cancel inertia for keyboard input, pointer dragging or the precise device's own momentum.
    pub(super) fn cancel(&self, except: Option<EntityId>, cx: &mut App) {
        let mut active = self.0.active.borrow_mut();
        if active
            .as_ref()
            .is_some_and(|active| Some(active.entity_id()) == except)
        {
            return;
        }
        if let Some(active) = active.take() {
            let _ = active.update(cx, |state, _| state.cancel());
        }
    }

    /// Native capture observers see the physical event; smooth wheel input replaces only bubbling.
    fn wheel(&self, event: &ScrollEvent, window: &mut Window, cx: &mut App) {
        let regions = self.0.candidates.borrow();
        if event.modifiers.control || event.modifiers.alt || event.modifiers.platform {
            drop(regions);
            self.cancel(None, cx);
            return;
        }
        if regions
            .iter()
            .rev()
            .filter_map(Weak::upgrade)
            .find(|region| region.hitbox.should_handle_scroll(window))
            .is_some_and(|region| {
                region
                    .wheel_filter
                    .as_ref()
                    .is_some_and(|accept| !accept(event))
            })
        {
            drop(regions);
            self.cancel(None, cx);
            return;
        }
        if event.delta.precise() || cx.reduce_motion() {
            // A projected comparison has no native vertical overflow. Apply continuous input
            // directly to its shared handle; ordinary native viewports receive their own event.
            if let Some(region) = regions
                .iter()
                .rev()
                .filter_map(Weak::upgrade)
                .find(|region| region.hitbox.should_handle_scroll(window))
                .filter(|region| region.projected)
            {
                if let Some(metrics) = (region.target.read)(cx) {
                    let next = metrics
                        .clamp(metrics.offset + region.axis.delta(event, region.line_height));
                    if next != metrics.offset {
                        (region.target.write)(next, window, cx);
                        cx.notify(region.owner);
                        cx.stop_propagation();
                    }
                }
            }
            drop(regions);
            self.cancel(None, cx);
            return;
        }
        for region in regions
            .iter()
            .rev()
            .filter_map(Weak::upgrade)
            .filter(|region| region.hitbox.should_handle_scroll(window))
        {
            let Some(metrics) = (region.target.read)(cx) else {
                continue;
            };
            let delta = region
                .axis
                .delta(event, metrics.line_height.unwrap_or(region.line_height));
            if region.state.update(cx, |state, cx| {
                state.push(metrics, delta, cx.background_executor().now())
            }) {
                let selected = region.state.entity_id();
                let active = region.state.downgrade();
                cx.notify(region.owner);
                drop(regions);
                self.cancel(Some(selected), cx);
                *self.0.active.borrow_mut() = Some(active);
                window.prevent_default();
                cx.stop_propagation();
                return;
            }
        }
        drop(regions);
        self.cancel(None, cx);
    }
}

/// Base owns the router lifecycle together with its native window root.
struct Presentation(Router);
impl Render for Presentation {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
impl gpui_base::RootPlugin for Presentation {
    fn decorate(
        &self,
        surface: AnyElement,
        _: &gpui_base::Root,
        _: &mut Window,
        _: &mut App,
    ) -> impl IntoElement {
        Root {
            content: surface,
            router: self.0.clone(),
        }
    }
}

/// The original root's layout and accessibility tree pass through unchanged.
struct Root {
    /// Original Base root content and overlays retain their native layout and input tree.
    content: AnyElement,
    /// A single registry is reused by every painted scroll surface in this window.
    router: Router,
}
impl IntoElement for Root {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for Root {
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
        self.content.prepaint(window, cx);
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
        let router = self.router.clone();
        window.on_mouse_event(move |_: &ScrollEvent, phase, _, _| {
            if phase.capture() {
                router.0.candidates.borrow_mut().clear();
            }
        });
        let router = self.router.clone();
        window.on_mouse_event(move |_: &MouseDownEvent, phase, _, cx| {
            if phase.capture() {
                router.cancel(None, cx);
            }
        });
        let router = self.router.clone();
        window.on_mouse_event(move |_: &MouseExitEvent, phase, _, cx| {
            if phase.capture() {
                router.cancel(None, cx);
            }
        });
        let router = self.router.clone();
        window.on_key_event(move |_: &KeyDownEvent, phase, _, cx| {
            if phase.capture() {
                router.cancel(None, cx);
            }
        });
        self.content.paint(window, cx);
        // Bubble listeners run in reverse paint order: choose a target before native offset writes.
        let router = self.router.clone();
        window.on_mouse_event(move |event: &ScrollEvent, phase, window, cx| {
            if phase.bubble() {
                router.wheel(event, window, cx);
                router.0.candidates.borrow_mut().clear();
            }
        });
    }
}
