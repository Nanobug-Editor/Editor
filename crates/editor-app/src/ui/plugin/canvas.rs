//! A keyed canvas owns its focus and IME; sibling native inputs keep their independent handlers.
use super::*;
use gpui_kit::{
    Bounds, ElementInputHandler, EntityInputHandler, FontWeight, InteractiveElement, KeyDownEvent,
    KeyUpEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels,
    Point, ScrollWheelEvent, StatefulInteractiveElement, Styled, Subscription, TextRun,
    UTF16Selection, canvas, div, fill, font, point, px, rgb, size,
};
use plugin_runtime::plugin_protocol::{
    self as protocol,
    ui::{Canvas, CanvasEvent, GridMetrics, PointerPhase, ScrollChange},
};
use std::ops::Range;
#[cfg(test)]
mod column_tests;
#[cfg(test)]
mod native_columns_tests;
mod overlays;
pub(in crate::ui::plugin) use overlays::OverlayLayout;
pub(super) mod scene;
#[cfg(all(test, target_os = "windows"))]
mod scene_native_tests;
mod scroll;
mod semantics;
mod targets;
mod text;
mod text_layout;
mod transfer;
pub(in crate::ui::plugin) use transfer::TransferFrame;

pub(crate) struct CanvasView {
    /// Names resolve against the same host locale as ordinary native plugin controls.
    pub(super) locale: String,
    /// Target identities retain native focus independently of the legacy Canvas opt-in.
    targets: targets::Targets,
    /// Ordinary keyed controls are rendered by their document owner above scene pixels.
    pub(super) overlay_renderer: Option<overlays::OverlayRenderer>,
    /// Last successful native geometry is presentation-only and survives menu input takeover.
    anchor_geometry: Option<overlays::AnchorGeometry>,
    /// Notify the document only when actual anchor geometry changes, never on idle paints.
    pub(super) anchor_changed: Option<Rc<dyn Fn(&mut App)>>,
    /// Forward an actually hit native source without synthesizing a guest pointer or borrowing newer data.
    pub(super) source_pressed: Option<
        Rc<
            dyn Fn(
                protocol::ui::CanvasAnchor,
                protocol::transfer::DragSource,
                u64,
                Point<Pixels>,
                &mut App,
            ),
        >,
    >,
    /// A removed focused native child waits for a successful actual paint before returning focus.
    pending_overlay_focus: Option<(u64, protocol::ui::CanvasTargetRef)>,
    pub(crate) drawing: Canvas,
    /// Optional guest-supplied visual projection; ordinary canvases retain their existing coordinates.
    pub(super) viewport: Option<protocol::ui::VisualViewport>,
    pub(crate) font: protocol::FontStyle,
    pub(super) enabled: bool,
    pub(super) revision: u64,
    /// Only editor-source descendants bind a short-lived pointer/dispatch credential.
    action_gesture: Option<action_gestures::ActionGesture>,
    pub(crate) foreground: u32,
    pub(super) images: Option<std::sync::Arc<Vec<Option<super::images::VectorImage>>>>,
    /// Worker-tessellated artwork must match the complete current public scene before painting.
    pub(super) scene_graphics: Option<std::sync::Arc<scene::PreparedScene>>,
    /// Display receipts converge per scene outcome, independently of ordinary native repaints.
    scene_presented: Option<CanvasEvent>,
    /// Query observations retain their exact drawing, source, projection and native font binding.
    text_layout_result: Option<(u64, CanvasEvent)>,
    focus: FocusHandle,
    bounds: Bounds<Pixels>,
    measured: Option<CanvasEvent>,
    /// Reuse bounded native sample widths until the request or its inherited family changes.
    text_measurements: Option<(Vec<protocol::ui::TextMeasure>, Option<String>)>,
    /// Column previews belong to native frames; the guest receives only completed widths.
    columns: editor_ui::column_resize::ColumnResize,
    column_bindings: Vec<Option<usize>>,
    composition: String,
    selection: Range<usize>,
    drag: Option<MouseButton>,
    scroll: scroll::CanvasScroll,
    sink: Rc<dyn Fn(CanvasEvent, u64, Option<action_gestures::ActionGesture>, &mut App)>,
    /// Native acceptance observes the original callback before its ordinary input gates.
    #[cfg(test)]
    key_observer: Option<Rc<dyn Fn(serde_json::Value)>>,
    /// Test-only evidence of text actually submitted by the last native paint, including disabled canvases.
    #[cfg(test)]
    painted_text: Vec<String>,
    #[cfg(test)]
    painted_boxes: Vec<protocol::Rect>,
    #[cfg(test)]
    paint_revision: u64,
    _subscriptions: Vec<Subscription>,
}

