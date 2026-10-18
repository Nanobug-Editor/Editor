//! Generic native tree projection. Base retains selection, direction keys and virtual scrolling;
//! the shared runtime resource retains bounded provider pages and rejects obsolete callbacks.
use super::action_gestures::ActionGesture;
use super::*;
use crate::ui::controls::{
    Icon, TreeRowAppearance,
    menu::{MenuStyle, PopupMenu},
    tree_row,
};
use gpui_base::{Tree, TreeEvent, TreeItem, TreeState};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    InteractiveElement as _, MouseButton, ParentElement as _, StatefulInteractiveElement as _,
    StyleRefinement, Styled as _, div,
};
use plugin_runtime::{TreeResource, TreeSnapshot, plugin_protocol::tree as api};
use rust_i18n::t;
use std::sync::Arc;

#[cfg(test)]
mod refresh_tests;

/// Loading remains an intent until the owning PluginView has checked the original native Window.
/// In particular, a rejected source action must not first allocate a pending runtime page request.
enum Dispatch {
    Event(api::Event),
    Load {
        parent: Option<String>,
        continuation: bool,
    },
}
type TreeSink = Rc<dyn Fn(Dispatch, u64, Option<ActionGesture>, &mut App)>;

/// One keyed UI node borrows one instance-owned tree; no domain providers or text copies live here.
pub(super) struct TreeView {
    pub(super) resource: Arc<TreeResource>,
    tree: Entity<TreeState>,
    snapshot: TreeSnapshot,
    selected: Option<String>,
    revision: u64,
    /// Source-owned rows bind native presses and deferred requests to one admitted scene.
    action_gesture: Option<ActionGesture>,
    window: gpui_kit::AnyWindowHandle,
    node: String,
    environment: Environment,
    gate: Rc<Cell<bool>>,
    sink: TreeSink,
    sentinels: BTreeMap<String, (Option<String>, bool)>,
    popup: Option<Entity<PopupMenu>>,
    popup_epoch: Option<(u64, u64)>,
    _subscriptions: Vec<Subscription>,
    _pulse: gpui_kit::Task<()>,
}
impl TreeView {
    /// Native callbacks always keep the original scene revision and immutable resource incarnation.
    fn new(
        resource: Arc<TreeResource>,
        node: String,
        revision: u64,
        action_gesture: Option<ActionGesture>,
        environment: Environment,
        gate: Rc<Cell<bool>>,
        sink: TreeSink,
        window: gpui_kit::AnyWindowHandle,
        cx: &mut Context<Self>,
    ) -> Self {
        let tree = cx.new(|cx| TreeState::new(cx));
        let expanded = cx.subscribe(&tree, |this, _, event: &TreeEvent, cx| {
            match event {
                TreeEvent::Expanded(id) => this.load(Some(id.to_string()), false, cx),
                TreeEvent::Collapsed(id) => this.resource.collapse(id),
            }
            this.reconcile(cx);
        });
        let selected = cx.observe(&tree, |this, tree, cx| {
            let id = tree
                .read(cx)
                .selected_item()
                .map(|item| item.id.to_string());
            if id.as_ref().is_some_and(|id| !id.starts_with('@')) && id != this.selected {
                this.selected = id.clone();
                this.emit(
                    api::Event::Selected {
                        tree: this.resource.handle().clone(),
                        epoch: this.snapshot.epoch,
                        node: id.unwrap(),
                    },
                    cx,
                );
            }
        });
        let pulse = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(150))
                    .await;
                if this.update(cx, |this, cx| this.reconcile(cx)).is_err() {
                    break;
                }
            }
        });
        // Removing the view cancels pending pages even while the worker still owns the tree resource.
        let released = resource.clone();
        cx.on_release(move |_, _| released.suspend()).detach();
        let mut this = Self {
            resource,
            tree,
            snapshot: TreeSnapshot::default(),
            selected: None,
            revision,
            action_gesture,
            window,
            node,
            environment,
            gate,
            sink,
            sentinels: BTreeMap::new(),
            popup: None,
            popup_epoch: None,
            _subscriptions: vec![expanded, selected],
            _pulse: pulse,
        };
        this.reconcile(cx);
        this
    }

    /// Revoke source presses and menus immediately, retaining Base's selection, focus and scroll.
    /// A false/true withdrawal without a frame still changes the callbacks' captured credential.
    pub(super) fn set_action_gesture(
        &mut self,
        gesture: Option<ActionGesture>,
        revision: u64,
        cx: &mut Context<Self>,
    ) {
        if self.action_gesture == gesture {
            return;
        }
        self.action_gesture = gesture;
        self.revision = revision;
        self.popup_epoch = None;
        if let Some(menu) = self.popup.take() {
            let target = self.window;
            // Dismiss through the native menu lifecycle after its current owner borrow ends.
            // The old callback checks its credential before it can affect a newly opened menu.
            cx.defer(move |cx| {
                let _ = target.update(cx, |_, window, cx| {
                    menu.update(cx, |menu, cx| menu.dismiss(window, cx));
                });
            });
        }
        cx.notify();
    }

    /// A retained native row or key handler cannot change resource state for another source scene.
    fn frame_current(&self, gesture: Option<ActionGesture>) -> bool {
        self.action_gesture == gesture && self.gate.get() && self.resource.is_live()
    }

    /// Base processes its navigation actions before raw key events. Capture only stale frames,
    /// leaving all current selection/expansion and focus behavior in the upstream primitive.
    fn source_action_guard<A: gpui_kit::Action>(
        gesture: Option<ActionGesture>,
        cx: &Context<Self>,
    ) -> impl Fn(&A, &mut Window, &mut App) + 'static {
        cx.listener(move |this, _: &A, _, cx| {
            if !this.frame_current(gesture) {
                cx.stop_propagation();
            }
        })
    }

    pub(super) fn update(
        &mut self,
        revision: u64,
        gesture: Option<ActionGesture>,
        environment: Environment,
        cx: &mut Context<Self>,
    ) {
        self.set_action_gesture(gesture, revision, cx);
        self.revision = revision;
        self.environment = environment;
        self.reconcile(cx);
    }
    /// Page allocation is deferred with the intent, after source and scene authority validation.
    fn load(&mut self, parent: Option<String>, continuation: bool, cx: &mut Context<Self>) {
        if !self.gate.get() || !self.resource.is_live() {
            return;
        }
        (self.sink)(
            Dispatch::Load {
                parent,
                continuation,
            },
            self.revision,
            self.action_gesture,
            cx,
        );
    }
    fn emit(&self, event: api::Event, cx: &mut Context<Self>) {
        if self.gate.get() && self.resource.is_live() {
            (self.sink)(
                Dispatch::Event(event),
                self.revision,
                self.action_gesture,
                cx,
            );
        }
    }
    /// Base resets selection when replacing items. Restore by stable provider ID without changing scroll.
    fn reconcile(&mut self, cx: &mut Context<Self>) {
        if !self.resource.is_live() {
            if !self.snapshot.nodes.is_empty() {
                self.tree
                    .update(cx, |tree, cx| tree.set_items(Vec::new(), cx));
                self.snapshot = TreeSnapshot::default();
                cx.notify();
            }
            return;
        }
        // Errors, including deadline failures, wait for the explicit native Retry action.
        let current = self.resource.snapshot();
        if self.gate.get()
            && current
                .branches
                .get(&None)
                .is_some_and(|branch| branch.error.is_none())
        {
            self.load(None, false, cx);
        }
        let snapshot = self.resource.snapshot();
        // A source handover can reject a deferred page intent without changing resource metadata.
        // Retry only current unloaded expanded pages before the unchanged-projection fast path;
        // load still captures the current source/scene authority and never replays native gestures.
        for id in &snapshot.expanded {
            if snapshot
                .nodes
                .get(id)
                .is_some_and(|node| node.branch && !node.disabled)
                && snapshot
                    .branches
                    .get(&Some(id.clone()))
                    .is_some_and(|branch| {
                        !branch.loaded && !branch.loading && branch.error.is_none()
                    })
            {
                self.load(Some(id.clone()), false, cx);
            }
        }
        if snapshot.revision == self.snapshot.revision {
            return;
        }
        self.snapshot = snapshot;
        self.sentinels.clear();
        let roots = self.items(None);
        let selected = self.selected.clone();
        self.tree.update(cx, |tree, cx| {
            tree.set_items(roots, cx);
            if let Some(selected) = selected {
                tree.set_selected_index(tree.index_of(&selected.into()), cx);
            }
        });
        cx.notify();
    }
    /// A host-reserved row keeps an unloaded branch expandable through Base's public folder contract.
    fn items(&mut self, parent: Option<String>) -> Vec<TreeItem> {
        let Some(branch) = self.snapshot.branches.get(&parent).cloned() else {
            return Vec::new();
        };
        let mut items = Vec::new();
        for id in branch.children {
            let Some(node) = self.snapshot.nodes.get(&id).cloned() else {
                continue;
            };
            let children = if node.branch {
                self.items(Some(id.clone()))
            } else {
                Vec::new()
            };
            items.push(
                TreeItem::new(id.clone(), node.label)
                    .children(children)
                    .expanded(self.snapshot.expanded.contains(&id))
                    .disabled(node.disabled),
            );
        }
        let status = if branch.loading {
            Some(("loading", t!("tree.loading").to_string(), false))
        } else if let Some(error) = branch.error {
            Some(("retry", format!("{}: {}", t!("tree.retry"), error), true))
        } else if !branch.loaded {
            Some(("lazy", t!("tree.expand").to_string(), false))
        } else if branch.next.is_some() {
            Some(("more", t!("tree.more").to_string(), true))
        } else if items.is_empty() && parent.is_some() {
            Some(("empty", t!("tree.empty").to_string(), false))
        } else {
            None
        };
        if let Some((kind, label, actionable)) = status {
            let id = format!("@{kind}/{}", parent.as_deref().unwrap_or("@root"));
            if actionable {
                self.sentinels
                    .insert(id.clone(), (parent, branch.loaded && branch.next.is_some()));
            }
            items.push(TreeItem::new(id, label).disabled(!actionable));
        }
        items
    }
    /// Disclosure changes only Base's current items; resource reconciliation preserves native selection.
    fn toggle(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.snapshot.expanded.contains(id) {
            self.resource.collapse(id);
        } else {
            self.load(Some(id.into()), false, cx);
        }
        self.reconcile(cx);
        self.tree.update(cx, |tree, cx| {
            tree.set_selected_index(tree.index_of(&id.to_string().into()), cx);
            tree.focus(window, cx);
        });
    }
    fn activate(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((parent, continuation)) = self.sentinels.get(id).cloned() {
            self.load(parent, continuation, cx);
            self.reconcile(cx);
            return;
        }
        if let Some(node) = self
            .snapshot
            .nodes
            .get(id)
            .cloned()
            .filter(|node| !node.disabled)
        {
            if node.activation.is_some() {
                self.invoke(id, api::NodeAction::Activation, cx);
            } else if node.branch {
                self.toggle(id, window, cx);
            }
        }
    }
    fn invoke(&self, id: &str, action: api::NodeAction, cx: &mut Context<Self>) {
        self.emit(
            api::Event::Invoke {
                tree: self.resource.handle().clone(),
                epoch: self.snapshot.epoch,
                node: id.to_string(),
                action,
            },
            cx,
        );
    }
    /// Captured menus carry their tree epoch; worker admission rejects stale node or command metadata.
    fn open_menu(
        &mut self,
        id: &str,
        position: gpui_kit::Point<gpui_kit::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(node) = self
            .snapshot
            .nodes
            .get(id)
            .filter(|node| !node.disabled && !node.menu.is_empty())
        else {
            return;
        };
        let items = node
            .menu
            .iter()
            .enumerate()
            .map(|(index, command)| ui::MenuItem {
                // Distinct slots keep separate native row identities even for the same typed command.
                id: format!("tree-menu-{index}"),
                label: command.label.for_locale(&self.environment.locale).into(),
                disabled: false,
                separator_before: false,
            })
            .collect::<Vec<_>>();
        let owner = cx.entity().downgrade();
        let id = id.to_string();
        let epoch = self.snapshot.epoch;
        let revision = self.revision;
        let gesture = self.action_gesture;
        let menu = cx.new(|cx| {
            PopupMenu::new(
                items,
                MenuStyle::current(cx),
                position,
                move |action, _, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        if !this.frame_current(gesture)
                            || this.popup_epoch != Some((epoch, revision))
                            || this.revision != revision
                            || this.snapshot.epoch != epoch
                        {
                            return;
                        }
                        if let Action::Select(item) = action {
                            if let Some(index) = item
                                .strip_prefix("tree-menu-")
                                .and_then(|index| index.parse::<u32>().ok())
                            {
                                this.invoke(&id, api::NodeAction::Menu { index }, cx);
                            }
                        }
                        this.popup = None;
                        this.popup_epoch = None;
                        cx.notify();
                    });
                },
                window,
                cx,
            )
        });
        self.popup = Some(menu);
        self.popup_epoch = Some((epoch, revision));
        cx.notify();
    }
}
impl Render for TreeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.gate.get()
            || !self.resource.is_live()
            || self
                .popup_epoch
                .is_some_and(|version| version != (self.snapshot.epoch, self.revision))
        {
            if let Some(menu) = self.popup.take() {
                // Normal dismissal restores owned focus; defer past this borrowed TreeView render.
                window.defer(cx, move |window, cx| {
                    menu.update(cx, |menu, cx| menu.dismiss(window, cx))
                });
            }
            self.popup_epoch = None;
        }
        let owner = cx.entity().downgrade();
        let snapshot = self.snapshot.clone();
        let rows_owner = owner.clone();
        let gesture = self.action_gesture;
        div()
            .id(action_gestures::native_id(
                format!("plugin-tree-{}", self.node),
                gesture,
            ))
            .size_full()
            .min_h_0()
            .relative()
            .capture_action(Self::source_action_guard::<gpui_base::actions::Confirm>(
                gesture, cx,
            ))
            .capture_action(Self::source_action_guard::<gpui_base::actions::SelectLeft>(
                gesture, cx,
            ))
            .capture_action(
                Self::source_action_guard::<gpui_base::actions::SelectRight>(gesture, cx),
            )
            .capture_action(Self::source_action_guard::<gpui_base::actions::SelectUp>(
                gesture, cx,
            ))
            .capture_action(Self::source_action_guard::<gpui_base::actions::SelectDown>(
                gesture, cx,
            ))
            .capture_key_down(cx.listener(move |this, _, _, cx| {
                if !this.frame_current(gesture) {
                    cx.stop_propagation();
                }
            }))
            .on_key_down(
                cx.listener(move |this, event: &gpui_kit::KeyDownEvent, window, cx| {
                    if !this.frame_current(gesture) {
                        return;
                    }
                    if event.keystroke.key == "enter" {
                        if let Some(id) = this
                            .tree
                            .read(cx)
                            .selected_item()
                            .map(|item| item.id.to_string())
                        {
                            cx.stop_propagation();
                            this.activate(&id, window, cx);
                        }
                    }
                }),
            )
            .child(
                Tree::new(&self.tree)
                    .size_full()
                    .p_1()
                    .list_style(StyleRefinement::default().flex_grow_1().size_full())
                    .item(move |index, entry, state, _, cx| {
                        let id = entry.item().id.to_string();
                        let data = snapshot.nodes.get(&id);
                        let icon = data
                            .and_then(|node| node.icon.as_ref())
                            .and_then(|icon| snapshot.icons.get(icon.path(cx.theme().is_dark())))
                            .map(|bytes| Icon::default().data(bytes))
                            .unwrap_or_else(|| {
                                Icon::default().path("icons/outline-definition.svg")
                            });
                        let toggle_owner = rows_owner.clone();
                        let toggle_id = id.clone();
                        let toggle = Rc::new(move |window: &mut Window, cx: &mut App| {
                            let _ = toggle_owner.update(cx, |this, cx| {
                                if this.frame_current(gesture) {
                                    this.toggle(&toggle_id, window, cx);
                                }
                            });
                        });
                        let click_owner = rows_owner.clone();
                        let click_id = id.clone();
                        let menu_owner = rows_owner.clone();
                        let menu_id = id.clone();
                        let description = data
                            .map(|node| node.description.clone())
                            .unwrap_or_default();
                        let row = tree_row(
                            action_gestures::native_id(format!("plugin-tree-row-{id}"), gesture)
                                .to_string(),
                            entry.item().label.to_string(),
                            entry.depth(),
                            icon,
                            entry.is_folder(),
                            entry.is_expanded(),
                            state.is_selected(),
                            id.starts_with("@retry/"),
                            TreeRowAppearance::Explorer,
                            cx,
                            entry
                                .is_folder()
                                .then_some(toggle as Rc<dyn Fn(&mut Window, &mut App)>),
                        )
                        .debug_selector(move || format!("plugin-tree-row-{id}").into())
                        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                            cx.stop_propagation();
                            let _ = click_owner.update(cx, |this, cx| {
                                if !this.frame_current(gesture) {
                                    return;
                                }
                                this.tree.update(cx, |tree, cx| {
                                    tree.set_selected_index(Some(index), cx);
                                    tree.focus(window, cx);
                                });
                                if event.click_count == 2 {
                                    this.activate(&click_id, window, cx);
                                }
                            });
                        })
                        .on_mouse_down(
                            MouseButton::Right,
                            move |event, window, cx| {
                                cx.stop_propagation();
                                let _ = menu_owner.update(cx, |this, cx| {
                                    if this.frame_current(gesture) {
                                        this.open_menu(&menu_id, event.position, window, cx);
                                    }
                                });
                            },
                        );
                        if description.is_empty() {
                            row.into_any_element()
                        } else {
                            row.tooltip(move |_, cx| {
                                cx.new(|_| crate::ui::controls::Tooltip::new(description.clone()))
                                    .into()
                            })
                            .into_any_element()
                        }
                    }),
            )
            .children(self.popup.clone())
    }
}

