//! One OLE registration per native window; each view publishes only its actually painted drop rectangles.
use super::*;
use gpui_kit::{App, AppContext as _, Global, IntoElement as _, Styled as _, Window, WindowId, px};
use platform_windows::drag;
use plugin_runtime::plugin_protocol::{api, ui};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
mod fallback;
mod paint_paths;
mod route;
use route::{Entry, State};

/// Global registration ownership ends with the actual native window, including when all panels hide.
#[derive(Default)]
struct Windows(BTreeMap<WindowId, Rc<Hub>>);
impl Global for Windows {}
struct Hub {
    owner: isize,
    state: Rc<RefCell<State>>,
    _registration: drag::Registration,
    _closed: gpui_kit::Subscription,
}
impl Hub {
    fn acquire(window: &mut Window, cx: &mut App) -> anyhow::Result<Rc<Self>> {
        let handle = window.window_handle();
        let id = handle.window_id();
        if let Some(hub) = cx.default_global::<Windows>().0.get(&id) {
            return Ok(hub.clone());
        }
        let raw = raw_window_handle::HasWindowHandle::window_handle(window)
            .map_err(|error| anyhow::anyhow!("Native transfer window handle: {error:?}"))?;
        let raw_window_handle::RawWindowHandle::Win32(raw) = raw.as_raw() else {
            anyhow::bail!("OLE needs an owned Windows window");
        };
        let fallback = fallback::start(handle, cx);
        let state = Rc::new(RefCell::new(State::new(fallback)));
        let route = state.clone();
        let registration =
            drag::Registration::new(raw.hwnd.get(), move |event| route.borrow_mut().route(event))?;
        let closed = cx.on_window_closed(move |cx, closed| {
            if closed == id {
                cx.global_mut::<Windows>().0.remove(&id);
            }
        });
        let hub = Rc::new(Self {
            owner: raw.hwnd.get(),
            state,
            _registration: registration,
            _closed: closed,
        });
        cx.global_mut::<Windows>().0.insert(id, hub.clone());
        Ok(hub)
    }
}

/// Exact source and ancestor ownership, independent of drawing-only UI revisions and coordinates.
#[derive(Clone, PartialEq)]
struct Binding {
    source: Option<api::DocumentVersion>,
    file: Option<api::FileVersion>,
    kind: std::mem::Discriminant<ui::Kind>,
    target: plugin_runtime::plugin_protocol::transfer::DropTarget,
    drag: Option<plugin_runtime::plugin_protocol::transfer::DragSource>,
    overlay: Option<u64>,
    /// Drawing-only movement retains authority; semantic target declaration changes revoke it.
    scene: Option<(u64, Vec<SceneTarget>)>,
}
/// Semantic source/receiver declarations stay live across drawing-only movement, never replacement.
#[derive(Clone, PartialEq)]
struct SceneTarget {
    id: String,
    epoch: u64,
    drop: Option<plugin_runtime::plugin_protocol::transfer::DropTarget>,
    drag: Option<plugin_runtime::plugin_protocol::transfer::DragSource>,
    visible: bool,
}
struct Owner {
    sink: Sink,
    document: std::sync::Arc<ui::Document>,
    guards: BTreeMap<String, (Binding, Arc<AtomicBool>)>,
    /// One composed view's native paint order and stable ordinary-tree foreground paths.
    order: u64,
    paths: BTreeMap<String, Vec<usize>>,
}

