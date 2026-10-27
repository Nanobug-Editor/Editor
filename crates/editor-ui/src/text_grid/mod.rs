//! Native fixed-cell text surfaces with GPUI input, IME and the shared Base scrollbar behavior.
//! The owner supplies drawing and scroll state; this control owns only transient native interaction.

use gpui_kit::{
    App, Bounds, Context, ElementInputHandler, EntityInputHandler, FocusHandle, Focusable,
    FontWeight, InteractiveElement, IntoElement, KeyDownEvent, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render,
    ScrollEvent, Styled, Subscription, TextRun, UTF16Selection, Window, canvas, div, fill, font,
    point, px, rgb, size,
};
use std::{ops::Range, rc::Rc};

mod input;
mod scroll;
#[cfg(test)]
mod tests;

/// A rectangle in grid-local logical pixels, independent from any plugin protocol.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GridRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Native grid typography. Layout metrics are measured from this exact font on the window.
#[derive(Clone, Debug, PartialEq)]
pub struct GridFont {
    pub family: String,
    pub size_px: f32,
    /// Native line-height multiplier; shared by grid measurement, glyph paint and preedit.
    pub line_height: f32,
}
impl Default for GridFont {
    fn default() -> Self {
        Self {
            family: "Cascadia Mono".into(),
            size_px: 14.,
            line_height: 1.45,
        }
    }
}

/// Fixed cell measurements, in logical pixels, supplied with each real viewport resize.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridMetrics {
    pub cell_width: f32,
    pub cell_height: f32,
}

/// Drawing operations for native cell grids; style and geometry have no serialization/revision layer.
#[derive(Clone, Debug, PartialEq)]
pub enum GridPaint {
    /// Fill a grid-local rectangle; background fills can follow the live viewport bottom during resize.
    Fill {
        rect: GridRect,
        color: u32,
        extend_to_bottom: bool,
    },
    /// Shape a native glyph/run at the given cell origin. Colors are RGB values.
    Text {
        x: f32,
        y: f32,
        text: String,
        color: u32,
        size: f32,
        bold: bool,
        italic: bool,
        font: Option<String>,
    },
}

/// Owner-controlled vertical scroll extent and absolute offset, both in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridScroll {
    pub content: f32,
    pub offset: f32,
}

/// The current native projection; the owner remains the sole terminal/document state authority.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GridDrawing {
    pub font: GridFont,
    pub paint: Vec<GridPaint>,
    /// IME candidate/preedit anchor, stable even while a blinking cursor is invisible.
    pub caret: Option<GridRect>,
    pub scroll: Option<GridScroll>,
}

/// Pointer gesture phases preserve capture beyond the original grid bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridPointerPhase {
    Down,
    Move,
    Up,
}

/// Native events sent directly to the owning in-process view, including complete pointer modifiers.
#[derive(Clone, Debug, PartialEq)]
pub enum GridEvent {
    Resize {
        width: f32,
        height: f32,
        grid: Option<GridMetrics>,
    },
    Focus {
        focused: bool,
    },
    Key {
        key: String,
        ctrl: bool,
        alt: bool,
        shift: bool,
    },
    Text {
        text: String,
    },
    /// Buttons use left=0, middle=1, right=2, no pressed button=3.
    Pointer {
        phase: GridPointerPhase,
        x: f32,
        y: f32,
        button: u8,
        clicks: u8,
        shift: bool,
        modifiers: Modifiers,
    },
    Wheel {
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
        shift: bool,
        modifiers: Modifiers,
    },
    Scroll {
        offset: f32,
    },
    /// File drops are data only; the owner decides how to quote or use their paths.
    Files {
        paths: Vec<std::path::PathBuf>,
    },
}

/// A retained native grid, with one focus/IME owner and no mutable copy of terminal content.
pub struct TextGrid {
    pub drawing: GridDrawing,
    pub foreground: u32,
    focus: FocusHandle,
    bounds: Bounds<Pixels>,
    measured: Option<GridEvent>,
    composition: String,
    selection: Range<usize>,
    drag: Option<MouseButton>,
    scroll: scroll::GridScrollbar,
    sink: Rc<dyn Fn(GridEvent, &mut App)>,
    _subscriptions: Vec<Subscription>,
}