#[cfg(test)]
impl PluginView {
    /// Observe the keyed native canvas's input state without changing focus or authority.
    /// The ID selects one canvas; window supplies actual focus ownership and cx reads its entity.
    pub(crate) fn native_canvas_input_snapshot(
        &self,
        id: &str,
        window: &Window,
        cx: &App,
    ) -> Option<serde_json::Value> {
        let canvas = self.canvases.get(id)?.read(cx);
        Some(serde_json::json!({
            "scene_revision": self.document.revision,
            "scene_current": self.scene_current.get(),
            "node_active": self.document.active_node(id).is_some(),
            "canvas_revision": canvas.revision,
            "enabled": canvas.enabled,
            "focusable": canvas.drawing.focusable,
            "focused": canvas.focus.is_focused(window),
            "canvas_focus": format!("{:?}", canvas.focus),
            "composition_empty": canvas.composition.is_empty(),
            "gesture_current": self.action_gesture_current(id, canvas.action_gesture),
            "source_gesture": canvas.action_gesture.is_some_and(action_gestures::ActionGesture::is_source),
            "bounds": format!("{:?}", canvas.bounds),
            "origin_x": canvas.bounds.origin.x / px(1.),
            "origin_y": canvas.bounds.origin.y / px(1.),
            "scroll_offset": canvas.drawing.scroll.as_ref().map(|scroll| scroll.offset),
            "paint_scroll_offset": canvas.drawing.scroll.as_ref().map(|scroll| scroll.offset - canvas.scroll_translation()),
            "paint_revision": canvas.paint_revision,
            "painted_text": canvas.painted_text,
            "painted_boxes": canvas.painted_boxes,
            "column_dragging": canvas.columns.dragging(),
            "column_hover": canvas.enabled && canvas.bounds.contains(&window.mouse_position())
                && canvas.columns.dividers().any(|x|
                    ((window.mouse_position().x - canvas.bounds.origin.x) / px(1.) - x).abs() <= 5.),
            "column_dividers": canvas.columns.dividers().collect::<Vec<_>>(),
            "width": canvas.bounds.size.width / px(1.),
            "height": canvas.bounds.size.height / px(1.),
        }))
    }

    /// Observe an unchanged, already-admitted UI event immediately before the original sink.
    /// The callback cannot replace its envelope, authorize a rejected event or replay input.
    pub(crate) fn observe_native_input(
        &mut self,
        id: &str,
        observe: Rc<dyn Fn(serde_json::Value)>,
        cx: &mut App,
    ) -> bool {
        let Some(canvas) = self.canvases.get(id) else {
            return false;
        };
        canvas.update(cx, |canvas, _| canvas.key_observer = Some(observe.clone()));
        let sink = self.sink.clone();
        let id = id.to_owned();
        self.sink = Rc::new(move |event, cx| {
            if event.node == id && matches!(&event.action, Action::Canvas(CanvasEvent::Key { .. }))
            {
                observe(serde_json::json!({"stage": "sink_forwarded", "event": event}));
            }
            sink(event, cx);
        });
        true
    }
}

impl CanvasView {
    /// Content replacement retires native gestures; drawing-only repaints retain their previews.
    pub(super) fn update_drawing(&mut self, drawing: Canvas) {
        if self.drawing.text_queries != drawing.text_queries {
            self.text_layout_result = None;
        }
        if self.drawing.scene.as_ref().map(|scene| scene.identity)
            != drawing.scene.as_ref().map(|scene| scene.identity)
        {
            self.deactivate();
        }
        if self.drawing.scene != drawing.scene {
            self.scene_graphics = None;
            self.scene_presented = None;
            self.text_layout_result = None;
        }
        if self.drawing.scroll_epoch != drawing.scroll_epoch {
            self.scroll.reset();
        }
        self.drawing = drawing;
        self.configure_columns();
    }

