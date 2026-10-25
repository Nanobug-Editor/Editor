//! Terminal-only split leaves reuse the local native resizable control and existing session ownership.

use super::*;
use gpui_kit::AnyElement;

pub(super) use editor_terminal::{PaneLayout, ViewGeometry as Geometry};

impl TerminalPanel {
    /// Capture the active geometry aliases used by existing task and PTY integration.
    fn current_geometry(&self) -> Geometry {
        Geometry {
            width: self.width,
            height: self.height,
            cell_width: self.cell_width,
            cell_height: self.cell_height,
        }
    }

    /// Select one session's measured geometry without resizing or sending process input.
    fn apply_geometry(&mut self, geometry: Geometry) {
        self.width = geometry.width;
        self.height = geometry.height;
        self.cell_width = geometry.cell_width;
        self.cell_height = geometry.cell_height;
    }

    /// Return visible identities, replacing only the previously focused leaf when a hidden tab is chosen.
    pub(super) fn visible_sessions(&mut self) -> Vec<u64> {
        if let Some(layout) = &mut self.panes {
            if let Some(active) = self.active {
                if !layout.sessions.contains(&active) {
                    let index = layout
                        .sessions
                        .iter()
                        .position(|id| *id == layout.focused)
                        .unwrap_or(0);
                    layout.sessions[index] = active;
                }
                layout.focused = active;
            }
            return layout.sessions.clone();
        }
        vec![self.active.unwrap_or(0)]
    }

    /// Create a new Shell leaf after the current visible leaves; failures preserve the old layout.
    pub(super) fn split_shell(&mut self, vertical: bool, cx: &mut Context<Self>) {
        let Some(previous) = self.active else {
            self.new_shell(self.settings.default_profile, cx);
            return;
        };
        let mut sessions = self.visible_sessions();
        self.new_shell(self.settings.default_profile, cx);
        if let Some(active) = self.active.filter(|id| *id != previous) {
            sessions.push(active);
            self.panes = Some(PaneLayout {
                vertical,
                sessions,
                focused: active,
            });
            self.measure_pending = true;
            self.dirty = true;
        }
    }

    /// Collapse the presentation to the selected session; the other tabs and children remain alive.
    pub(super) fn unsplit(&mut self, cx: &mut Context<Self>) {
        self.panes = None;
        self.measure_pending = true;
        self.focus_pending = true;
        self.dirty = true;
        cx.notify();
    }

    /// Address a native event to its immutable session identity, never the last globally selected tab.
    /// Passive resize/focus-out/wheel events restore the user's active leaf after routing.
    fn pane_event(&mut self, id: u64, event: GridEvent, cx: &mut Context<Self>) {
        if !self.sessions.iter().any(|session| session.id == id) {
            return;
        }
        let previous = self.active;
        let old_geometry = self.current_geometry();
        let old_wheel = self.wheel_remainder;
        self.wheel_remainder = self.wheel_remainders.get(&id).copied().unwrap_or(0.);
        let activate = matches!(
            &event,
            GridEvent::Focus { focused: true }
                | GridEvent::Pointer {
                    phase: GridPointerPhase::Down,
                    ..
                }
        );
        let geometry = self.geometry.get(&id).copied().unwrap_or(old_geometry);
        if activate && previous != Some(id) {
            self.link = None;
            self.wheel_remainder = 0.;
        }
        self.active = Some(id);
        self.apply_geometry(geometry);
        self.grid = self.grids.get(&id).cloned();
        self.grid_event(event, cx);
        self.wheel_remainders.insert(id, self.wheel_remainder);
        self.geometry.insert(id, self.current_geometry());
        if !activate && self.active == Some(id) {
            self.active = previous;
            self.apply_geometry(old_geometry);
            if previous != Some(id) {
                self.wheel_remainder = old_wheel;
            }
        } else if previous != self.active {
            self.visible_sessions();
        }
        self.grid = self.active.and_then(|id| self.grids.get(&id).cloned());
    }

    /// Allocate each visible keyed canvas once, preserving focus, composition and pointer capture.
    pub(super) fn reconcile_panes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let visible = self.visible_sessions();
        for id in visible {
            if self.grids.contains_key(&id) {
                continue;
            }
            let owner = cx.entity().downgrade();
            let canvas = cx.new(|cx| {
                TextGrid::new(
                    GridDrawing::default(),
                    move |event, cx| {
                        let _ = owner.update(cx, |panel, cx| panel.pane_event(id, event, cx));
                    },
                    window,
                    cx,
                )
            });
            self.grids.insert(id, canvas);
        }
        self.grid = self.grids.get(&self.active.unwrap_or(0)).cloned();
        for session in &mut self.sessions {
            session.engine.line_height = self.settings.line_height;
            session.engine.search_visible = self.search_open && self.active == Some(session.id);
            if cx.reduce_motion() {
                session.engine.cursor_visible = true;
            }
            session
                .engine
                .set_cursor_style(self.settings.cursor.shape(), self.settings.cursor_blink);
            let focused = self
                .grids
                .get(&session.id)
                .is_some_and(|grid| grid.read(cx).focus_handle().is_focused(window));
            session.engine.set_focused(focused);
        }
        if let Some(geometry) = self.active.and_then(|id| self.geometry.get(&id)).copied() {
            self.apply_geometry(geometry);
        }
    }

    /// Project each visible grid with its own measured dimensions and the common user typography/palette.
    pub(super) fn draw_panes(
        &mut self,
        family: &str,
        font_size: f32,
        colors: &engine::Colors,
        cx: &mut Context<Self>,
    ) {
        let default = self.current_geometry();
        for id in self.visible_sessions() {
            let Some(canvas) = self.grids.get(&id).cloned() else {
                continue;
            };
            let geometry = self.geometry.get(&id).copied().unwrap_or(default);
            let drawing = self
                .sessions
                .iter_mut()
                .find(|session| session.id == id)
                .map(|session| {
                    session.engine.drawing(
                        geometry.width,
                        geometry.height,
                        geometry.cell_width,
                        geometry.cell_height,
                        family,
                        font_size,
                        colors,
                    )
                })
                .unwrap_or_default();
            canvas.update(cx, |canvas, cx| {
                if canvas.drawing != drawing {
                    canvas.drawing = drawing;
                    canvas.foreground = colors.foreground;
                    cx.notify();
                }
            });
        }
    }

    /// Compose native resizable leaves; focus borders and hit testing belong to each retained canvas.
    pub(super) fn pane_content(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let ids = self.visible_sessions();
        let split = self.panes.as_ref().map(|layout| layout.vertical);
        let children = ids
            .into_iter()
            .filter_map(|id| {
                let canvas = self.grids.get(&id)?.clone();
                Some((
                    None,
                    div()
                        .id(("terminal-pane", id))
                        .debug_selector(move || format!("terminal-pane-{id}").into())
                        .size_full()
                        .min_w_0()
                        .min_h_0()
                        .when(split.is_some(), |frame| {
                            frame.border_1().border_color(if self.active == Some(id) {
                                cx.theme().primary
                            } else {
                                cx.theme().border
                            })
                        })
                        .child(canvas)
                        .into_any_element(),
                ))
            })
            .collect::<Vec<_>>();
        if let Some(vertical) = split {
            editor_ui::split::split_container(
                "native-terminal-panes".into(),
                vertical,
                children,
                cx,
            )
        } else {
            children
                .into_iter()
                .next()
                .map(|(_, child)| child)
                .unwrap_or_else(|| div().into_any_element())
        }
    }
}