impl TextGrid {
    /// Mount an owner-supplied drawing and direct event sink in `window`; no process is launched here.
    pub fn new(
        drawing: GridDrawing,
        sink: impl Fn(GridEvent, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        let subscriptions = vec![
            cx.on_focus(&focus, window, |grid, _, cx| {
                grid.emit(GridEvent::Focus { focused: true }, cx)
            }),
            cx.on_blur(&focus, window, |grid, _, cx| {
                grid.drag = None;
                grid.composition.clear();
                grid.selection = 0..0;
                grid.emit(GridEvent::Focus { focused: false }, cx);
                cx.notify();
            }),
        ];
        Self {
            drawing,
            foreground: 0xffffff,
            focus,
            bounds: Bounds::default(),
            measured: None,
            composition: String::new(),
            selection: 0..0,
            drag: None,
            scroll: Default::default(),
            sink: Rc::new(sink),
            _subscriptions: subscriptions,
        }
    }

    /// Request a new measurement after font changes or a session becomes visible again.
    pub fn invalidate_measurement(&mut self) {
        self.measured = None;
    }

    /// Check preedit ownership before a parent routes shortcuts; candidate keystrokes stay with the IME.
    pub fn is_composing(&self) -> bool {
        !self.composition.is_empty()
    }

    /// Return the existing input focus handle; no other entity owns committed terminal text.
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    /// Translate a native position inside this exact grid to local logical pixels.
    pub fn local_position(&self, position: Point<Pixels>) -> Option<(f32, f32)> {
        self.bounds.contains(&position).then(|| {
            let at = position - self.bounds.origin;
            (at.x / px(1.), at.y / px(1.))
        })
    }

    /// Dispatch an event synchronously to the owner; grid text is never interpreted here.
    fn emit(&self, event: GridEvent, cx: &mut Context<Self>) {
        (self.sink)(event, cx);
    }

    /// Measure native font metrics and the shared scrollbar, emitting only geometry changes.
    fn measure(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        self.bounds = bounds;
        self.scroll.update(bounds, self.drawing.scroll);
        if let Some(offset) = self.scroll.take_offset() {
            self.emit(GridEvent::Scroll { offset }, cx);
        }
        let face = font(self.drawing.font.family.clone());
        let id = window.text_system().resolve_font(&face);
        let size = self.drawing.font.size_px;
        let grid = GridMetrics {
            cell_width: window
                .text_system()
                .advance(id, px(size), 'M')
                .map(|advance| advance.width / px(1.))
                .unwrap_or(8.4)
                .max(1.),
            cell_height: (size * self.drawing.font.line_height).ceil().max(1.),
        };
        let event = GridEvent::Resize {
            width: bounds.size.width / px(1.),
            height: bounds.size.height / px(1.),
            grid: Some(grid),
        };
        if self.measured.as_ref() != Some(&event) {
            self.measured = Some(event.clone());
            self.emit(event, cx);
        }
    }

    /// Submit native quads and shaped text, retaining a stable input handler and preedit anchor.
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        self.capture_pointer(window, cx);
        if self.focus.is_focused(window) {
            window.handle_input(
                &self.focus,
                ElementInputHandler::new(bounds, cx.entity()),
                cx,
            );
        }
        for operation in &self.drawing.paint {
            match operation {
                GridPaint::Fill {
                    rect,
                    color,
                    extend_to_bottom,
                } => {
                    let mut rect = *rect;
                    if *extend_to_bottom {
                        rect.h = (bounds.size.height / px(1.) - rect.y).max(0.);
                    }
                    window.paint_quad(fill(rect_bounds(rect, bounds.origin), rgb(*color)));
                }
                GridPaint::Text {
                    x,
                    y,
                    text,
                    color,
                    size,
                    bold,
                    italic,
                    font: family,
                } => {
                    let mut face = font(
                        family
                            .clone()
                            .unwrap_or_else(|| self.drawing.font.family.clone()),
                    );
                    if *bold {
                        face.weight = FontWeight::BOLD;
                    }
                    if *italic {
                        face.style = gpui_kit::FontStyle::Italic;
                    }
                    paint_text(
                        text,
                        *size,
                        self.drawing.font.line_height,
                        face,
                        *color,
                        bounds.origin + point(px(*x), px(*y)),
                        window,
                        cx,
                    );
                }
            }
        }
        if !self.composition.is_empty() {
            let caret = self.drawing.caret.unwrap_or_default();
            paint_text(
                &self.composition,
                self.drawing.font.size_px,
                self.drawing.font.line_height,
                font(self.drawing.font.family.clone()),
                self.foreground,
                bounds.origin + point(px(caret.x), px(caret.y)),
                window,
                cx,
            );
        }
    }
}