    /// Bind validated paint indices once; the drawing and provider's text remain immutable.
    fn configure_columns(&mut self) {
        let was_dragging = self.columns.dragging();
        self.column_bindings = vec![None; self.drawing.paint.len()];
        if let Some(columns) = &self.drawing.column_resize {
            self.columns
                .configure(columns.epoch, columns.x, &columns.widths, &columns.minimum);
            for binding in &columns.bindings {
                if let Some(slot) = self.column_bindings.get_mut(binding.paint) {
                    *slot = Some(binding.column);
                }
            }
        } else {
            self.columns.reset();
        }
        if was_dragging && !self.columns.dragging() {
            self.drag = None;
        }
    }
    /// Keep the locally painted result until acknowledgement and persist through one completion.
    fn finish_columns(&mut self, cx: &mut Context<Self>) {
        if let Some(widths) = self.columns.finish() {
            if let Some(columns) = &self.drawing.column_resize {
                self.emit(
                    CanvasEvent::ColumnsResized {
                        epoch: columns.epoch,
                        widths,
                    },
                    cx,
                );
            }
        }
    }
    /// Stretch one retained text rectangle without copying artwork or calling a worker.
    fn column_rect(&self, index: usize, mut rect: protocol::Rect) -> protocol::Rect {
        if let Some(Some(column)) = self.column_bindings.get(index) {
            (rect.x, rect.w) = self.columns.project(*column, rect.x, rect.w);
        }
        rect
    }

    /// Opted-in ranges carry content authority independently of the drawing revision.
    fn scroll_event(&self, change: ScrollChange) -> CanvasEvent {
        if let Some(epoch) = self.drawing.scroll_epoch {
            CanvasEvent::ScrollInput { epoch, change }
        } else {
            match change {
                ScrollChange::To { offset } => CanvasEvent::Scroll { offset },
                ScrollChange::Delta { .. } => unreachable!("legacy wheels retain their position"),
            }
        }
    }
    /// Canvas artwork uses the same public content transform as either native image kind.
    fn project(&self, rect: protocol::Rect, bounds: Bounds<Pixels>) -> protocol::Rect {
        let mut projected = self
            .viewport
            .as_ref()
            .and_then(|viewport| {
                viewport.content.map(|content| {
                    viewport.transform.unwrap_or_default().project(
                        rect,
                        content,
                        protocol::ui::ContentSize {
                            width: bounds.size.width / px(1.),
                            height: bounds.size.height / px(1.),
                        },
                    )
                })
            })
            .unwrap_or(rect);
        projected.y += self.scroll_translation();
        projected
    }