/// Scope cleanup revokes pending native reads even when no future paint will run.
pub(super) struct Scope {
    id: u64,
    hub: Rc<Hub>,
}
impl Scope {
    /// Native source guards are issued only for an active declared source in this same view scope.
    pub(super) fn source_guard(
        &self,
        address: &Address,
        expected: &plugin_runtime::plugin_protocol::transfer::DragSource,
    ) -> Option<(Sink, Arc<AtomicBool>, isize)> {
        let state = self.hub.state.borrow();
        let owner = state.owners.get(&self.id)?;
        let (binding, alive) = owner.guards.get(&address.node)?;
        let declared = match &address.canvas {
            Some(anchor) => binding.scene.as_ref().is_some_and(|(identity, targets)| {
                *identity == anchor.identity
                    && targets.iter().any(|target| {
                        target.id == anchor.target.id
                            && target.epoch == anchor.target.epoch
                            && target.visible
                            && target.drag.as_ref() == Some(expected)
                    })
            }),
            None => binding.drag.as_ref() == Some(expected),
        };
        (declared && alive.load(Ordering::Acquire))
            .then(|| (owner.sink.clone(), alive.clone(), self.hub.owner))
    }
    pub(super) fn attach(
        sink: Sink,
        window: &mut Window,
        cx: &mut App,
    ) -> anyhow::Result<Rc<Self>> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let hub = Hub::acquire(window, cx)?;
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        hub.state.borrow_mut().owners.insert(
            id,
            Owner {
                sink,
                document: Arc::new(ui::Document::new(ui::Node::text("empty", ""))),
                guards: BTreeMap::new(),
                order: 0,
                paths: BTreeMap::new(),
            },
        );
        Ok(Rc::new(Self { id, hub }))
    }
    pub(super) fn reconcile(
        &self,
        document: &ui::Document,
        current: bool,
        overlay: impl Fn(&str) -> Option<u64>,
    ) {
        let mut state = self.hub.state.borrow_mut();
        let Some(owner) = state.owners.get_mut(&self.id) else {
            return;
        };
        let mut next = BTreeMap::new();
        if current {
            let mut visit = |node: &ui::Node| {
                if document.active_node(&node.id).is_none() || node.disabled {
                    return;
                }
                let scene = match &node.kind {
                    ui::Kind::Canvas(canvas) => canvas.scene.as_ref().map(|scene| {
                        (
                            scene.identity,
                            scene
                                .targets
                                .iter()
                                .filter(|target| {
                                    target.drop_target.is_some() || target.drag_source.is_some()
                                })
                                .map(|target| SceneTarget {
                                    id: target.id.clone(),
                                    epoch: target.epoch,
                                    drop: target.drop_target.clone(),
                                    drag: target.drag_source.clone(),
                                    visible: target.pointer_events
                                        && scene
                                            .group_placement(target.space.as_deref())
                                            .is_some_and(|group| group.visible),
                                })
                                .collect::<Vec<_>>(),
                        )
                    }),
                    _ => None,
                };
                let overlay = overlay(&node.id);
                let target = node
                    .drop_target
                    .clone()
                    .or_else(|| {
                        node.drag_source.as_ref().map(|_| {
                            plugin_runtime::plugin_protocol::transfer::DropTarget {
                                preferred: Vec::new(),
                            }
                        })
                    })
                    .or_else(|| {
                        scene.as_ref().and_then(|(_, targets)| {
                            let preferred = targets
                                .iter()
                                .filter(|target| target.visible)
                                .filter_map(|target| target.drop.as_ref())
                                .flat_map(|target| target.preferred.iter().cloned())
                                .collect::<std::collections::BTreeSet<_>>()
                                .into_iter()
                                .collect::<Vec<_>>();
                            (!targets.is_empty()).then_some(
                                plugin_runtime::plugin_protocol::transfer::DropTarget { preferred },
                            )
                        })
                    })
                    .or_else(|| {
                        overlay.map(|_| plugin_runtime::plugin_protocol::transfer::DropTarget {
                            preferred: Vec::new(),
                        })
                    });
                let Some(target) = target else {
                    return;
                };
                let binding = Binding {
                    source: document.source.clone(),
                    file: document.file.clone(),
                    kind: std::mem::discriminant(&node.kind),
                    target,
                    drag: node.drag_source.clone(),
                    overlay,
                    scene,
                };
                let alive = owner
                    .guards
                    .get(&node.id)
                    .filter(|(previous, _)| previous == &binding)
                    .map_or_else(
                        || Arc::new(AtomicBool::new(true)),
                        |(_, alive)| alive.clone(),
                    );
                next.insert(node.id.clone(), (binding, alive));
            };
            document.root.visit(&mut visit);
            if let Some(dialog) = &document.dialog {
                dialog.content.visit(&mut visit);
            }
        }
        for (node, (_, alive)) in &owner.guards {
            if next
                .get(node)
                .is_none_or(|(_, next)| !Arc::ptr_eq(next, alive))
            {
                alive.store(false, Ordering::Release);
            }
        }
        owner.guards = next;
        if owner.document.as_ref() != document {
            owner.paths = paint_paths::collect(document);
            owner.document = Arc::new(document.clone());
        }
        state
            .entries
            .retain(|(scope, _), entry| *scope != self.id || entry.alive.load(Ordering::Acquire));
        state.cancel_dead();
    }
    pub(super) fn begin_frame(&self) {
        let mut state = self.hub.state.borrow_mut();
        state.entries.retain(|(scope, _), _| *scope != self.id);
        let order = state.sequence;
        state.sequence = state.sequence.saturating_add(1);
        if let Some(owner) = state.owners.get_mut(&self.id) {
            owner.order = order;
        }
    }
    pub(super) fn probe(
        self: &Rc<Self>,
        node: &ui::Node,
        scale: f32,
        canvas: Option<gpui_kit::Entity<crate::ui::plugin::canvas::CanvasView>>,
    ) -> Option<gpui_kit::AnyElement> {
        if !self
            .hub
            .state
            .borrow()
            .owners
            .get(&self.id)?
            .guards
            .contains_key(&node.id)
        {
            return None;
        }
        let scope = self.clone();
        let node = node.id.clone();
        let native_drop = self
            .hub
            .state
            .borrow()
            .owners
            .get(&self.id)?
            .document
            .active_node(&node)?
            .drop_target
            .is_some();
        Some(
            gpui_kit::canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| {
                    let frame = canvas
                        .as_ref()
                        .and_then(|canvas| canvas.read(cx).transfer_frame());
                    let clipped = bounds.intersect(&window.content_mask().bounds);
                    let mut state = scope.hub.state.borrow_mut();
                    let Some(owner) = state.owners.get(&scope.id) else {
                        return;
                    };
                    let Some((binding, alive)) = owner.guards.get(&node) else {
                        return;
                    };
                    if !alive.load(Ordering::Acquire)
                        || clipped.size.width <= px(0.)
                        || clipped.size.height <= px(0.)
                    {
                        return;
                    }
                    let entry = Entry {
                        address: Address {
                            node: node.clone(),
                            ui_revision: frame
                                .as_ref()
                                .map_or(owner.document.revision, |frame| frame.revision),
                            source: owner.document.source.clone(),
                            file: owner.document.file.clone(),
                            canvas: None,
                        },
                        bounds: [
                            clipped.origin.x / px(1.),
                            clipped.origin.y / px(1.),
                            clipped.size.width / px(1.),
                            clipped.size.height / px(1.),
                        ],
                        origin: [bounds.origin.x / px(1.), bounds.origin.y / px(1.)],
                        scale,
                        dpi: window.scale_factor(),
                        preferred: binding.target.preferred.clone(),
                        alive: alive.clone(),
                        sink: owner.sink.clone(),
                        order: owner.order,
                        path: owner.paths.get(&node).cloned().unwrap_or_default(),
                        frame,
                        native_drop,
                    };
                    let key = (scope.id, node.clone());
                    if state.entries.len() < 4096 || state.entries.contains_key(&key) {
                        state.entries.insert(key, entry);
                    }
                },
            )
            .absolute()
            .size_full()
            .into_any_element(),
        )
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        let mut state = self.hub.state.borrow_mut();
        if let Some(owner) = state.owners.remove(&self.id) {
            for (_, (_, alive)) in owner.guards {
                alive.store(false, Ordering::Release);
            }
        }
        state.entries.retain(|(scope, _), _| *scope != self.id);
        state.cancel_dead();
    }
}
