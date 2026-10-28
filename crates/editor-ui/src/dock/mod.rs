//! Project-owned presentation for the GPUI Base dock layout engine.
//!
//! GPUI Base owns the lasting tree, constraints and native panel identities. This module owns
//! local appearance, placement and regional reveal policy; each panel retains its split slot.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::Arc,
};

use crate::dock::panel::DragPanel;
use crate::dock::panel::DropIndicator;
use crate::dock::panel::PanelHandle;
use crate::dock::panel::TabGroupContext;
use crate::dock::panel::TabGroupRenderer;
use crate::scroll::ScrollableElement as _;
use crate::theme::ActiveTheme as _;
use crate::theme::component_styles;
use gpui_base::dock::{
    DockArea, DockAreaRenderer, DockContext, DockEvent, DockPlacement, InsertTarget, NodeId,
    PanelView,
};
use gpui_base::h_flex;
use gpui_base::{ElementExt as _, HandleEdge, Placement, TestSupportExt as _, resize_handle};
use gpui_kit::{
    AnyElement, AnyView, App, AppContext as _, Bounds, Context, Div, Empty, Entity, FocusHandle,
    InteractiveElement as _, IntoElement, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels,
    Render, Stateful, StatefulInteractiveElement as _, StyleRefinement, Styled as _, Subscription,
    WeakEntity, Window, canvas, div, prelude::FluentBuilder as _, px, size,
};
use plugin_schema::ThemeComponent;

/// Transient paint allocations only; Base retains the sole persistent layout tree.
#[derive(Clone, Default)]
struct PaintedFrames {
    /// Tab slots supply source dimensions and pointer hit targets.
    panels: Rc<RefCell<HashMap<NodeId, Bounds<Pixels>>>>,
    /// Four native regions bound post-removal projection even when a child overflows its slot.
    regions: Rc<RefCell<HashMap<DockPlacement, Bounds<Pixels>>>>,
}

/// A local Dock appearance with independent panels and a shared title height.
#[derive(Clone)]
pub struct LocalDock {
    title_height: f32,
    /// Shared paint geometry lets the root prepare editor-local widths before Base lays out a new frame.
    frames: PaintedFrames,
    /// All users of this surface share one reversible drag transaction, including plugin panels.
    dragging: dragging::LiveDrag,
    /// Only temporarily hidden slot preferences live outside Base's visible size projection.
    hidden_sizes: hidden_sizes::HiddenSizes,
    /// Correct visible-neighbor resizing through Base's public size edit when slots are hidden.
    split_resize: split_resize::SplitResize,
}

impl LocalDock {
    /// Create one surface's state and chrome with a shared logical-pixel title height.
    pub fn new(title_height: f32) -> Self {
        Self {
            title_height,
            frames: Default::default(),
            dragging: Default::default(),
            hidden_sizes: Default::default(),
            split_resize: Default::default(),
        }
    }

    /// Mark `panel` as the elastic center, with at most one visible region on each of its sides.
    /// A first-level title may carry its whole combination to relocate that occupied side.
    /// Other free edges and concrete tool targets move one panel; nested children never promote peers.
    /// The anchor absorbs width changes. Surfaces without an anchor keep ordinary edge placement.
    pub fn with_elastic_panel(self, panel: gpui_base::dock::PanelId) -> Self {
        self.dragging.set_anchor(panel);
        self
    }