    /// Retained drawing follows native motion without awaiting a guest scene acknowledgement.
    /// Legacy canvases keep their original coordinates and visual viewport transform semantics.
    fn scroll_translation(&self) -> f32 {
        self.drawing
            .scroll_buffer
            .as_ref()
            .map_or(0., |buffer| self.scroll.translation(buffer))
    }
    /// Sibling native controls return keyboard input to the addressed canvas after their action.
    pub(crate) fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }
    pub(crate) fn new(
        drawing: Canvas,
        sink: impl Fn(CanvasEvent, u64, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_versioned(
            drawing,
            move |event, revision, _, cx| sink(event, revision, cx),
            window,
            cx,
        )
    }

    /// Plugin views retain the source credential at emission, including deferred geometry events.
    /// Standalone native canvases use `new` and retain their ordinary cross-render drag behavior.
    pub(super) fn new_versioned(
        drawing: Canvas,
        sink: impl Fn(CanvasEvent, u64, Option<action_gestures::ActionGesture>, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        targets::init(cx);
        Self::install_draft_retirement(window, cx);
        let focus = cx.focus_handle();
        let subscriptions = vec![
            cx.on_focus(&focus, window, |this, _, cx| {
                this.emit(CanvasEvent::Focus { focused: true }, cx)
            }),
            cx.on_blur(&focus, window, |this, _, cx| {
                this.finish_columns(cx);
                this.drag = None;
                this.cancel_composition();
                this.emit(CanvasEvent::Focus { focused: false }, cx);
                cx.notify();
            }),
            cx.observe_window_activation(window, |this, window, cx| {
                this.reconcile_target_activation(window, cx);
            }),
        ];
        let mut view = Self {
            locale: String::new(),
            targets: targets::Targets::new(cx),
            overlay_renderer: None,
            anchor_geometry: None,
            anchor_changed: None,
            source_pressed: None,
            pending_overlay_focus: None,
            drawing,
            viewport: None,
            font: Default::default(),
            enabled: true,
            revision: 0,
            action_gesture: None,
            foreground: 0,
            images: None,
            scene_graphics: None,
            scene_presented: None,
            text_layout_result: None,
            #[cfg(test)]
            painted_text: Vec::new(),
            #[cfg(test)]
            painted_boxes: Vec::new(),
            #[cfg(test)]
            paint_revision: 0,
            focus,
            bounds: Bounds::default(),
            measured: None,
            text_measurements: None,
            columns: Default::default(),
            column_bindings: Vec::new(),
            composition: String::new(),
            selection: 0..0,
            drag: None,
            scroll: Default::default(),
            sink: Rc::new(sink),
            #[cfg(test)]
            key_observer: None,
            _subscriptions: subscriptions,
        };
        view.configure_columns();
        view
    }
    fn emit(&self, event: CanvasEvent, cx: &mut Context<Self>) {
        if matches!(
            event,
            CanvasEvent::Pointer { .. } | CanvasEvent::Wheel { .. }
        ) {
            if let Some(scene) = &self.drawing.scene {
                if !matches!(&self.scene_presented,Some(CanvasEvent::ScenePresented{identity,scene_revision,projection,error:None,..})
                    if *identity==scene.identity&&*scene_revision==scene.revision&&*projection==self.viewport)
                {
                    return;
                }
            }
        }
        if self.enabled {
            (self.sink)(event, self.revision, self.action_gesture, cx);
        }
    }
    /// Revoke only source-owned gesture state; ordinary guest repaints keep their captured drag.
    pub(super) fn set_action_gesture(
        &mut self,
        gesture: Option<action_gestures::ActionGesture>,
    ) -> bool {
        if self.action_gesture == gesture {
            return false;
        }
        self.deactivate();
        self.action_gesture = gesture;
        true
    }
    /// A removed or disabled input target must not retain an unfinished native composition.
    pub(super) fn cancel_composition(&mut self) {
        self.composition.clear();
        self.selection = 0..0;
    }
    /// Deactivation revokes all input ownership; a non-focusable but active image keeps its drag.
    pub(super) fn deactivate(&mut self) {
        self.pending_overlay_focus = None;
        self.targets.clear();
        self.scene_presented = None;
        self.text_layout_result = None;
        self.cancel_composition();
        self.drag = None;
        self.columns.reset();
    }
    /// A rejected deferred callback or reactivation must retry its current native geometry.
    pub(crate) fn invalidate_measurement(&mut self) {
        self.scene_presented = None;
        self.text_layout_result = None;
        self.measured = None;
        self.text_measurements = None;
    }
    fn pointer(
        &self,
        phase: PointerPhase,
        position: Point<Pixels>,
        button: u8,
        clicks: u8,
        shift: bool,
        cx: &mut Context<Self>,
    ) {
        let point = position - self.bounds.origin;
        self.emit(
            CanvasEvent::Pointer {
                phase,
                x: point.x / px(1.),
                // Hit testing refers to the guest drawing, including unacknowledged local motion.
                y: point.y / px(1.) - self.scroll_translation(),
                button,
                clicks,
                shift,
            },
            cx,
        );
    }
    /// Geometry is measured at this layout node, independent of surrounding toolbars and panel edges.
    fn measure(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        // The captured anchor uses the old local origin; moving the surface must not reinterpret it.
        if self.bounds != bounds && self.columns.dragging() {
            self.columns.cancel();
            self.drag = None;
        }
        if self.bounds != bounds && !self.drawing.overlays.is_empty() {
            // A first layout (or resize) resolves viewport fitting before foreground construction.
            cx.notify();
        }
        self.bounds = bounds;
        self.scroll.update(bounds, self.drawing.scroll.clone());
        if let Some(offset) = self.scroll.take_offset() {
            self.emit(self.scroll_event(ScrollChange::To { offset }), cx);
        }
        let grid = self.drawing.grid.then(|| {
            let face = font(
                self.font
                    .family
                    .clone()
                    .unwrap_or_else(|| "Consolas".into()),
            );
            let id = window.text_system().resolve_font(&face);
            let size = self.font.size_px.unwrap_or(14.);
            GridMetrics {
                cell_width: window
                    .text_system()
                    .advance(id, px(size), 'M')
                    .map(|s| s.width / px(1.))
                    .unwrap_or(8.4),
                cell_height: (size * 1.45).ceil(),
            }
        });
        let event = CanvasEvent::Resize {
            width: bounds.size.width / px(1.),
            height: bounds.size.height / px(1.),
            grid,
        };
        if self.enabled && self.measured.as_ref() != Some(&event) {
            self.measured = Some(event.clone());
            self.emit(event, cx);
        }
        self.measure_text(window, cx);
    }
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        // The hit areas own ordinary hover; only a captured gesture overrides other cursor hits.
        if self.columns.dragging() {
            window.set_window_cursor_style(gpui_kit::CursorStyle::ResizeColumn);
        }
        #[cfg(test)]
        {
            self.painted_boxes.clear();
        }
        #[cfg(test)]
        {
            self.paint_revision = self.revision;
            self.painted_text = self
                .drawing
                .paint
                .iter()
                .filter_map(|operation| {
                    let (x, y, text, size) = match operation {
                        protocol::Paint::Text {
                            x, y, text, size, ..
                        } => (*x, *y, text, *size),
                        protocol::Paint::TextBox {
                            rect, text, size, ..
                        } => (rect.x, rect.y, text, *size),
                        _ => return None,
                    };
                    (!text.is_empty()
                        && x >= 0.
                        && x < bounds.size.width / px(1.)
                        && y + size > 0.
                        && y < bounds.size.height / px(1.))
                    .then(|| text.clone())
                })
                .collect();
        }
        self.capture_pointer(window, cx);
        self.capture_target_pointer(window, cx);
        if self.enabled && self.drawing.focusable && self.focus.is_focused(window) {
            window.handle_input(
                &self.focus,
                ElementInputHandler::new(bounds, cx.entity()),
                cx,
            );
        }
        for (index, operation) in self.drawing.paint.iter().enumerate() {
            match operation {
                protocol::Paint::Fill {
                    rect,
                    color,
                    extend_to_bottom,
                } => {
                    let mut rect = *rect;
                    if *extend_to_bottom {
                        let height = self
                            .viewport
                            .as_ref()
                            .and_then(|v| v.content)
                            .map_or(bounds.size.height / px(1.), |v| v.height);
                        rect.h = (height - rect.y).max(0.);
                    }
                    window.paint_quad(fill(
                        rect_bounds(self.project(rect, bounds), bounds.origin),
                        rgb(*color),
                    ));
                }
                protocol::Paint::Text {
                    x,
                    y,
                    text,
                    color,
                    size,
                    bold,
                    font: family,
                } => {
                    let mut face = font(
                        family
                            .clone()
                            .or(self.font.family.clone())
                            .unwrap_or_else(|| "Segoe UI".into()),
                    );
                    if *bold {
                        face.weight = FontWeight::BOLD;
                    }
                    paint_text(
                        text,
                        *size
                            * self
                                .viewport
                                .as_ref()
                                .and_then(|v| v.transform)
                                .map_or(1., |t| t.scale),
                        face,
                        rgb(*color).into(),
                        {
                            let rect = self.project(
                                protocol::Rect {
                                    x: *x,
                                    y: *y,
                                    w: 0.,
                                    h: 0.,
                                },
                                bounds,
                            );
                            bounds.origin + point(px(rect.x), px(rect.y))
                        },
                        window,
                        cx,
                    );
                }
                protocol::Paint::TextBox { rect, .. } => {
                    #[cfg(test)]
                    {
                        self.painted_boxes.push(self.column_rect(index, *rect));
                    }
                    text::paint_box(self, index, operation, bounds, window, cx)
                }
                protocol::Paint::Svg { .. } => {
                    if let Some(image) = self
                        .images
                        .as_ref()
                        .and_then(|images| images.get(index))
                        .and_then(Option::as_ref)
                    {
                        let _ = window.paint_image(
                            bounds,
                            rect_bounds(self.project(image.rect, bounds), bounds.origin),
                            Default::default(),
                            image.image.clone(),
                            0,
                            false,
                        );
                    }
                }
            }
        }
        let presentation = if let Some(prepared) = &self.scene_graphics {
            if self.drawing.scene.as_ref() == Some(&prepared.source) {
                let error = match &prepared.drawing {
                    Ok(drawing) => drawing.paint(self, bounds, window, cx).err(),
                    Err(error) => Some(error.clone()),
                };
                let error = error.map(|error| {
                    window.paint_quad(fill(bounds, gpui_kit::rgba(0xc0505022)));
                    paint_text(
                        &rust_i18n::t!("preview.canvas_failed"),
                        14.,
                        font(
                            self.font
                                .family
                                .clone()
                                .unwrap_or_else(|| "Segoe UI".into()),
                        ),
                        rgb(self.foreground).into(),
                        bounds.origin + point(px(8.), px(8.)),
                        window,
                        cx,
                    );
                    error.chars().take(256).collect::<String>()
                });
                Some(CanvasEvent::ScenePresented {
                    identity: prepared.source.identity,
                    scene_revision: prepared.source.revision,
                    source: prepared.document_source.clone(),
                    file: prepared.file_source.clone(),
                    projection: self.viewport.clone(),
                    error,
                })
            } else {
                None
            }
        } else {
            None
        };
        if let Some(presentation) = presentation {
            self.record_anchor_geometry(&presentation, bounds, cx);
            self.record_target_frame(&presentation, bounds, window, cx);
            if self.enabled && self.scene_presented.as_ref() != Some(&presentation) {
                self.scene_presented = Some(presentation.clone());
                self.emit(presentation, cx);
            }
            self.apply_overlay_focus(window, cx);
        }
        self.measure_paragraph_layouts(window, cx);
        self.install_target_text_input(window, cx);
        if !self.composition.is_empty() {
            let anchor = self.project(self.drawing.caret.unwrap_or_default(), bounds);
            paint_text(
                &self.composition,
                (self.font.size_px.unwrap_or(14.)
                    * self
                        .viewport
                        .as_ref()
                        .and_then(|v| v.transform)
                        .map_or(1., |t| t.scale))
                .clamp(1., 128.),
                font(
                    self.font
                        .family
                        .clone()
                        .unwrap_or_else(|| "Segoe UI".into()),
                ),
                rgb(self.foreground).into(),
                bounds.origin + point(px(anchor.x), px(anchor.y)),
                window,
                cx,
            );
        }
    }

    /// Capture only a drag begun here, retaining ownership outside the canvas until release.
    fn capture_pointer(&self, window: &mut Window, cx: &Context<Self>) {
        let gesture = self.action_gesture;
        let moved = cx.entity().downgrade();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
            if phase.capture() {
                let _ = moved.update(cx, |this, cx| {
                    if this.action_gesture == gesture
                        && this.enabled
                        && this.drag.is_some()
                        && this.drag == event.pressed_button
                    {
                        if this.columns.dragging() {
                            if this
                                .columns
                                .move_to((event.position.x - this.bounds.origin.x) / px(1.))
                            {
                                cx.notify();
                            }
                            cx.stop_propagation();
                            return;
                        }
                        this.pointer(
                            PointerPhase::Move,
                            event.position,
                            button_index(this.drag.unwrap()),
                            1,
                            event.modifiers.shift,
                            cx,
                        );
                        cx.stop_propagation();
                    }
                });
            }
        });
        let released = cx.entity().downgrade();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
            if phase.capture() {
                let _ = released.update(cx, |this, cx| {
                    if this.action_gesture == gesture
                        && this.enabled
                        && this.drag == Some(event.button)
                    {
                        this.drag = None;
                        if this.columns.dragging() {
                            this.columns
                                .move_to((event.position.x - this.bounds.origin.x) / px(1.));
                            this.finish_columns(cx);
                            cx.notify();
                            cx.stop_propagation();
                            return;
                        }
                        this.pointer(
                            PointerPhase::Up,
                            event.position,
                            button_index(event.button),
                            1,
                            event.modifiers.shift,
                            cx,
                        );
                        cx.stop_propagation();
                    }
                });
            }
        });
    }
}