impl PluginView {
    /// Source binding and withdrawal change before resource publication or another native frame.
    /// Keep the same TreeState while revoking every retained callback with its captured credential.
    pub(super) fn sync_tree_action_gestures(&mut self, cx: &mut Context<Self>) {
        for (id, tree) in &self.trees {
            let gesture = self.action_gesture(id);
            tree.update(cx, |view, cx| {
                view.set_action_gesture(gesture, self.document.revision, cx);
            });
        }
    }

    /// Base tree observation and paging have no Window parameter. Resume in the original surface
    /// after the tree's current borrow ends, then apply the same gate as every other native action.
    fn dispatch_tree(
        &mut self,
        id: &str,
        revision: u64,
        gesture: Option<ActionGesture>,
        resource: &TreeResource,
        dispatch: Dispatch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.scene_current.get()
            || !self.action_gesture_current(id, gesture)
            || revision != self.document.revision
            || !resource.is_live()
            || !self.document.active_node(id).is_some_and(
                |node| matches!(&node.kind, Kind::Tree { handle } if handle == resource.handle()),
            )
            || !self.event_source_current_in(id, window, cx)
        {
            return;
        }
        let event = match dispatch {
            Dispatch::Event(event) => event,
            Dispatch::Load {
                parent,
                continuation,
            } => {
                let Ok(Some(request)) = resource.request(parent, continuation) else {
                    return;
                };
                api::Event::Request(request)
            }
        };
        self.emit_version_in(id, revision, Action::Tree(event), window, cx);
    }