    /// Creates a behavior-backed dock area using this project's own renderer.
    pub fn create_area(
        &self,
        id: impl Into<gpui_kit::SharedString>,
        version: Option<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<DockArea> {
        cx.new(|cx| {
            let renderer: Rc<dyn DockAreaRenderer> = Rc::new(LocalDockRenderer {
                title_height: self.title_height,
                resizing: Rc::new(Cell::new(None)),
                area: cx.entity().downgrade(),
                frames: self.frames.clone(),
                dragging: self.dragging.clone(),
                split_resize: self.split_resize.clone(),
            });
            DockArea::new(id, version, window, cx).with_renderer(renderer)
        })
    }

    /// Persistence and regional reconciliation resume after a move or resize gesture ends.
    pub fn is_dragging(&self) -> bool {
        self.dragging.active() || self.split_resize.active()
    }

    /// Save Base's current tree, restoring hidden slot preferences into its existing size fields.
    pub fn dump(&self, area: &DockArea, cx: &App) -> DockAreaState {
        self.hidden_sizes.dump(area, self.dragging.anchor(), cx)
    }

    /// Discard a previous visibility projection immediately before loading a complete saved layout.
    pub fn reset_hidden_sizes(&self) {
        self.hidden_sizes.clear();
    }

    /// Prepare this root area's `width` from its current paint measurements, leaving Editor elastic.
    /// The host calls this after visibility reconciliation and before rendering the Base entity.
    pub fn prepare_frame(
        &self,
        area: &mut DockArea,
        width: Pixels,
        window: &mut Window,
        cx: &mut Context<DockArea>,
    ) {
        // Explicit visibility changes take effect immediately, including during a gesture.
        // They invalidate its original snapshot; ordinary hover never changes this projection.
        self.hidden_sizes
            .prepare(area, self.dragging.anchor(), window, cx);
        // Regional reflow still waits until the gesture releases or cancels its source geometry.
        if self.is_dragging() || cx.has_active_drag() {
            return;
        }
        let frames = self.frames.panels.borrow().clone();
        let center_width = self
            .frames
            .regions
            .borrow()
            .get(&DockPlacement::Center)
            .map(|bounds| bounds.size.width);
        columns::prepare(
            area,
            &frames,
            width,
            center_width,
            self.dragging.anchor(),
            window,
            cx,
        );
    }
}

struct LocalDockRenderer {
    title_height: f32,
    /// The UI tracks the active handle; Base owns all size constraints and layout state.
    resizing: Rc<Cell<Option<DockPlacement>>>,
    area: WeakEntity<DockArea>,
    frames: PaintedFrames,
    dragging: dragging::LiveDrag,
    split_resize: split_resize::SplitResize,
}

impl DockAreaRenderer for LocalDockRenderer {
    /// Record Base's actual center column; capture subtracts the separately painted bottom dock.
    fn center_frame(&self, _: &mut Window, _: &mut App) -> Stateful<Div> {
        let regions = self.frames.regions.clone();
        div()
            .id("dock-area-center")
            .on_prepaint(move |bounds, _, _| {
                regions.borrow_mut().insert(DockPlacement::Center, bounds);
            })
    }
    /// Window-level capture keeps moves and cancellation working across changing dock hit rectangles.
    fn frame(&self, _: &mut Window, cx: &mut App) -> Stateful<Div> {
        let frame = div().id("local-dock-area").child(
            self.dragging
                .surface(self.area.clone(), self.frames.clone()),
        );
        let frame = frame.child(gpui_kit::deferred(self.dragging.markers(cx)).with_priority(10));
        let frame = frame.child(self.split_resize.tracker(self.area.clone()));
        let frame = self.dragging.capture_escape(frame, self.area.clone());
        self.split_resize.capture_escape(frame, self.area.clone())
    }

    /// Hidden slots stay in Base's saved tree, while their visible neighbors share one handle.
    fn split_frame(
        &self,
        node: NodeId,
        _: gpui_kit::Axis,
        _: &mut Window,
        _: &mut App,
    ) -> Stateful<Div> {
        div()
            .id(("dock-split-frame", node.as_u64()))
            .relative()
            .child(gpui_kit::deferred(self.split_resize.surface(
                node,
                self.area.clone(),
                self.frames.clone(),
            )))
    }

    /// Retained hidden slots must not add a divider before the first visible sibling.
    fn render_split_handle(
        &self,
        handle: &gpui_base::ResizeHandleContext,
        _: &mut Window,
        _: &mut App,
    ) -> Option<AnyElement> {
        Some(split_resize::separator(
            handle,
            self.area.clone(),
            self.frames.clone(),
        ))
    }

    /// Compose project-owned dock chrome over Base's public resize behavior.
    fn render_dock(
        &self,
        dock: &DockContext,
        content: AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        resize::render(
            dock,
            content,
            self.resizing.clone(),
            self.area.clone(),
            self.frames.regions.clone(),
            window,
            cx,
        )
    }