/// The portable pointer numbering stays independent of GPUI's enum representation.
fn button_index(button: MouseButton) -> u8 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        MouseButton::Navigate(gpui_kit::NavigationDirection::Back) => 3,
        MouseButton::Navigate(gpui_kit::NavigationDirection::Forward) => 4,
    }
}

/// Host shaping preserves Unicode runs instead of delegating text layout to each WASM guest.
fn paint_text(
    text: &str,
    size: f32,
    face: gpui_kit::Font,
    color: gpui_kit::Hsla,
    origin: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let run = TextRun {
        len: text.len(),
        font: face,
        color,
        ..Default::default()
    };
    let line = window
        .text_system()
        .shape_line(text.to_owned().into(), px(size), &[run], None);
    let _ = line.paint(
        origin,
        px((size * 1.45).ceil()),
        gpui_kit::TextAlign::Left,
        None,
        window,
        cx,
    );
}
fn rect_bounds(rect: protocol::Rect, origin: Point<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        origin + point(px(rect.x), px(rect.y)),
        size(px(rect.w), px(rect.h)),
    )
}

impl Render for CanvasView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_targets(window, cx);
        self.sync_native_draft(window, cx);
        let gesture = self.action_gesture;
        #[cfg(test)]
        let callback_revision = self.revision;
        let measure = cx.entity().downgrade();
        let paint = measure.clone();
        let mut element = div()
            .id("composed-canvas")
            .size_full()
            .relative()
            .overflow_hidden()
            .track_focus(&self.focus)
            .cursor(self.target_cursor())
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.target_pointer_move(event, cx) {
                    cx.stop_propagation();
                }
            }))
            .on_hover(cx.listener(|this, over: &bool, window, cx| {
                if !*over {
                    this.leave_targets(window.mouse_position(), window.modifiers(), cx);
                }
            }))
            .capture_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if this.action_gesture == gesture
                    && this.enabled
                    && this.target_key(
                        &event.keystroke,
                        protocol::ui::CanvasKeyPhase::Down,
                        event.is_held,
                        window,
                        cx,
                    )
                {
                    let key = &event.keystroke;
                    if !event.prefer_character_input
                        && (key.modifiers.control
                            || key.modifiers.alt
                            || key.modifiers.platform
                            || (key.key.chars().count() > 1 && key.key != "space"))
                    {
                        cx.stop_propagation();
                    }
                }
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                #[cfg(test)]
                if let Some(observe) = &this.key_observer {
                    observe(serde_json::json!({
                        "stage": "canvas_received", "key": event.keystroke.key,
                        "canvas_revision": this.revision, "callback_revision": callback_revision,
                        "enabled": this.enabled, "focusable": this.drawing.focusable,
                        "focused": this.focus.is_focused(window),
                        "composition_empty": this.composition.is_empty(),
                        "gesture_current": this.action_gesture == gesture,
                    }));
                }
                if this.action_gesture != gesture
                    || !this.enabled
                    || !this.drawing.focusable
                    || !this.focus.is_focused(window)
                    || !this.composition.is_empty()
                {
                    return;
                }
                let key = &event.keystroke;
                this.emit(
                    CanvasEvent::Key {
                        key: key.key.clone(),
                        ctrl: key.modifiers.control,
                        alt: key.modifiers.alt,
                        shift: key.modifiers.shift,
                    },
                    cx,
                );
                // Count Unicode characters, not bytes: a Chinese character is ordinary IME text.
                if key.modifiers.control
                    || key.modifiers.alt
                    || (key.key.chars().count() > 1 && key.key != "space")
                {
                    cx.stop_propagation();
                }
            }))
            .capture_key_up(cx.listener(move |this, event: &KeyUpEvent, window, cx| {
                if this.action_gesture == gesture
                    && this.enabled
                    && this.target_key(
                        &event.keystroke,
                        protocol::ui::CanvasKeyPhase::Up,
                        false,
                        window,
                        cx,
                    )
                {
                    cx.stop_propagation();
                }
            }));
        // Each pointer button retains its identity; canvas clients decide its meaning.
        for (button, index) in [
            (MouseButton::Left, 0),
            (MouseButton::Middle, 1),
            (MouseButton::Right, 2),
            (
                MouseButton::Navigate(gpui_kit::NavigationDirection::Back),
                3,
            ),
            (
                MouseButton::Navigate(gpui_kit::NavigationDirection::Forward),
                4,
            ),
        ] {
            element = element
                .on_mouse_down(
                    button,
                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        if this.action_gesture != gesture || !this.enabled {
                            return;
                        }
                        if this.target_pointer_down(event, window, cx) {
                            cx.stop_propagation();
                            return;
                        }
                        if matches!(button, MouseButton::Navigate(_)) {
                            return;
                        }
                        this.drag = Some(button);
                        if this.enabled && this.drawing.focusable {
                            this.focus.focus(window, cx);
                        }
                        if button == MouseButton::Left
                            && this
                                .columns
                                .begin((event.position.x - this.bounds.origin.x) / px(1.))
                        {
                            cx.notify();
                            cx.stop_propagation();
                            return;
                        }
                        this.pointer(
                            PointerPhase::Down,
                            event.position,
                            index,
                            event.click_count as u8,
                            event.modifiers.shift,
                            cx,
                        );
                        cx.stop_propagation();
                    }),
                )
                .on_mouse_up(
                    button,
                    cx.listener(move |this, event: &MouseUpEvent, _, cx| {
                        if this.action_gesture == gesture
                            && this.enabled
                            && this.target_pointer_up(event, cx)
                        {
                            cx.stop_propagation();
                        }
                    }),
                );
        }
        element = element
            .on_scroll_wheel(cx.listener(move |this, event: &ScrollWheelEvent, _, cx| {
                if this.action_gesture != gesture {
                    return;
                }
                if this.target_wheel(event, cx) {
                    cx.stop_propagation();
                    return;
                }
                // Line wheels on a grid use measured rows, not the smaller font point size.
                let line_height = match &this.measured {
                    Some(CanvasEvent::Resize {
                        grid: Some(grid), ..
                    }) => grid.cell_height,
                    _ => this.font.size_px.unwrap_or(14.),
                };
                let delta = event.delta.pixel_delta(px(line_height));
                let at = event.position - this.bounds.origin;
                let event = if this.drawing.scroll_epoch.is_some()
                    && delta.x == px(0.)
                    && !event.modifiers.shift
                {
                    this.scroll_event(ScrollChange::Delta {
                        delta_y: delta.y / px(1.),
                    })
                } else {
                    CanvasEvent::Wheel {
                        x: at.x / px(1.),
                        y: at.y / px(1.),
                        delta_x: delta.x / px(1.),
                        delta_y: delta.y / px(1.),
                        shift: event.modifiers.shift,
                    }
                };
                this.emit(event, cx);
                cx.stop_propagation();
            }))
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let _ = measure.update(cx, |this, cx| {
                            if this.action_gesture == gesture {
                                this.measure(bounds, window, cx);
                            }
                        });
                    },
                    move |bounds, _, window, cx| {
                        let _ = paint.update(cx, |this, cx| this.paint(bounds, window, cx));
                    },
                )
                .size_full(),
            );
        if self.enabled && self.drawing.column_resize.is_some() {
            element = element.child(self.columns.handles());
        }
        element = element
            .children(self.target_elements())
            .children(self.semantic_elements(cx))
            .children(self.overlay_elements(window, cx));
        // The overlay must paint after the guest canvas, which may fill its entire bounds.
        if self.enabled && self.drawing.scroll.is_some() {
            element = element.child(editor_ui::vertical_viewport_scrollbar(&self.scroll, cx));
        }
        if self.enabled && self.drawing.scroll.is_some() {
            let line_height = match &self.measured {
                Some(CanvasEvent::Resize {
                    grid: Some(grid), ..
                }) => grid.cell_height,
                _ => self.font.size_px.unwrap_or(14.),
            };
            // Content identity retires kinetic motion on replacement. Legacy scenes remain
            // revision-bound; opted-in drawing repaints retain the same shared motion state.
            let identity = self.drawing.scroll_epoch.unwrap_or(self.revision);
            let motion_id = action_gestures::native_id(
                gpui_kit::SharedString::from(format!(
                    "canvas-wheel:{:?}:{identity}",
                    cx.entity_id()
                )),
                gesture,
            );
            editor_ui::scroll::SmoothScroll::new(
                motion_id,
                &self.scroll,
                editor_ui::scroll::ScrollAxis::Vertical,
                element,
            )
            .external_scroll(px(line_height))
            .filter_wheel(|event| {
                !event.modifiers.shift && event.delta.pixel_delta(px(1.)).x == px(0.)
            })
            .into_any_element()
        } else {
            element.into_any_element()
        }
    }
}

