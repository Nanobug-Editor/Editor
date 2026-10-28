//! Minimal real local controls used to distinguish frame pacing from application workload.

use super::*;
use gpui_base::ElementExt as _;
use gpui_base::dock::{DockArea, DockPlacement, Panel, PanelEvent};
use gpui_kit::{AnyElement, App, Entity, EventEmitter, FocusHandle, Focusable};
use std::sync::Arc;

/// Inert parent gives the dialog the same native ownership semantics as a settings window.
pub(super) struct Parent;
impl Render for Parent {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(gpui_kit::rgb(0x304050))
    }
}

/// One measured content pane preserves its actual allocated bounds across cached frames.
struct Pane {
    /// The original Base panel focus owner.
    focus: FocusHandle,
    /// Measured content excludes the title, making its grab position unambiguous.
    geometry: Rc<Cell<Bounds<Pixels>>>,
    /// Optional real source workload for width-sensitive resize acceptance.
    editor: Option<Entity<gpui_base::input::EditorState>>,
}
impl EventEmitter<PanelEvent> for Pane {}
impl Focusable for Pane {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Panel for Pane {
    fn panel_name(&self) -> &'static str {
        "RefreshAcceptance"
    }
}
impl crate::dock::Panel for Pane {
    fn title(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        "刷新率 / refresh"
    }
}
impl Render for Pane {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        if let Some(editor) = &self.editor {
            let geometry = self.geometry.clone();
            div()
                .size_full()
                .on_prepaint(move |bounds, _, _| geometry.set(bounds))
                .child(crate::source_editor::Editor::new(editor))
                .into_any_element()
        } else {
            pane(self.geometry.clone()).into_any_element()
        }
    }
}

/// Production local control and read-only measurement state, with no duplicate layout model.
pub(super) struct Surface {
    /// Requested acceptance case.
    pub kind: String,
    /// First pane's actual layout allocation.
    pub geometry: Rc<Cell<Bounds<Pixels>>>,
    /// Original Base area for dock title and edge gestures.
    dock: Option<Entity<DockArea>>,
    /// Reuse one native document throughout the gesture; never regenerate text per frame.
    editor: Option<Entity<gpui_base::input::EditorState>>,
}
impl Surface {
    /// Build the selected surface in `window`; the native cases need only visible content.
    pub fn new(
        kind: String,
        geometry: Rc<Cell<Bounds<Pixels>>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let source = if let Some(path) = std::env::var_os("NANOBUG_DRAG_SOURCE_PATH") {
            Some(std::fs::read_to_string(path).expect("source acceptance file"))
        } else {
            std::env::var("NANOBUG_DRAG_SOURCE_ROWS").ok().map(|rows| {
                let rows = rows.parse::<usize>().expect("source row count");
                assert!((1..=10_000).contains(&rows));
                (0..rows)
                    .map(|row| format!("fn row_{row:04}() {{ let message = \"原生编辑器 resize 中文😀 wrapped source\"; }}\n"))
                    .collect::<String>()
            })
        };
        let editor = source.map(|text| {
            cx.new(|cx| {
                gpui_base::input::EditorState::new(window, cx)
                    .line_number(true)
                    .default_value(text)
            })
        });
        let dock = if kind.starts_with("dock") {
            let area =
                crate::dock::LocalDock::new(28.).create_area("refresh-dock", None, window, cx);
            area.update(cx, |area, cx| {
                for (placement, measurement) in [
                    (DockPlacement::Left, geometry.clone()),
                    (DockPlacement::Center, Rc::new(Cell::new(Bounds::default()))),
                ] {
                    let panel = cx.new(|cx| Pane {
                        focus: cx.focus_handle(),
                        geometry: measurement,
                        editor: (placement == DockPlacement::Center)
                            .then(|| editor.clone())
                            .flatten(),
                    });
                    crate::dock::add_panel_view(
                        area,
                        Arc::new(panel),
                        placement,
                        Some(px(230.)),
                        window,
                        cx,
                    );
                }
            });
            Some(area)
        } else {
            None
        };
        Self {
            kind,
            geometry,
            dock,
            editor,
        }
    }

    /// Compose actual local controls; input and state remain their unmodified production implementations.
    pub fn content(&self, _: &mut Window, cx: &mut App) -> AnyElement {
        if let Some(dock) = &self.dock {
            return dock.clone().into_any_element();
        }
        if self.kind.starts_with("split") {
            return crate::split_container(
                "refresh-split".into(),
                self.kind == "split-vertical",
                vec![
                    (Some(230.), pane(self.geometry.clone()).into_any_element()),
                    (
                        None,
                        div()
                            .size_full()
                            .bg(gpui_kit::rgb(0x405060))
                            .children(self.editor.as_ref().map(crate::source_editor::Editor::new))
                            .into_any_element(),
                    ),
                ],
                cx,
            );
        }
        pane(self.geometry.clone()).into_any_element()
    }
}

/// Capture actual layout while painting enough text and color to inspect native content visually.
fn pane(geometry: Rc<Cell<Bounds<Pixels>>>) -> impl IntoElement {
    div()
        .size_full()
        .bg(gpui_kit::rgb(0x203040))
        .text_color(gpui_kit::rgb(0xf0f0f0))
        .on_prepaint(move |bounds, _, _| geometry.set(bounds))
        .child("Nanobug — 拖动刷新验证 / native drag refresh")
}