    fn tab_group_renderer(&self) -> Rc<dyn TabGroupRenderer> {
        Rc::new(LocalTabGroupRenderer {
            title_height: self.title_height,
            area: self.area.clone(),
            node: Cell::new(None),
            bounds: Rc::new(Cell::new(Bounds::default())),
            frames: self.frames.clone(),
            dragging: self.dragging.clone(),
            hovered: Rc::new(Cell::new(false)),
            focus_subscriptions: RefCell::new(None),
        })
    }
}

struct LocalTabGroupRenderer {
    /// Each split leaf reveals only its own title actions when the pointer enters its full frame.
    hovered: Rc<Cell<bool>>,
    /// Base's original focus scope supplies keyboard reveal; replace listeners on active-panel changes.
    focus_subscriptions: RefCell<Option<(FocusHandle, [Subscription; 2])>>,
    title_height: f32,
    /// Only hit geometry is local; all lasting panel positions and sizes remain in DockArea.
    area: WeakEntity<DockArea>,
    node: Cell<Option<NodeId>>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    frames: PaintedFrames,
    dragging: dragging::LiveDrag,
}

impl Drop for LocalTabGroupRenderer {
    fn drop(&mut self) {
        // Removed native groups must not leave stale hit/size geometry behind.
        if let Some(node) = self.node.get() {
            self.frames.panels.borrow_mut().remove(&node);
        }
    }
}

impl LocalTabGroupRenderer {
    /// Draw `panel`'s live heading in `window`/`cx`, forwarding its pointer-or-focus `active` state.
    fn panel_title(
        panel: &Arc<dyn crate::dock::panel::BasePanelView>,
        active: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        PanelHandle::of(panel)
            .map(|handle| handle.title(active, window, cx))
            .unwrap_or_else(|| {
                // Keep dock identities stable while translating their visible fallback titles.
                let title = match panel.panel_name(cx) {
                    "Explorer" => rust_i18n::t!("panel.explorer").to_string(),
                    "Editor" => rust_i18n::t!("panel.editor").to_string(),
                    name => name.to_owned(),
                };
                title.into_any_element()
            })
    }
}

impl TabGroupRenderer for LocalTabGroupRenderer {
    /// Track `group`'s native focus and pointer scope in `window`/`cx`, including title and body.
    /// Return its measured frame; Base retains focus identity, activation and panel lifecycle.
    fn frame(&self, group: &TabGroupContext, window: &mut Window, cx: &mut App) -> Stateful<Div> {
        let node = group.node();
        self.node.set(Some(node));
        let frames = self.frames.clone();
        let focus = group.active_panel().map(|panel| panel.focus_handle(cx));
        let mut subscriptions = self.focus_subscriptions.borrow_mut();
        if subscriptions.as_ref().map(|(handle, _)| handle) != focus.as_ref() {
            *subscriptions = focus.map(|focus| {
                let entered = window.on_focus_in(&focus, cx, |window, _| window.refresh());
                let exited = window.on_focus_out(&focus, cx, |_, window, _| window.refresh());
                (focus, [entered, exited])
            });
        }
        let hovered = self.hovered.clone();
        div()
            .id("tab-group")
            .on_hover(move |inside, window, _| {
                // Pointer reveal never moves keyboard focus or activates a different panel.
                if hovered.replace(*inside) != *inside {
                    window.refresh();
                }
            })
            .on_prepaint(move |bounds, _, _| {
                frames.panels.borrow_mut().insert(node, bounds);
            })
    }

    /// Keep native drag events, rejecting only the central tab-merge zone.
    fn content_frame(&self, group: &TabGroupContext, _: &mut Window, _: &mut App) -> Stateful<Div> {
        let node = group.node();
        self.node.set(Some(node));
        let bounds = self.bounds.clone();
        let measured = bounds.clone();
        let droppable = group.is_droppable();
        div()
            .id("local-dock-content-frame")
            .debug_selector(move || format!("local-dock-content-{}", node.as_u64()))
            .on_prepaint(move |rect, _, _| measured.set(rect))
            .can_drop(move |value, window, _| {
                // GPUI Base 0.7 uses the middle 30% on each axis for tab merging.
                // Filter that zone against the live pointer, not a previous frame's hint.
                // Base owns the drag payload and normalized tree; local policy rejects tab merges.
                let rect = bounds.get();
                let pointer = window.mouse_position();
                droppable
                    && value.is::<DragPanel>()
                    && rect.contains(&pointer)
                    && (pointer.x < rect.left() + rect.size.width * 0.35
                        || pointer.x > rect.left() + rect.size.width * 0.65
                        || pointer.y < rect.top() + rect.size.height * 0.35
                        || pointer.y > rect.top() + rect.size.height * 0.65)
            })
    }