impl EntityInputHandler for CanvasView {
    /// Only uncommitted composition lives in the native canvas; committed text is an addressed event.
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let (bytes, units) = composition_range(&self.composition, range);
        *adjusted = Some(units);
        Some(self.composition[bytes].into())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.selection.clone(),
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        (!self.composition.is_empty()).then(|| 0..self.composition.encode_utf16().count())
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.cancel_composition();
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range.unwrap_or(0..self.composition.encode_utf16().count());
        let (bytes, _) = composition_range(&self.composition, range);
        self.composition.replace_range(bytes, text);
        let text = std::mem::take(&mut self.composition);
        self.selection = 0..0;
        self.emit(CanvasEvent::Text { text }, cx);
        cx.notify();
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range.unwrap_or(0..self.composition.encode_utf16().count());
        let (bytes, units) = composition_range(&self.composition, range);
        self.composition.replace_range(bytes, text);
        let end = text.encode_utf16().count();
        let selected = selected.unwrap_or(end..end);
        self.selection = composition_range(
            &self.composition,
            units.start + selected.start..units.start + selected.end,
        )
        .1;
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(rect_bounds(
            self.project(
                self.drawing.caret.unwrap_or(protocol::Rect {
                    x: 0.,
                    y: 0.,
                    w: 1.,
                    h: self.font.size_px.unwrap_or(14.),
                }),
                self.bounds,
            ),
            self.bounds.origin,
        ))
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(0)
    }
}

/// GPUI ranges use UTF-16; expand split surrogate boundaries to whole Unicode scalars.
fn composition_range(text: &str, range: Range<usize>) -> (Range<usize>, Range<usize>) {
    let mut offsets = vec![(0, 0)];
    let mut units = 0;
    for (byte, character) in text.char_indices() {
        units += character.len_utf16();
        offsets.push((byte + character.len_utf8(), units));
    }
    let start = range.start.min(units);
    let end = range.end.max(start).min(units);
    let first = offsets
        .iter()
        .rev()
        .find(|(_, unit)| *unit <= start)
        .unwrap();
    let last = offsets.iter().find(|(_, unit)| *unit >= end).unwrap();
    (first.0..last.0, first.1..last.1)
}
