//! Window-root pointer projection over native controls, without another text or selection model.
//!
//! Install exactly once around the complete window root. Capture listeners precede the wrapped
//! tree, so neither physical events being translated nor pointer-restoration events leak into it.
//! Components own region leases and publish layout-derived projections; this module knows no rows.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use gpui_kit::{
    AnyElement, App, Bounds, Context, Element, ElementId, GlobalElementId, Hitbox,
    InspectorElementId, IntoElement, LayoutId, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, PlatformInput, Point, Render, Window, WindowId, div,
    point, px,
};

#[cfg(test)]
mod tests;

/// Registers the final presentation decorator after `gpui_kit::init` and other Root plugins,
/// before creating windows. Keeping this last shields their overlays and decorators while retaining
/// `gpui_base::Root` as the native window root type. Repeated initialization is idempotent.
pub fn init(cx: &mut App) {
    gpui_base::Root::register_plugin::<PointerPresentation>(cx, |_, _| PointerPresentation {
        router: AlignedPointerRouter::default(),
    });
}

/// Finds the window-owned router without a global registry or changing the native root type.
/// Returns None for windows created before registration or windows that do not use Base Root.
pub fn for_window(window: &Window, cx: &App) -> Option<AlignedPointerRouter> {
    let root = window.root::<gpui_base::Root>().flatten()?;
    let presentation = root.read(cx).plugin::<PointerPresentation>()?;
    Some(presentation.read(cx).router.clone())
}

/// Base owns this presentation state and its lifetime, including the complete decorated surface.
struct PointerPresentation {
    router: AlignedPointerRouter,
}

impl Render for PointerPresentation {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

impl gpui_base::RootPlugin for PointerPresentation {
    fn decorate(
        &self,
        surface: AnyElement,
        _: &gpui_base::Root,
        _: &mut Window,
        _: &mut App,
    ) -> impl IntoElement {
        self.router.wrap(surface)
    }
}

/// A component's current native hit geometry and physical-to-native coordinate projection.
struct Projection {
    hitbox: Hitbox,
    native_bounds: Bounds<Pixels>,
    map: Rc<dyn Fn(Point<Pixels>, Option<Point<Pixels>>) -> Point<Pixels>>,
    /// Optional native geometry preparation runs only after the physical callback unwinds.
    prepare: Option<Rc<dyn Fn(Point<Pixels>, Option<Point<Pixels>>, &mut Window, &mut App)>>,
}

/// Owned independently of the router so closing a component releases its captured resources.
#[derive(Default)]
struct Region {
    projection: RefCell<Option<Projection>>,
    epoch: Cell<u64>,
    router: Weak<RouterState>,
}

impl Drop for Region {
    fn drop(&mut self) {
        if let Some(router) = self.router.upgrade() {
            router.request_cancel(self);
        }
    }
}

/// Mapping resource owned by one visible pane. Clones refer to the same registration.
///
/// Owners can request cancellation before withdrawing a pane without borrowing its Window. The
/// root flushes native release against the old frame before painting the replacement. Dropping the
/// final lease also requests cancellation; hiding a window directly requires an explicit flush.
#[derive(Clone)]
pub struct PointerRegion(Rc<Region>);

impl PointerRegion {
    /// Publishes geometry from component prepaint. The hitbox must cover only the native input
    /// surface, excluding buttons, scrollbars and overlays. The mapper uses window coordinates.
    /// Captured drag coordinates are clamped to `native_bounds` to keep another pane out of dispatch.
    pub fn publish(
        &self,
        hitbox: Hitbox,
        native_bounds: Bounds<Pixels>,
        map: impl Fn(Point<Pixels>) -> Point<Pixels> + 'static,
    ) {
        self.publish_drag(hitbox, native_bounds, move |point, _| map(point));
    }

    /// Supplies the physical mouse-down origin during a captured drag, for discontinuous layouts.
    pub fn publish_drag(
        &self,
        hitbox: Hitbox,
        native_bounds: Bounds<Pixels>,
        map: impl Fn(Point<Pixels>, Option<Point<Pixels>>) -> Point<Pixels> + 'static,
    ) {
        *self.0.projection.borrow_mut() = Some(Projection {
            hitbox,
            native_bounds,
            map: Rc::new(map),
            prepare: None,
        });
    }