    fn render_tab_bar(
        &self,
        group: &TabGroupContext,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        // Every local layout slot contains one panel, so only its title is drawn.
        let Some(panel) = group.active_panel() else {
            return Empty.into_any_element();
        };
        if PanelHandle::of(panel).is_some_and(|handle| !handle.title_bar(cx)) {
            return Empty.into_any_element();
        }
        let style = component_styles(cx, ThemeComponent::DockTitleBar).base;
        let panel = panel.clone();
        let node = group.node();
        let drag = group.drag_panel(group.active_ix(), cx);
        let active = self.hovered.get() || panel.focus_handle(cx).contains_focused(window, cx);
        h_flex()
            .id("local-dock-title-bar")
            .debug_selector(move || format!("local-dock-title-{}", node.as_u64()))
            .relative()
            .w_full()
            .h(px(self.title_height))
            .flex_shrink_0()
            .items_center()
            .px(px(style.padding_x_px.unwrap_or(8.)))
            .text_size(px(style.font_size_px.unwrap_or(14.)))
            // A bottom-only border quad also antialiases its zero-width top edge. Paint a
            // solid strip instead, keeping its original layout inset and the caption seam thin.
            .pb(px(1.))
            .bg(style.background.unwrap_or(cx.theme().tab_bar))
            .text_color(style.foreground.unwrap_or(cx.theme().foreground))
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .w_full()
                    .h(px(1.))
                    .bg(style.border.unwrap_or(cx.theme().border)),
            )
            .child(Self::panel_title(&panel, active, window, cx))
            .when_some(
                drag.filter(|_| {
                    !group.is_locked() && Some(panel.panel_id(cx)) != self.dragging.anchor()
                }),
                |bar, drag| {
                    // The project permits moving a region's sole panel into another region; Base's local "last panel" guard
                    // would pin that leaf even when Editor and other docks remain visible. Preserve its lock/zoom authority.
                    // Base still owns the public payload, normalized split edits, and activation.
                    self.dragging.attach(
                        bar,
                        drag,
                        self.area.clone(),
                        self.frames.clone(),
                        self.title_height,
                    )
                },
            )
            .into_any_element()
    }
    fn render_active_panel(
        &self,
        panel: AnyView,
        group: &TabGroupContext,
        _: &mut Window,
        _: &mut App,
    ) -> AnyElement {
        if group.is_collapsed() {
            return Empty.into_any_element();
        }
        div()
            .id("local-dock-panel-content")
            .relative()
            .overflow_x_hidden()
            .flex_1()
            .child(panel.cached(StyleRefinement::default().absolute().size_full()))
            .overflow_y_scrollbar()
            .into_any_element()
    }
    /// The local marker layer owns exact-size previews; Base's default half-zone hint stays hidden.
    fn render_drop_indicator(
        &self,
        _: DropIndicator,
        _: &mut Window,
        _: &mut App,
    ) -> Option<AnyElement> {
        None
    }
}

/// Insert new panels as independent split leaves using the public DockArea API.
pub fn add_panel_view(
    area: &mut DockArea,
    panel: Arc<dyn PanelView>,
    placement: DockPlacement,
    size: Option<Pixels>,
    window: &mut Window,
    cx: &mut Context<DockArea>,
) {
    let id = panel.panel_id(cx);
    if area.panel(id).is_some() {
        return;
    }
    // A newly revealed exclusive leaf can still carry the previous frame's suppression.
    // Admission uses its raw intent; the next presentation pass selects the displayed combination.
    let visible =
        PanelHandle::of(&panel).map_or_else(|| panel.visible(cx), |handle| handle.visible(cx));
    let exclusive = presentation::is_exclusive(&panel, cx);
    // A declared left panel joins the existing sidebar instead of reserving another
    // outer column. Register first, then let Base split the retained sidebar band.
    if placement == DockPlacement::Left && placement::left_sidebar_target(area, cx).is_some() {
        area.add_panel_view(panel, DockPlacement::Center, None, window, cx);
        move_to_left_sidebar(area, id, window, cx);
        if visible {
            reveal_panel(area, id, window, cx);
        }
        return;
    }
    // The initial host right band is center-local so Bottom can span beneath it. New right
    // contributions join that band rather than create a full-height outer right dock.
    if placement == DockPlacement::Right && !area.has_dock(placement) {
        if let Some(node) = placement::center_right_target(area, cx) {
            area.add_panel_view(panel, DockPlacement::Center, None, window, cx);
            area.move_panel(
                id,
                InsertTarget::Split {
                    node,
                    placement: Placement::Bottom,
                    size: exclusive.then_some(px(0.)),
                },
                window,
                cx,
            );
            if visible {
                reveal_panel(area, id, window, cx);
            }
            return;
        }
    }
    let neighbor = area.layout(placement).and_then(|tree| {
        // Hidden alternatives have no ordinary share to divide. Prefer a displayed member
        // so a new ordinary leaf cannot inherit an exclusive leaf's zero-sized placeholder.
        tree.panels()
            .filter(|id| {
                area.panel(*id)
                    .is_some_and(|view| view.visible(cx) && !presentation::is_exclusive(view, cx))
            })
            .last()
            .or_else(|| {
                tree.panels()
                    .filter(|id| {
                        area.panel(*id)
                            .is_some_and(|view| !presentation::is_exclusive(view, cx))
                    })
                    .last()
            })
            .or_else(|| tree.panels().last())
            .and_then(|panel| tree.find_panel_node(panel))
    });
    // Base registers the view before it can be moved; finish both edits in one UI turn.
    area.add_panel_view(panel, placement, size, window, cx);
    if let Some(node) = neighbor {
        let placement = match placement {
            DockPlacement::Left | DockPlacement::Right => Placement::Bottom,
            _ => Placement::Right,
        };
        area.move_panel(
            id,
            InsertTarget::Split {
                node,
                placement,
                size: exclusive.then_some(px(0.)),
            },
            window,
            cx,
        );
    }
    if visible {
        reveal_panel(area, id, window, cx);
    }
}

