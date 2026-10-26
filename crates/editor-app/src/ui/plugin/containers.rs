//! Recursive containers and editor references have a small stack independent of leaf control complexity.
use super::*;
use editor_ui::scroll::ScrollableElement as _;
use gpui_base::{Tab, Tabs};
use gpui_kit::{
    AnyElement, InteractiveElement, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, div, prelude::FluentBuilder as _, px,
};
use plugin_runtime::plugin_protocol::ui::Node;

impl PluginView {
    /// Borrow native editor state or arrange/scroll children through generic publicly declared geometry.
    pub(super) fn layout_node(
        &mut self,
        node: &Node,
        disabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let native_id = SharedString::from(format!("plugin-ui-{}", node.id));
        let colors = self.colors(node.theme_role(), cx);
        match &node.kind {
            Kind::Editor(editor) => {
                let current = editor
                    .document_version()
                    .is_none_or(|document| self.code_action_native_current(document, window, cx));
                let actions = (editor.line_actions.is_some()).then(|| {
                    editor_ui::EditorActionElements::new(
                        editor
                            .left_actions()
                            .iter()
                            .map(|action| {
                                (
                                    action.offset,
                                    self.node(&action.content, disabled || !current, window, cx),
                                )
                            })
                            .collect(),
                        editor
                            .right_actions()
                            .iter()
                            .map(|action| {
                                (
                                    action.offset,
                                    self.node(&action.content, disabled || !current, window, cx),
                                )
                            })
                            .collect(),
                    )
                });
                let source = match &editor.source {
                    ui::EditorSource::Document { document } => self
                        .native_editor
                        .as_ref()
                        .filter(|_| !disabled)
                        .and_then(|render| {
                            render(
                                document,
                                actions,
                                self.code_alignment_pane(&node.id),
                                window,
                                cx,
                            )
                        })
                        .unwrap_or_else(|| div().into_any_element()),
                    ui::EditorSource::Snapshot { .. } => {
                        self.render_editor_snapshot(&node.id, actions, window, cx)
                    }
                };
                self.code_sync_pane(&node.id, source)
            }
            Kind::Column { children } | Kind::Row { children } => {
                let initial_sizes = children
                    .iter()
                    .map(|child| {
                        if matches!(node.kind, Kind::Column { .. }) {
                            child.layout.height
                        } else {
                            child.layout.width
                        }
                    })
                    .collect::<Vec<_>>();
                let children: Vec<_> = children
                    .iter()
                    .map(|n| self.node(n, disabled, window, cx))
                    .collect();
                if node.layout.resizable {
                    editor_ui::split_container(
                        SharedString::from(node.id.clone()),
                        matches!(node.kind, Kind::Column { .. }),
                        initial_sizes.into_iter().zip(children).collect(),
                        cx,
                    )
                } else {
                    div()
                        .flex()
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .gap(px(node.layout.gap))
                        .when(matches!(node.kind, Kind::Column { .. }), |v| v.flex_col())
                        .when(
                            matches!(node.kind, Kind::Row { .. }) && node.layout.wrap,
                            |v| v.flex_wrap(),
                        )
                        .when_some(node.layout.align, |view, align| {
                            // The public alignment is horizontal even when the container is a column.
                            match (matches!(node.kind, Kind::Row { .. }), align) {
                                (true, ui::Alignment::Start) => view.justify_start(),
                                (true, ui::Alignment::Center) => view.justify_center(),
                                (true, ui::Alignment::End) => view.justify_end(),
                                (false, ui::Alignment::Start) => view.items_start(),
                                (false, ui::Alignment::Center) => view.items_center(),
                                (false, ui::Alignment::End) => view.items_end(),
                            }
                        })
                        .children(children)
                        .into_any_element()
                }
            }
            Kind::Scroll { content } => {
                // Materialize mapped content near the viewport while retaining native reveal targets.
                let content = self.windowed_content(content, &node.id);
                let child = self.node(&content, disabled, window, cx);
                let handle = self.scrolls.get(&node.id).cloned().unwrap_or_default();
                div()
                    .id(native_id.clone())
                    .relative()
                    .size_full()
                    .min_h_0()
                    .overflow_y_scrollbar_with(&handle)
                    .child(child)
                    .into_any_element()
            }
            Kind::Tabs { tabs, selected } => {
                let gesture = self.action_gesture(&node.id);
                let mut strip = Tabs::new(native_id.clone())
                    .flex()
                    .gap_1()
                    .border_b_1()
                    .border_color(colors.border);
                for (index, tab) in tabs.iter().enumerate() {
                    let node_id = node.id.clone();
                    let tab_id = tab.id.clone();
                    strip = strip.child(
                        Tab::new(action_gestures::native_id(
                            format!("{}-tab-{index}", node.id),
                            gesture,
                        ))
                        .selected(&tab.id == selected)
                        .debug_selector(move || format!("plugin-tab-{index}"))
                        .disabled(disabled)
                        .accessibility_label(tab.label.clone())
                        .set_position(index + 1, tabs.len())
                        .px_3()
                        .py_2()
                        .bg(if &tab.id == selected {
                            colors.active
                        } else {
                            colors.background
                        })
                        .hover(|s| s.bg(colors.hover))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if this.action_gesture_current(&node_id, gesture) {
                                this.emit_in(&node_id, Action::Select(tab_id.clone()), window, cx);
                            }
                        }))
                        .child(tab.label.clone()),
                    );
                }
                // Overflowing guest tab labels use the same native horizontal viewport as host
                // tabs; Base keeps selection/focus semantics and the UI layer owns wheel motion.
                let mut body = div().flex().flex_col().gap_2().child(
                    div()
                        .id(gpui_kit::ElementId::NamedChild(
                            gpui_kit::ElementId::Name(native_id.clone()).into(),
                            "tab-scroll".into(),
                        ))
                        .relative()
                        .flex()
                        .w_full()
                        .min_w_0()
                        .overflow_x_scrollbar()
                        .child(strip.flex_shrink_0()),
                );
                if let Some(tab) = tabs.iter().find(|t| &t.id == selected) {
                    body = body.child(self.node(&tab.content, disabled, window, cx));
                }
                body.into_any_element()
            }
            _ => unreachable!("layout kinds are dispatched separately"),
        }
    }
}