    /// Prepare the existing native input geometry before forwarding a physical pointer event.
    /// `prepare` may redraw an invisible authority; it must preserve the region's gesture epoch
    /// and text owner. The router then reads the newly published map before native dispatch.
    pub fn prepare_native_input(
        &self,
        prepare: impl Fn(Point<Pixels>, Option<Point<Pixels>>, &mut Window, &mut App) + 'static,
    ) {
        if let Some(projection) = self.0.projection.borrow_mut().as_mut() {
            projection.prepare = Some(Rc::new(prepare));
        }
    }

    /// Revokes an idle region. Returns false during its drag: the caller must use `retire`, since
    /// discarding a native MouseUp can leave the original editor selecting after it is remounted.
    pub fn clear(&self) -> bool {
        if self.0.router.upgrade().is_some_and(|router| {
            router
                .drag
                .borrow()
                .as_ref()
                .is_some_and(|drag| drag.region.ptr_eq(&Rc::downgrade(&self.0)))
        }) {
            return false;
        }
        self.invalidate();
        true
    }

    /// Seal queued projections and record any active native release, without dispatching input or
    /// requiring a Window. Repeated requests are idempotent for the current gesture. The router
    /// retains only the old drag coordinates/button/modifiers and a weak lease, never a source.
    pub fn request_cancel(&self) {
        if let Some(router) = self.0.router.upgrade() {
            router.request_cancel(&self.0);
        }
        self.invalidate();
    }

    /// Invalidates pending events without changing native selection; retirement supplies MouseUp.
    fn invalidate(&self) {
        self.0.epoch.set(self.0.epoch.get().wrapping_add(1));
        self.0.projection.borrow_mut().take();
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Physical,
    Forwarded,
    Restore,
}

/// Keeps the original pane even when the pointer crosses another pane or leaves its bounds.
#[derive(Clone)]
struct Drag {
    region: Weak<Region>,
    epoch: u64,
    button: MouseButton,
    position: Point<Pixels>,
    /// Physical mouse-down origin identifies the native segment of discontinuous presentations.
    origin: Point<Pixels>,
    modifiers: Modifiers,
    cancel_requested: bool,
}

#[derive(Default)]
struct RouterState {
    regions: RefCell<Vec<Weak<Region>>>,
    drag: RefCell<Option<Drag>>,
    phase: Cell<Phase>,
    window: Cell<Option<WindowId>>,
    pending_cancel: RefCell<Vec<Drag>>,
    capture_busy: Cell<bool>,
    capture_deferred: Cell<bool>,
    cancel_deferred: Cell<bool>,
}

impl RouterState {
    /// The source may disappear before the next draw. Copy only the release receipt while the weak
    /// region still identifies this gesture; an outstanding physical tail remains owned separately.
    fn request_cancel(&self, region: &Region) {
        let mut active = self.drag.borrow_mut();
        if let Some(drag) = active
            .as_mut()
            .filter(|drag| std::ptr::eq(drag.region.as_ptr(), region) && !drag.cancel_requested)
        {
            drag.cancel_requested = true;
            self.pending_cancel.borrow_mut().push(drag.clone());
        }
    }
}

/// One persistent router per native window. It wraps the entire root, never just an editor subtree.
#[derive(Clone, Default)]
pub struct AlignedPointerRouter(Rc<RouterState>);

impl AlignedPointerRouter {
    /// Allocates a lease without retaining the component or its projection after the lease dies.
    pub fn register(&self) -> PointerRegion {
        let region = Rc::new(Region {
            projection: RefCell::new(None),
            epoch: Cell::new(0),
            router: Rc::downgrade(&self.0),
        });
        let mut regions = self.0.regions.borrow_mut();
        regions.retain(|region| region.strong_count() != 0);
        regions.push(Rc::downgrade(&region));
        PointerRegion(region)
    }

