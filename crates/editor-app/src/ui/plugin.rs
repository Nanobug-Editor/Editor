//! Native plugin view lifecycle. GPUI entities stay here; guests receive typed events only.
mod action_gestures;
mod atlas;
pub(crate) mod bitmap;
mod canvas;
// Native host features use the same keyed canvas directly, without a guest Document or dispatch.
pub(crate) use canvas::CanvasView;
mod code;
mod code_sync;
mod containers;
#[cfg(test)]
mod content_styles_tests;
pub(crate) mod controls;
mod file_image;
mod form_delivery;
pub(crate) use form_delivery::{FormChangeSink, FormReceipt};
#[cfg(test)]
mod form_delivery_tests;
#[cfg(test)]
mod image_dimensions_tests;
pub(crate) mod images;
mod layout;
mod leaf;
#[cfg(test)]
mod link_tests;
mod links;
mod readonly_code;
mod render;
#[cfg(test)]
mod scrollbar_tests;
mod svg;
#[cfg(test)]
mod tests;
mod textareas;
mod theme;
mod tree;
#[cfg(test)]
mod tree_tests;
mod viewport;
#[cfg(test)]
mod viewport_tests;
mod virtual_blocks;
#[cfg(test)]
mod visibility_gesture_tests;
mod widgets;

use gpui_base::input::{InputEvent, InputState};
use gpui_kit::{
    App, AppContext as _, Context, Entity, FocusHandle, IntoElement, Render, ScrollHandle,
    Subscription, Window,
};
use plugin_runtime::plugin_protocol::{
    Environment,
    ui::{self, Action, Document, Kind, UiEvent},
};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

type EventSink = Rc<dyn Fn(UiEvent, &mut App)>;
/// Host-only borrowing adapter; the public guest contract contains no GPUI entity or editable copy.
pub(crate) type NativeEditorRenderer = Rc<
    dyn Fn(
        &plugin_runtime::plugin_protocol::api::DocumentVersion,
        Option<editor_ui::EditorActionElements>,
        Option<(editor_ui::aligned_editor::AlignmentGroup, usize)>,
        &mut Window,
        &mut App,
    ) -> Option<gpui_kit::AnyElement>,
>;

struct NativeInput {
    state: Entity<InputState>,
    value_revision: u64,
    last_value: String,
    _subscription: Subscription,
}

pub(crate) struct PluginView {
    pub(crate) native_editor: Option<NativeEditorRenderer>,
    /// Editor edge controls are checked again at input time, after native edits and queued paints.
    pub(crate) action_source_current:
        Option<Rc<dyn Fn(&plugin_runtime::plugin_protocol::api::DocumentVersion, &App) -> bool>>,
    /// Rail appearance includes disabled siblings; activation authority remains a separate gate.
    editor_action_nodes: BTreeSet<String>,
    plugin: String,
    document: Document,
    environment: Environment,
    /// Read-only code requests retain the project that supplied this scene.
    providers: Option<crate::language::providers::Providers>,
    sink: EventSink,
    /// Nested canvas/control sinks share the gate; retained readonly trees cannot dispatch stale input.
    scene_current: Rc<Cell<bool>>,
    /// Callbacks without a Window defer to this original native surface before checking IME.
    window: gpui_kit::AnyWindowHandle,
    inputs: BTreeMap<String, NativeInput>,
    /// Only undelivered ordinary form drafts wait for their exact native/guest incarnation.
    forms: form_delivery::Forms,
    textareas: BTreeMap<String, textareas::NativeTextarea>,
    readonly_code: BTreeMap<String, readonly_code::Snapshot>,
    /// Native pane synchronization owns derived byte maps and cancellable scroll locations only.
    code_sync: code_sync::CodeSync,
    scrolls: BTreeMap<String, ScrollHandle>,
    /// Read-only source block ownership and one revision-bound reveal; native scroll remains in Base.
    scene_layout: layout::SceneLayout,
    /// Coalesced source-block geometry and one locate receipt; Base owns the actual scroll offset.
    viewport: viewport::ViewportState,
    /// Exact block extents survive scene revisions independently of temporary visible elements.
    virtual_blocks: virtual_blocks::VirtualBlocks,
    /// Base resolves the release target; the local adapter retains the real native press owner.
    link_press: Option<links::LinkPress>,
    /// Per-target Base focus survives source refresh; subscriptions reveal only the focused link cue.
    link_focus: BTreeMap<String, links::LinkFocus>,
    /// Versioned readonly capture cache and a cancellable, bounded native background batch.
    code_highlights: code::CodeHighlights,
    canvases: BTreeMap<String, Entity<canvas::CanvasView>>,
    /// Geometry publications are deduplicated; zoom/pan state belongs exclusively to the guest.
    visual_measurements: BTreeMap<String, Rc<Cell<Option<(ui::ContentSize, ui::ContentSize)>>>>,
    /// Collection widgets retain native rename/drag state independently of canvas redraws.
    collections: BTreeMap<String, Entity<controls::CollectionView>>,
    /// Generic trees retain Base state by UI node and runtime resource incarnation.
    trees: BTreeMap<String, Entity<tree::TreeView>>,
    popup: Option<Entity<controls::CollectionView>>,
    origin: gpui_kit::Point<gpui_kit::Pixels>,
    dialog_focus: FocusHandle,
    /// The surface identifies descendant control focus without adding a container tab stop.
    view_focus: FocusHandle,
    /// Control focus survives a source refresh; gesture identity is separately bound to its scene.
    checkbox_focus: BTreeMap<String, FocusHandle>,
    /// Source controls revoke Base presses independently of retained input, scroll and focus state.
    action_gestures: action_gestures::ActionGestures,
    previous_focus: Option<FocusHandle>,
    dismissed_dialog: Option<String>,
    /// Editor-local toolbar projections fit their wrapped content instead of consuming the full pane.
    content_sized: bool,
    /// Decoded resources are accepted only for this tree's source version and URI.
    photos: BTreeMap<String, std::sync::Arc<images::Photo>>,
    /// GPU textures outlive pixel Arcs unless the last native projection explicitly evicts them.
    image_leases: atlas::ImageLeases,
}