impl Focusable for TextGrid {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TextGrid {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let measure = cx.entity().downgrade();
        let paint = measure.clone();
        let mut frame = div()
            .id("native-text-grid")
            .size_full()
            .relative()
            .overflow_hidden()
            .track_focus(&self.focus)
            .on_drop(
                cx.listener(|grid, paths: &gpui_kit::ExternalPaths, window, cx| {
                    grid.focus.focus(window, cx);
                    grid.emit(
                        GridEvent::Files {
                            paths: paths.paths().to_vec(),
                        },
                        cx,
                    );
                }),
            )
            .on_key_down(cx.listener(|grid, event: &KeyDownEvent, window, cx| {
                if !grid.focus.is_focused(window)
                    || grid.is_composing()
                    || event.prefer_character_input
                {
                    return;
                }
                let key = &event.keystroke;
                grid.emit(
                    GridEvent::Key {
                        key: key.key.clone(),
                        ctrl: key.modifiers.control,
                        alt: key.modifiers.alt,
                        shift: key.modifiers.shift,
                    },
                    cx,
                );
                if key.modifiers.control
                    || key.modifiers.alt
                    || (key.key.chars().count() > 1 && key.key != "space")
                {
                    cx.stop_propagation();
                }
            }))
            .on_mouse_move(cx.listener(|grid, event: &MouseMoveEvent, _, cx| {
                if event.pressed_button.is_none() {
                    grid.pointer(
                        GridPointerPhase::Move,
                        event.position,
                        3,
                        0,
                        event.modifiers,
                        cx,
                    );
                }
            }))
            .on_scroll(cx.listener(|grid, event: &ScrollEvent, _, cx| {
                let height = match &grid.measured {
                    Some(GridEvent::Resize {
                        grid: Some(metrics),
                        ..
                    }) => metrics.cell_height,
                    _ => 21.,
                };
                let delta = event.delta.pixel_delta(px(height));
                let at = event.position - grid.bounds.origin;
                grid.emit(
                    GridEvent::Wheel {
                        x: at.x / px(1.),
                        y: at.y / px(1.),
                        delta_x: delta.x / px(1.),
                        delta_y: delta.y / px(1.),
                        shift: event.modifiers.shift,
                        modifiers: event.modifiers,
                    },
                    cx,
                );
                cx.stop_propagation();
            }));
        for (button, index) in [
            (MouseButton::Left, 0),
            (MouseButton::Middle, 1),
            (MouseButton::Right, 2),
        ] {
            frame = frame.on_mouse_down(
                button,
                cx.listener(move |grid, event: &MouseDownEvent, window, cx| {
                    grid.drag = Some(button);
                    grid.focus.focus(window, cx);
                    grid.pointer(
                        GridPointerPhase::Down,
                        event.position,
                        index,
                        event.click_count as u8,
                        event.modifiers,
                        cx,
                    );
                    cx.stop_propagation();
                }),
            );
        }
        frame = frame.child(
            canvas(
                move |bounds, window, cx| {
                    let _ = measure.update(cx, |grid, cx| grid.measure(bounds, window, cx));
                },
                move |bounds, _, window, cx| {
                    let _ = paint.update(cx, |grid, cx| grid.paint(bounds, window, cx));
                },
            )
            .size_full(),
        );
        if self.drawing.scroll.is_some() {
            frame = frame.child(crate::vertical_viewport_scrollbar(&self.scroll, cx));
        }
        frame
    }
}

/// Convert a grid-local rectangle to actual native window bounds.
fn rect_bounds(rect: GridRect, origin: Point<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        origin + point(px(rect.x), px(rect.y)),
        size(px(rect.w), px(rect.h)),
    )
}

/// Shape native text with explicit style; the cell owner supplies grid spacing and glyph origins.
fn paint_text(
    text: &str,
    size: f32,
    line_height: f32,
    face: gpui_kit::Font,
    color: u32,
    origin: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let run = TextRun {
        len: text.len(),
        font: face,
        color: rgb(color).into(),
        ..Default::default()
    };
    let line = window
        .text_system()
        .shape_line(text.to_owned().into(), px(size), &[run], None);
    let _ = line.paint(
        origin,
        px((size * line_height).ceil()),
        gpui_kit::TextAlign::Left,
        None,
        window,
        cx,
    );
}