/// Reveal a registered leaf on its actual side, joining an existing region if its old slot is isolated.
/// Shared/nested placements and peers' visibility survive; hidden native docks reopen as needed.
pub fn reveal_panel(
    area: &mut DockArea,
    panel: gpui_base::dock::PanelId,
    window: &mut Window,
    cx: &mut Context<DockArea>,
) {
    if let Some((node, placement)) = placement::reveal_target(area, panel, cx) {
        // An exclusive alternative never takes a share from the ordinary combination.
        // Base expands its sole visible slot; its zero retained weight preserves peer proportions.
        let size = area
            .panel(panel)
            .is_some_and(|view| presentation::is_exclusive(view, cx))
            .then_some(px(0.));
        area.move_panel(
            panel,
            InsertTarget::Split {
                node,
                placement,
                size,
            },
            window,
            cx,
        );
    }
    for placement in [
        DockPlacement::Left,
        DockPlacement::Right,
        DockPlacement::Bottom,
    ] {
        if area
            .layout(placement)
            .is_some_and(|tree| tree.find_panel_node(panel).is_some())
            && !area.is_dock_open(placement)
        {
            area.toggle_dock(placement, window, cx);
        }
    }
    area.select_panel(panel, window, cx);
}

/// Whether a registered leaf is painted in its actual region, including center-local placements.
/// Presentation filtering is supplied by the panel handle; a closed native dock is not visible.
pub fn panel_visible(area: &DockArea, panel: gpui_base::dock::PanelId, cx: &App) -> bool {
    area.panel(panel).is_some_and(|view| view.visible(cx))
        && [
            DockPlacement::Center,
            DockPlacement::Left,
            DockPlacement::Right,
            DockPlacement::Bottom,
        ]
        .into_iter()
        .any(|placement| {
            (placement == DockPlacement::Center || area.is_dock_open(placement))
                && area
                    .layout(placement)
                    .is_some_and(|tree| tree.find_panel_node(panel).is_some())
        })
}

/// Move a registered panel into the existing left band without replacing its view or layout tree.
/// Returns false when no left band exists; callers retain the ordinary peripheral fallback.
pub fn move_to_left_sidebar(
    area: &mut DockArea,
    panel: gpui_base::dock::PanelId,
    window: &mut Window,
    cx: &mut Context<DockArea>,
) -> bool {
    if area.panel(panel).is_none() {
        return false;
    }
    let Some((node, height)) = placement::left_sidebar_target(area, cx) else {
        return false;
    };
    // A new peer shares the whole sidebar, rather than halving a possibly hidden
    // last leaf. Give it the existing band's aggregate height as an initial weight;
    // before measurement use the viewport. Exclusive alternatives retain no ordinary share.
    let height = if area
        .panel(panel)
        .is_some_and(|view| presentation::is_exclusive(view, cx))
    {
        px(0.)
    } else {
        height.unwrap_or_else(|| window.viewport_size().height)
    };
    area.move_panel(
        panel,
        InsertTarget::Split {
            node,
            placement: Placement::Bottom,
            size: Some(height),
        },
        window,
        cx,
    );
    true
}

mod columns;
mod dragging;
mod hidden_sizes;
mod panel;
pub use panel::{
    BasePanel, BasePanelView, DockAreaState, DockState, Panel, PanelId, PanelState, TabGroup,
    panel_handle,
};
mod placement;
mod presentation;
mod resize;
mod split_resize;
mod title_actions;
pub use placement::stack_axis;
pub use presentation::PanelPresentation;
pub use presentation::has_visible_region;
pub use title_actions::title_actions;

#[cfg(test)]
mod tests;