    /// Releases native gesture state while its input frame still exists, then lets the owner hide
    /// or replace that pane. The caller must keep the old native mount until `after_release` runs.
    /// A physical up arriving afterwards is swallowed as the tail of this cancelled gesture.
    pub fn retire(
        &self,
        region: &PointerRegion,
        window: &mut Window,
        cx: &mut App,
        after_release: impl FnOnce(&mut Window, &mut App) + 'static,
    ) {
        region.request_cancel();
        let router = self.clone();
        window.defer(cx, move |window, cx| {
            router.finish_retirement(window, cx, Box::new(after_release));
        });
    }

    /// Deliver pending native releases while the old rendered frame still owns its listeners.
    /// Call outside entity render/update borrows used by native listeners, before a direct hide.
    /// Returns false if dispatch is busy: the receipt is retained and a deferred retry is scheduled.
    pub fn flush(&self, window: &mut Window, cx: &mut App) -> bool {
        if self.0.pending_cancel.borrow().is_empty() {
            return true;
        }
        if self.0.capture_busy.get() || self.0.phase.get() != Phase::Physical {
            self.defer_flush(window, cx);
            return false;
        }
        let pending = std::mem::take(&mut *self.0.pending_cancel.borrow_mut());
        for drag in pending {
            let original = window.mouse_position();
            self.dispatch(
                PlatformInput::MouseUp(MouseUpEvent {
                    button: drag.button,
                    position: drag.position,
                    modifiers: drag.modifiers,
                    click_count: 1,
                }),
                original,
                drag.modifiers,
                window,
                cx,
            );
        }
        let done = self.0.pending_cancel.borrow().is_empty();
        if !done {
            self.defer_flush(window, cx);
        }
        done
    }

    /// Run a hide/remove callback only after every pending native release has reached the old
    /// frame. Idle release completes synchronously; busy dispatch retains the callback until safe.
    /// Keep the old root mounted until this callback runs, including when rejecting a close request.
    pub fn after_pending_release(
        &self,
        window: &mut Window,
        cx: &mut App,
        after_release: impl FnOnce(&mut Window, &mut App) + 'static,
    ) {
        self.finish_retirement(window, cx, Box::new(after_release));
    }

    fn defer_flush(&self, window: &mut Window, cx: &mut App) {
        if !self.0.cancel_deferred.replace(true) {
            let router = self.clone();
            window.defer(cx, move |window, cx| {
                router.0.cancel_deferred.set(false);
                router.flush(window, cx);
            });
        }
    }

    fn finish_retirement(
        &self,
        window: &mut Window,
        cx: &mut App,
        after_release: Box<dyn FnOnce(&mut Window, &mut App)>,
    ) {
        if self.flush(window, cx) {
            after_release(window, cx);
        } else {
            let router = self.clone();
            window.defer(cx, move |window, cx| {
                router.finish_retirement(window, cx, after_release);
            });
        }
    }

    /// Delegates the root's layout, focus and drawing unchanged, installing capture before paint.
    /// No outside mouse-listener-bearing ancestor may wrap this element. Do not share across windows.
    pub fn wrap(&self, root: impl IntoElement) -> AnyElement {
        PointerRoot {
            router: self.clone(),
            content: root.into_any_element(),
        }
        .into_any_element()
    }