impl PluginView {
    /// Withdraw projected gestures before an owning native window or pane is removed. The router
    /// flushes the original control's release while its old frame is still mounted.
    pub(crate) fn retire_code_panes(&mut self, cx: &mut Context<Self>) {
        self.invalidate_action_gestures(cx);
        self.code_sync.cancel(cx);
    }

    /// Suspend guest input and pending locations while retaining paint and the native text entity.
    pub(crate) fn set_scene_current(&mut self, current: bool, cx: &mut Context<Self>) {
        if self.scene_current.get() && !current {
            // Ordinary controls now survive hiding; their old pending Base presses must not.
            self.action_gestures.retained_visibility = true;
            self.invalidate_action_gestures(cx);
            self.suspend_popup(cx);
            for tree in self.trees.values() {
                tree.read(cx).resource.suspend();
            }
        }
        if !current {
            // Accepted requests may still await the next layout. Losing source authority
            // withdraws their effects too, without undoing an already applied Base offset.
            self.scene_layout.cancel_reveal();
            self.viewport.reset_scene();
            self.link_press = None;
            if self.document.code_alignment {
                if self.scene_current.get() {
                    self.code_sync.cancel_gestures();
                }
            } else {
                self.code_sync.cancel(cx);
            }
        }
        if self.scene_current.replace(current) != current {
            cx.notify();
        }
        if current {
            self.schedule_form_delivery(cx);
        }
    }
    /// Image decoding belongs to the worker; native children only borrow the matching immutable raster.
    pub(crate) fn update_images(
        &mut self,
        panel_key: &str,
        images: &images::SceneImages,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut photos = BTreeMap::new();
        let mut visit = |node: &plugin_runtime::plugin_protocol::ui::Node| {
            let expected = match &node.kind {
                Kind::Image { source, .. } => self.document.source.as_ref().map(|version| {
                    (
                        plugin_runtime::plugin_protocol::api::ContentVersion::Document(
                            version.clone(),
                        ),
                        source.as_str(),
                    )
                }),
                Kind::FileImage { .. } => self.document.file.as_ref().map(|version| {
                    (
                        plugin_runtime::plugin_protocol::api::ContentVersion::File(version.clone()),
                        "@current-file",
                    )
                }),
                _ => None,
            };
            if let Some((version, source)) = expected
                && let Some(photo) = images.photos.get(&format!("{panel_key}/image/{}", node.id))
                && photo.resource.source == version
                && source == photo.resource.uri
            {
                photos.insert(node.id.clone(), photo.clone());
            }
        };
        self.document.root.visit(&mut visit);
        if let Some(toolbar) = &self.document.editor_toolbar {
            toolbar.visit(&mut visit);
        }
        if let Some(dialog) = &self.document.dialog {
            dialog.content.visit(&mut visit);
        }
        let changed = photos.len() != self.photos.len()
            || photos.iter().any(|(id, image)| {
                self.photos
                    .get(id)
                    .is_none_or(|old| !std::sync::Arc::ptr_eq(old, image))
            });
        self.photos = photos;
        // Decoding may supply intrinsic dimensions after the original block was measured.
        if changed {
            self.virtual_blocks.clear();
        }
        if changed {
            self.link_press = None;
            cx.notify();
        }
        for (id, canvas) in &self.canvases {
            let graphics = images
                .graphics
                .get(&format!("{panel_key}/canvas/{id}"))
                .cloned();
            let images = images.get(&format!("{panel_key}/canvas/{id}")).cloned();
            canvas.update(cx, |view, cx| {
                let graphics = graphics.filter(|prepared| {
                    view.drawing.scene.as_ref() == Some(&prepared.source)
                        && prepared.belongs_to(&self.document)
                });
                if match (&view.scene_graphics, &graphics) {
                    (Some(old), Some(new)) => !std::sync::Arc::ptr_eq(old, new),
                    (None, None) => false,
                    _ => true,
                } {
                    view.scene_graphics = graphics;
                    cx.notify();
                }
                if match (&view.images, &images) {
                    (Some(old), Some(new)) => !std::sync::Arc::ptr_eq(old, new),
                    (None, None) => false,
                    _ => true,
                } {
                    view.images = images;
                    cx.notify();
                }
            });
        }
        self.sync_image_leases(window, cx);
    }