    /// Published maps are joined by the entire opaque owner, never by a business plugin ID.
    pub(crate) fn update_trees(&mut self, resources: &[Arc<TreeResource>], cx: &mut Context<Self>) {
        let mut requested = Vec::new();
        self.document.root.visit(&mut |node| {
            if let Kind::Tree { handle } = &node.kind {
                requested.push((node.id.clone(), handle.clone()));
            }
        });
        let previous = self.trees.len();
        self.trees.retain(|id, view| {
            let resource = &view.read(cx).resource;
            // Withdraw retired authorities immediately; a pulse must never keep stale rows visible.
            resource.is_live()
                && resources
                    .iter()
                    .any(|current| current.handle() == resource.handle())
                && requested
                    .iter()
                    .any(|(node, handle)| node == id && resource.handle() == handle)
        });
        if self.trees.len() != previous {
            cx.notify();
        }
        for (id, handle) in requested {
            let Some(resource) = resources
                .iter()
                .find(|resource| resource.handle() == &handle && resource.is_live())
            else {
                continue;
            };
            let gesture = self.action_gesture(&id);
            if let Some(view) = self.trees.get(&id) {
                view.update(cx, |view, cx| {
                    view.update(
                        self.document.revision,
                        gesture,
                        self.environment.clone(),
                        cx,
                    )
                });
            } else {
                let resource = resource.clone();
                let environment = self.environment.clone();
                let gate = self.scene_current.clone();
                let owner = cx.entity().downgrade();
                let target_window = self.window;
                let target_node = id.clone();
                let target_resource = resource.clone();
                let sink: TreeSink = Rc::new(move |dispatch, revision, gesture, cx| {
                    let owner = owner.clone();
                    let node = target_node.clone();
                    let resource = target_resource.clone();
                    cx.defer(move |cx| {
                        let _ = target_window.update(cx, |_, window, cx| {
                            let _ = owner.update(cx, |this, cx| {
                                this.dispatch_tree(
                                    &node, revision, gesture, &resource, dispatch, window, cx,
                                )
                            });
                        });
                    });
                });
                let revision = self.document.revision;
                self.trees.insert(
                    id.clone(),
                    cx.new(|cx| {
                        TreeView::new(
                            resource,
                            id,
                            revision,
                            gesture,
                            environment,
                            gate,
                            sink,
                            target_window,
                            cx,
                        )
                    }),
                );
            }
        }
    }
}