    /// Installed from the outermost paint, before even the root's own interactivity is registered.
    fn install(&self, window: &mut Window) {
        let id = window.window_handle().window_id();
        if let Some(existing) = self.0.window.replace(Some(id)) {
            assert_eq!(
                existing, id,
                "pointer routers belong to exactly one native window"
            );
        }
        let router = self.clone();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            if phase.capture() {
                router.capture(PlatformInput::MouseDown(event.clone()), window, cx);
            }
        });
        let router = self.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
            if phase.capture() {
                router.capture(PlatformInput::MouseMove(event.clone()), window, cx);
            }
        });
        let router = self.clone();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
            if phase.capture() {
                router.capture(PlatformInput::MouseUp(event.clone()), window, cx);
            }
        });
    }

    /// Chooses a pane only at the start of a sequence. A revoked drag remains owned until its up.
    fn target(&self, event: &PlatformInput, window: &Window) -> Option<(Rc<Region>, u64, bool)> {
        if let Some(drag) = self.0.drag.borrow().as_ref() {
            let belongs = match event {
                PlatformInput::MouseMove(event) => event.pressed_button == Some(drag.button),
                PlatformInput::MouseUp(event) => event.button == drag.button,
                _ => false,
            };
            if belongs {
                return drag
                    .region
                    .upgrade()
                    .map(|region| (region, drag.epoch, true));
            }
        }
        // A drag that originated in an ordinary control must not be adopted when it crosses a pane.
        if matches!(event, PlatformInput::MouseMove(event) if event.pressed_button.is_some())
            || matches!(event, PlatformInput::MouseUp(_))
        {
            return None;
        }
        self.0
            .regions
            .borrow()
            .iter()
            .rev()
            .filter_map(Weak::upgrade)
            .find_map(|region| {
                let hovered = region
                    .projection
                    .borrow()
                    .as_ref()
                    .is_some_and(|projection| projection.hitbox.is_hovered(window));
                hovered.then(|| {
                    let epoch = region.epoch.get();
                    (region, epoch, false)
                })
            })
    }

    /// Stops physical input, then defers dispatch until GPUI has restored its listener vector.
    fn capture(&self, mut event: PlatformInput, window: &mut Window, cx: &mut App) {
        match self.0.phase.get() {
            Phase::Forwarded => return,
            Phase::Restore => {
                cx.stop_propagation();
                return;
            }
            Phase::Physical => {}
        }
        if matches!(event, PlatformInput::MouseDown(_)) {
            // A projected source may repaint its invisible authority before forwarding Down.
            // Stop kinetic movement first so that repaint cannot move the clicked source row.
            crate::scroll::cancel_motion(window, cx);
        }
        // GPUI moves rendered_frame.mouse_listeners out during physical capture. Even if a
        // component invokes flush from a later ordinary listener, release must wait for unwind.
        self.0.capture_busy.set(true);
        if !self.0.capture_deferred.replace(true) {
            let router = self.clone();
            window.defer(cx, move |window, cx| {
                router.0.capture_deferred.set(false);
                router.0.capture_busy.set(false);
                router.flush(window, cx);
            });
        }
        let owned = self
            .0
            .drag
            .borrow()
            .as_ref()
            .is_some_and(|drag| match &event {
                PlatformInput::MouseMove(event) => event.pressed_button == Some(drag.button),
                PlatformInput::MouseUp(event) => event.button == drag.button,
                _ => false,
            });
        let target = self.target(&event, window);
        let release = owned && matches!(&event, PlatformInput::MouseUp(_));
        let Some((region, epoch, dragging)) = target else {
            // A closed origin owns the remainder of its gesture; don't activate whatever is behind it.
            if owned {
                cx.stop_propagation();
            }
            if release {
                self.0.drag.borrow_mut().take();
            }
            return;
        };
        if region.epoch.get() != epoch {
            cx.stop_propagation();
            if release {
                self.0.drag.borrow_mut().take();
            }
            return;
        }
        let (position, modifiers) = match &mut event {
            PlatformInput::MouseDown(event) => (&mut event.position, event.modifiers),
            PlatformInput::MouseMove(event) => (&mut event.position, event.modifiers),
            PlatformInput::MouseUp(event) => (&mut event.position, event.modifiers),
            _ => unreachable!("only registered mouse event types enter the router"),
        };
        let original = *position;
        let mapped = {
            let projection = region.projection.borrow();
            let Some(projection) = projection.as_ref() else {
                cx.stop_propagation();
                return;
            };
            let anchor = dragging
                .then(|| self.0.drag.borrow().as_ref().map(|drag| drag.origin))
                .flatten();
            let mapped = (projection.map)(original, anchor);
            if dragging {
                clamp_to_input(mapped, projection.native_bounds)
            } else {
                mapped
            }
        };
        *position = mapped;
        if dragging {
            if let Some(drag) = self.0.drag.borrow_mut().as_mut() {
                drag.position = mapped;
                drag.modifiers = modifiers;
            }
        }
        if let PlatformInput::MouseDown(event) = &event {
            *self.0.drag.borrow_mut() = Some(Drag {
                region: Rc::downgrade(&region),
                epoch,
                button: event.button,
                position: mapped,
                origin: original,
                modifiers,
                cancel_requested: false,
            });
        }
        let region = Rc::downgrade(&region);
        let router = self.clone();
        window.defer(cx, move |window, cx| {
            let valid = region.upgrade().is_some_and(|region| {
                region.epoch.get() == epoch && region.projection.borrow().is_some()
            });
            if !valid {
                return;
            }
            let anchor = dragging
                .then(|| router.0.drag.borrow().as_ref().map(|drag| drag.origin))
                .flatten();
            let prepare = region.upgrade().and_then(|region| {
                region
                    .projection
                    .borrow()
                    .as_ref()
                    .and_then(|projection| projection.prepare.clone())
            });
            if let Some(prepare) = prepare {
                prepare(original, anchor, window, cx);
                let Some(current) = region
                    .upgrade()
                    .filter(|region| region.epoch.get() == epoch)
                else {
                    return;
                };
                let projected = current.projection.borrow();
                let Some(projected) = projected.as_ref() else {
                    return;
                };
                let mut point = (projected.map)(original, anchor);
                if dragging {
                    point = clamp_to_input(point, projected.native_bounds);
                }
                match &mut event {
                    PlatformInput::MouseDown(event) => event.position = point,
                    PlatformInput::MouseMove(event) => event.position = point,
                    PlatformInput::MouseUp(event) => event.position = point,
                    _ => unreachable!("only pointer events use native preparation"),
                }
                if let Some(drag) = router
                    .0
                    .drag
                    .borrow_mut()
                    .as_mut()
                    .filter(|drag| drag.epoch == epoch)
                {
                    drag.position = point;
                }
            }
            router.dispatch(event, original, modifiers, window, cx);
            // Retain ownership until native MouseUp has actually run. A hide request between
            // physical capture and deferred dispatch must still take the retirement path.
            if release {
                let same_drag = router
                    .0
                    .drag
                    .borrow()
                    .as_ref()
                    .is_some_and(|drag| drag.epoch == epoch && drag.region.ptr_eq(&region));
                if same_drag {
                    router.0.drag.borrow_mut().take();
                }
            }
        });
        cx.stop_propagation();
    }

    /// Dispatches after the physical callback unwinds, restoring pointer state before returning.
    fn dispatch(
        &self,
        event: PlatformInput,
        original: Point<Pixels>,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.0.phase.set(Phase::Forwarded);
        window.dispatch_event(event, cx);
        // Root capture blocks restoration before every ordinary listener.
        self.0.phase.set(Phase::Restore);
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: original,
                pressed_button: None,
                modifiers,
            }),
            cx,
        );
        self.0.phase.set(Phase::Physical);
    }
}

/// Keeps a captured gesture over its native source, including release outside the window's panes.
fn clamp_to_input(position: Point<Pixels>, bounds: Bounds<Pixels>) -> Point<Pixels> {
    point(
        position
            .x
            .max(bounds.left())
            .min((bounds.right() - px(0.5)).max(bounds.left())),
        position
            .y
            .max(bounds.top())
            .min((bounds.bottom() - px(0.5)).max(bounds.top())),
    )
}

/// An invisible structural wrapper: the child owns its original layout and accessible subtree.
struct PointerRoot {
    router: AlignedPointerRouter,
    content: AnyElement,
}

impl IntoElement for PointerRoot {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for PointerRoot {
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
        // Native global MouseUp still exists in rendered_frame here, even after the owner's render
        // withdrew its pane. A second flush catches requests made by child prepaint before paint
        // installs the new frame. The surrounding view's Render borrow has already ended.
        self.router.flush(window, cx);
        self.content.prepaint(window, cx);
        self.router.flush(window, cx);
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
        self.router.install(window);
        self.content.paint(window, cx);
    }
}