    /// Reconcile all decoded native pixels once per projection, including shared dialog/canvas images.
    fn sync_image_leases(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut pixels = self
            .photos
            .values()
            .filter_map(|photo| {
                photo
                    .decoded
                    .as_ref()
                    .ok()
                    .and_then(Option::as_ref)
                    .map(|bitmap| bitmap.image.clone())
            })
            .collect::<Vec<_>>();
        for canvas in self.canvases.values() {
            if let Some(images) = &canvas.read(cx).images {
                pixels.extend(images.iter().flatten().map(|image| image.image.clone()));
            }
            if let Some(prepared) = &canvas.read(cx).scene_graphics {
                if let Ok(drawing) = &prepared.drawing {
                    pixels.extend(drawing.images());
                }
            }
        }
        for image in self.image_leases.update(pixels, cx) {
            // GPUI removes the borrowed window from App.windows during rendering and updates.
            cx.drop_image(image, Some(window));
        }
    }
    pub(crate) fn new(
        plugin: String,
        document: Document,
        environment: Environment,
        sink: impl Fn(UiEvent, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // RenderImage Arcs do not evict GPUI's atlas. Entity retirement therefore owns
        // an explicit cleanup, including the current window while App temporarily lends it out.
        let image_window = window.window_handle();
        cx.on_release(move |this, cx| {
            let pixels = this.image_leases.clear(cx);
            if image_window
                .update(cx, |_, window, cx| {
                    for image in &pixels {
                        cx.drop_image(image.clone(), Some(window));
                    }
                })
                .is_err()
            {
                // A closed owner window is already gone; shared application atlases still need retirement.
                for image in pixels {
                    cx.drop_image(image, None);
                }
            }
        })
        .detach();
        let scene_current = Rc::new(Cell::new(true));
        let gate = scene_current.clone();
        let mut this = Self {
            providers: crate::language::providers::Providers::for_workspace(std::path::Path::new(
                &environment.workspace,
            )),
            native_editor: None,
            action_source_current: None,
            editor_action_nodes: BTreeSet::new(),
            plugin,
            document,
            environment,
            sink: Rc::new(move |event, cx| {
                if gate.get() {
                    sink(event, cx)
                }
            }),
            scene_current,
            window: window.window_handle(),
            inputs: BTreeMap::new(),
            forms: Default::default(),
            textareas: BTreeMap::new(),
            readonly_code: BTreeMap::new(),
            code_sync: Default::default(),
            scrolls: BTreeMap::new(),
            scene_layout: Default::default(),
            viewport: Default::default(),
            virtual_blocks: Default::default(),
            link_press: None,
            link_focus: BTreeMap::new(),
            code_highlights: Default::default(),
            canvases: BTreeMap::new(),
            visual_measurements: BTreeMap::new(),
            collections: BTreeMap::new(),
            trees: BTreeMap::new(),
            popup: None,
            origin: Default::default(),
            dialog_focus: cx.focus_handle(),
            view_focus: cx.focus_handle().tab_stop(false),
            checkbox_focus: BTreeMap::new(),
            action_gestures: Default::default(),
            previous_focus: None,
            dismissed_dialog: None,
            content_sized: false,
            photos: BTreeMap::new(),
            image_leases: Default::default(),
        };
        this.sync_native(window, cx);
        if this.document.dialog.is_some() {
            this.focus_dialog(window, cx);
        }
        this
    }

    /// Retain the same native controls and event gate while letting a surrounding layout own height.
    pub(crate) fn content_sized(mut self) -> Self {
        self.content_sized = true;
        self
    }

    /// Check document identity, excluding revision, before retaining hidden native state during refresh.
    pub(crate) fn same_source_document(
        &self,
        version: &plugin_runtime::plugin_protocol::api::DocumentVersion,
    ) -> bool {
        self.document
            .source
            .as_ref()
            .is_some_and(|source| source.id == version.id && source.path == version.path)
    }

    /// A preview edit preserves its originating native control; toolbar edits still focus source.
    pub(crate) fn contains_focus(&self, window: &Window, cx: &App) -> bool {
        self.view_focus.contains_focused(window, cx)
    }

    /// Reconcile keyed native state; ordinary guest renders must not recreate focused inputs.
    pub(crate) fn update_document(
        &mut self,
        document: Document,
        environment: Environment,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.document == document && self.environment == environment {
            return;
        }
        // Withdraw target ownership while the old frame/gesture still exists. Deferred cleanup
        // keeps only that original scope and is separately checked against Manager's native leases.
        for (id, canvas) in &self.canvases {
            let next = document
                .active_node(id)
                .filter(|node| !node.disabled)
                .and_then(|node| {
                    if let Kind::Canvas(canvas) = &node.kind {
                        Some(canvas)
                    } else {
                        None
                    }
                });
            let current = self.document.source == document.source
                && self.document.file == document.file
                && (self.document == document || self.document.editor_action_source(id).is_none());
            let reason = if document.dialog.is_some() || document.menu.is_some() {
                ui::CanvasCancelReason::Modal
            } else if !current {
                ui::CanvasCancelReason::SceneReplaced
            } else {
                ui::CanvasCancelReason::TargetRemoved
            };
            canvas.update(cx, |view, cx| {
                view.reconcile_target_lifetimes(next, current, reason, window, cx)
            });
        }
        if self.document != document {
            self.invalidate_action_gestures(cx);
            self.invalidate_code_highlighting(cx);
            if self.document.code_alignment != document.code_alignment {
                self.code_sync.cancel(cx);
            }
        }
        let old_dialog = self.document.dialog.as_ref().map(|d| d.id.clone());
        // New resource authority must publish its intrinsic geometry again, even at the same size.
        if self.document.file != document.file || self.document.source != document.source {
            // A retained node name cannot carry a draft into another source owner.
            self.cancel_form_delivery();
            self.visual_measurements.clear();
        }
        if self.document.source.as_ref().map(|source| &source.id)
            != document.source.as_ref().map(|source| &source.id)
        {
            self.source_takes_viewport();
            self.virtual_blocks.clear();
        }
        // A new scene/theme cannot adopt an earlier pointer press, even when node IDs are reused.
        self.link_press = None;
        let next_dialog = document.dialog.as_ref().map(|d| d.id.clone());
        let menu_changed = self.document.menu.as_ref().map(|menu| &menu.id)
            != document.menu.as_ref().map(|menu| &menu.id);
        if self.environment != environment
            || self.document.content_styles != document.content_styles
        {
            self.virtual_blocks.clear();
        }
        self.virtual_blocks.retain(&document);
        self.document = document;
        // Source replacement revokes old scene pixels before another background publication arrives.
        for canvas in self.canvases.values() {
            canvas.update(cx, |canvas, _| {
                if canvas
                    .scene_graphics
                    .as_ref()
                    .is_some_and(|scene| !scene.belongs_to(&self.document))
                {
                    canvas.scene_graphics = None;
                    canvas.deactivate();
                    canvas.invalidate_measurement();
                }
            });
        }
        self.environment = environment;
        // Each popup opening owns a fresh dismiss acknowledgement, even when it reuses an ID.
        if menu_changed {
            self.dismissed_dialog = None;
        }
        if old_dialog != next_dialog {
            self.dismissed_dialog = None;
            if next_dialog.is_some() {
                self.focus_dialog(window, cx);
            } else if let Some(focus) = self.previous_focus.take() {
                focus.focus(window, cx);
            }
        }
        self.sync_native(window, cx);
        // A removed image or source replacement can retire this tree without another worker update.
        let mut keep = BTreeSet::new();
        let mut visit = |node: &plugin_runtime::plugin_protocol::ui::Node| {
            let Some(photo) = self.photos.get(&node.id) else {
                return;
            };
            // Both source types must retain their exact authority, even when the node ID is reused.
            let current = match &node.kind {
                Kind::Image { source, .. } => {
                    self.document
                        .source
                        .as_ref()
                        .is_some_and(|version| photo.resource.source == *version)
                        && source == &photo.resource.uri
                }
                Kind::FileImage { .. } => {
                    self.document.file.as_ref().is_some_and(|version| {
                        photo.resource.source
                            == plugin_runtime::plugin_protocol::api::ContentVersion::File(
                                version.clone(),
                            )
                    }) && photo.resource.uri == "@current-file"
                }
                _ => false,
            };
            if current {
                keep.insert(node.id.clone());
            }
        };
        self.document.root.visit(&mut visit);
        if let Some(toolbar) = &self.document.editor_toolbar {
            toolbar.visit(&mut visit);
        }
        if let Some(dialog) = &self.document.dialog {
            dialog.content.visit(&mut visit);
        }
        self.photos.retain(|id, _| keep.contains(id));
        self.sync_image_leases(window, cx);
        // Closing a source-only overlay can retire this view before it is rendered again.
        // Reconcile popup removal here as well so its previous native focus is returned first.
        self.sync_widgets(window, cx);
        cx.notify();
    }

    fn focus_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.previous_focus.is_none() {
            self.previous_focus = window.focused(cx);
        }
        self.dialog_focus.focus(window, cx);
    }

    fn sync_native(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.scene_layout.reset(&self.document);
        self.viewport.reset_scene();
        self.sync_link_focus(window, cx);
        self.sync_tree_action_gestures(cx);
        let mut nodes = vec![];
        self.document
            .root
            .visit(&mut |node| nodes.push(node.clone()));
        if let Some(dialog) = &self.document.dialog {
            dialog.content.visit(&mut |node| nodes.push(node.clone()));
        }
        self.editor_action_nodes.clear();
        for node in &nodes {
            if let Kind::EditorWithActions { left, right, .. } = &node.kind {
                for action in left.iter().chain(right) {
                    action.content.visit(&mut |child| {
                        self.editor_action_nodes.insert(child.id.clone());
                    });
                }
            }
        }
        let input_ids: BTreeSet<_> = nodes
            .iter()
            .filter(|n| matches!(n.kind, Kind::Input(_)))
            .map(|n| n.id.clone())
            .collect();
        self.inputs.retain(|id, _| input_ids.contains(id));
        self.sync_textareas(&nodes, window, cx);
        self.sync_readonly_code(&nodes, window, cx);
        let scroll_ids: BTreeSet<_> = nodes
            .iter()
            .filter(|n| matches!(n.kind, Kind::Scroll { .. }))
            .map(|n| n.id.clone())
            .collect();
        self.scrolls.retain(|id, _| scroll_ids.contains(id));
        let canvas_ids: BTreeSet<_> = nodes
            .iter()
            .filter(|node| matches!(node.kind, Kind::Canvas(_)))
            .map(|node| node.id.clone())
            .collect();
        self.canvases.retain(|id, _| canvas_ids.contains(id));
        self.visual_measurements.retain(|id, _| {
            nodes.iter().any(|node| {
                node.id == *id && node.viewport.is_some() && self.document.active_node(id).is_some()
            })
        });
        let checkbox_ids: BTreeSet<_> = nodes
            .iter()
            .filter(|node| matches!(node.kind, Kind::Checkbox { .. }))
            .map(|node| node.id.clone())
            .collect();
        self.checkbox_focus
            .retain(|id, _| checkbox_ids.contains(id));
        for id in checkbox_ids {
            self.checkbox_focus
                .entry(id)
                .or_insert_with(|| cx.focus_handle());
        }
        self.sync_action_focus(&nodes, cx);
        for node in nodes {
            if let Kind::Canvas(drawing) = &node.kind {
                if !self.canvases.contains_key(&node.id) {
                    let owner = cx.entity().downgrade();
                    let id = node.id.clone();
                    let target_window = self.window;
                    let view = cx.new(|cx| {
                        let canvas = cx.entity().downgrade();
                        canvas::CanvasView::new_versioned(
                            drawing.clone(),
                            move |event, revision, gesture, cx| {
                                let owner = owner.clone();
                                let id = id.clone();
                                let canvas = canvas.clone();
                                let measurement = matches!(
                                    event,
                                    plugin_runtime::plugin_protocol::ui::CanvasEvent::Resize { .. }
                                        | plugin_runtime::plugin_protocol::ui::CanvasEvent::TextMeasured { .. }
                                );
                                cx.defer(move |cx| {
                                    let accepted = target_window.update(cx, |_, window, cx| {
                                        owner.update(cx, |this, cx| {
                                            if !this.action_gesture_current(&id, gesture)&&!canvas_cleanup_event(&event) {
                                                return false;
                                            }
                                            // Canvas and image viewports use the same input envelope, without host zoom state.
                                            let action = this
                                                .document
                                                .active_node(&id)
                                                .filter(|_| !matches!(event, ui::CanvasEvent::TextMeasured { .. }|ui::CanvasEvent::Input{..}))
                                                .and_then(|node| node.viewport.as_ref())
                                                .and_then(|viewport| viewport.content)
                                                .map_or_else(
                                                    || Action::Canvas(event.clone()),
                                                    |content| {
                                                        Action::ViewportInput(ui::ViewportInput {
                                                            content,
                                                            event: event.clone(),
                                                        })
                                                    },
                                                );
                                            this.emit_version_in(&id, revision, action, window, cx)
                                        })
                                    });
                                    if measurement && !matches!(accepted, Ok(Ok(true))) {
                                        let _ = canvas.update(cx, |view, cx| {
                                            view.invalidate_measurement();
                                            cx.notify();
                                        });
                                    }
                                });
                            },
                            window,
                            cx,
                        )
                    });
                    self.canvases.insert(node.id.clone(), view);
                }
                let active = !node.disabled && self.document.active_node(&node.id).is_some();
                let font = drawing.font.clone().over(self.environment.font_style(
                    &self.plugin,
                    node.theme_role(),
                    drawing.grid,
                ));
                let gesture = self.action_gesture(&node.id);
                self.canvases[&node.id].update(cx, |view, cx| {
                    view.set_action_gesture(gesture);
                    view.update_drawing(drawing.clone());
                    if view.viewport.as_ref().and_then(|v| v.content)
                        != node.viewport.as_ref().and_then(|v| v.content)
                    {
                        view.invalidate_measurement();
                    }
                    view.viewport = node.viewport.clone();
                    view.locale = self.environment.locale.clone();
                    view.font = font;
                    if active && !view.enabled {
                        view.invalidate_measurement();
                    }
                    view.enabled = active;
                    view.foreground = self.environment.foreground;
                    if !active {
                        view.deactivate();
                    } else if !drawing.focusable {
                        view.cancel_composition();
                    }
                    view.revision = self.document.revision;
                    cx.notify();
                });
            }
            if let Kind::Scroll { .. } = &node.kind {
                self.scrolls.entry(node.id.clone()).or_default();
            }
            let Kind::Input(input) = &node.kind else {
                continue;
            };
            if !self.inputs.contains_key(&node.id) {
                let state = cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(input.value.clone())
                        .placeholder(input.placeholder.clone())
                });
                let id = node.id.clone();
                let subscription = cx.subscribe_in(
                    &state,
                    window,
                    move |this, state, event: &InputEvent, window, cx| {
                        let value = state.read(cx).value().to_string();
                        let action = match event {
                            InputEvent::Change => {
                                let Some(entry) = this.inputs.get_mut(&id) else {
                                    return;
                                };
                                if entry.last_value == value {
                                    return;
                                }
                                entry.last_value = value.clone();
                                let revision = entry.value_revision;
                                this.form_changed(
                                    &id,
                                    state.entity_id(),
                                    revision,
                                    value,
                                    window,
                                    cx,
                                );
                                return;
                            }
                            InputEvent::PressEnter { .. } => Action::Submit(value),
                            _ => return,
                        };
                        this.emit_in(&id, action, window, cx);
                    },
                );
                self.inputs.insert(
                    node.id.clone(),
                    NativeInput {
                        state,
                        value_revision: input.value_revision,
                        last_value: input.value.clone(),
                        _subscription: subscription,
                    },
                );
            }
            let disabled = self.document.active_node(&node.id).is_none();
            let entry = self.inputs.get_mut(&node.id).unwrap();
            let replace = entry.value_revision != input.value_revision;
            if replace {
                self.forms.cancel(&node.id);
                entry.last_value = input.value.clone();
                entry.value_revision = input.value_revision;
            }
            entry.state.update(cx, |state, cx| {
                state.set_disabled(disabled, cx);
                state.set_placeholder(input.placeholder.clone(), window, cx);
                if replace {
                    state.set_value(input.value.clone(), window, cx);
                }
            });
        }
        self.reconcile_form_delivery(cx);
    }

    /// Non-source notifications can use an App-only entry. Source actions always require a Window.
    fn emit(&mut self, id: &str, action: Action, cx: &mut Context<Self>) {
        self.emit_version(id, self.document.revision, action, cx);
    }

    /// App-only callbacks fail closed for source actions: their IME state cannot be checked here.
    fn emit_version(
        &mut self,
        id: &str,
        revision: u64,
        action: Action,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.document.editor_action_source(id).is_some() {
            return false;
        }
        self.forward_version(id, revision, action, cx)
    }

    /// Native input retains its real Window so marked text is checked before owner authority.
    fn emit_in(&mut self, id: &str, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        self.emit_version_in(id, self.document.revision, action, window, cx);
    }

    /// Validate the local immutable native identity before reading the authoritative source owner.
    /// This path is for actual input/deferred dispatch only, never for constructing a native frame.
    fn event_source_current_in(&self, id: &str, window: &mut Window, cx: &mut App) -> bool {
        self.document.editor_action_source(id).is_none_or(|source| {
            self.code_action_native_current(source, window, cx)
                && self
                    .action_source_current
                    .as_ref()
                    .is_some_and(|current| current(source, cx))
        })
    }

    /// Deferred events retain their captured UI revision and return true only after actual forwarding.
    fn emit_version_in(
        &mut self,
        id: &str,
        revision: u64,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !canvas_cleanup_action(&action) && !self.event_source_current_in(id, window, cx) {
            return false;
        }
        self.forward_version(id, revision, action, cx)
    }

    /// Every native adapter shares scene, event-shape and dismissal validation immediately at send.
    fn forward_version(
        &mut self,
        id: &str,
        revision: u64,
        action: Action,
        cx: &mut Context<Self>,
    ) -> bool {
        let event = UiEvent {
            revision,
            node: id.into(),
            action,
        };
        if !self.scene_current.get()
            || self.document.validate_event(&event).is_err()
            // Never queue or retarget a business gesture ahead of a newer unsent field value.
            // Dismiss and ordinary observations remain available while that draft catches up.
            || (matches!(event.action, Action::Click | Action::Submit(_))
                && self.form_activation_blocked())
            || (matches!(event.action, Action::Dismiss)
                && self.dismissed_dialog.as_deref() == Some(id))
        {
            return false;
        }
        if matches!(event.action, Action::Dismiss) {
            self.dismissed_dialog = Some(id.into());
            cx.notify();
        }
        (self.sink)(event, cx);
        true
    }
}

/// Original-scoped cleanup is an observation, never a way to dispatch a replaced source action.
fn canvas_cleanup_event(event: &ui::CanvasEvent) -> bool {
    matches!(event,ui::CanvasEvent::Input{input} if input.cleanup_kind().is_some())
}
/// Both native Canvas envelopes retain the same narrow cleanup classification.
fn canvas_cleanup_action(action: &Action) -> bool {
    match action {
        Action::Canvas(event) | Action::ViewportInput(ui::ViewportInput { event, .. }) => {
            canvas_cleanup_event(event)
        }
        _ => false,
    }
}

impl Render for PluginView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_document(window, cx)
    }
}
