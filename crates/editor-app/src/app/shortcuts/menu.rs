//! Grouped project and application commands preserve the active panel across popup focus transfers.

use super::ShortcutOrigin;
use crate::ui::controls::menu::{MenuStyle, PopupMenu};
use crate::*;
use plugin_runtime::plugin_protocol::ui::{Action as MenuAction, MenuItem};

/// Retain one menu and its original dispatch context independently of shortcut bindings.
#[derive(Default)]
pub(crate) struct MenuState {
    popup: Option<Entity<PopupMenu>>,
    origin: Option<ShortcutOrigin>,
    pointer_origin: Option<ShortcutOrigin>,
    anchor: Rc<Cell<Point<Pixels>>>,
    generation: u64,
}

impl MenuState {
    /// Remove a superseded menu and invalidate deferred callbacks without moving keyboard focus.
    pub(crate) fn clear(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.popup = None;
        self.origin = None;
        self.pointer_origin = None;
    }
}

impl EditorApp {
    /// Draw the persistent application-menu trigger using the local Base-backed button.
    ///
    /// Capture pointer context before Base focuses its trigger. Keyboard activation captures
    /// the currently focused context rather than reusing an abandoned pointer gesture.
    pub(crate) fn render_application_menu_trigger(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let anchor = self.application_menu.anchor.clone();
        div()
            .relative()
            .h_full()
            .flex()
            .items_center()
            .capture_any_mouse_down(cx.listener(|app, event: &MouseDownEvent, window, cx| {
                if event.button == MouseButton::Left {
                    app.application_menu.pointer_origin =
                        Some(ShortcutOrigin::capture(app, window, cx));
                }
            }))
            .child(
                Button::new("application-menu-trigger")
                    .debug_selector(|| "application-menu-trigger".into())
                    // The selected catalog SVG uses local icon sizing and the current theme's foreground.
                    .icon(Icon::default().path("icons/menu.svg"))
                    .accessibility_label(t!("shortcut_menu.application").to_string())
                    .tooltip(t!("shortcut_menu.application").to_string())
                    .expanded(self.application_menu.popup.is_some())
                    .small()
                    .compact()
                    .ghost()
                    .on_click(cx.listener(|app, event: &ClickEvent, window, cx| {
                        let origin = if matches!(event, ClickEvent::Mouse(_)) {
                            app.application_menu.pointer_origin.take()
                        } else {
                            app.application_menu.pointer_origin = None;
                            None
                        }
                        .unwrap_or_else(|| ShortcutOrigin::capture(app, window, cx));
                        let anchor = app.application_menu.anchor.get();
                        app.open_application_menu_from(origin, anchor, window, cx);
                    })),
            )
            // Anchor to measured geometry so window scaling or label translation cannot move
            // the popup over its trigger. The canvas adds no independent hit target.
            .child(
                gpui_kit::canvas(
                    move |bounds, _, _| anchor.set(bounds.bottom_left()),
                    |_, _, _, _| {},
                )
                .absolute()
                // Explicit insets avoid the absolute canvas's static position after the button.
                .inset_0(),
            )
    }

    /// Return the retained popup for the shell's overlay layer; rendering never opens a menu.
    pub(crate) fn render_application_menu(&self) -> Option<Entity<PopupMenu>> {
        self.application_menu.popup.clone()
    }

    /// Keep project, keyboard lookup and application-exit groups in one translated identity list.
    /// Standalone embedded shells cannot operate a project owner, so those rows remain disabled.
    fn application_menu_items(&self) -> Vec<MenuItem> {
        let has_project = self.project_window.is_some();
        [
            (
                "project-open-another",
                "projects.open_folder",
                !has_project,
                false,
            ),
            ("project-show-home", "projects.home", !has_project, false),
            ("item-shortcuts", "shortcuts.title", false, true),
            (
                "project-quit-application",
                "projects.quit_application",
                false,
                true,
            ),
        ]
        .into_iter()
        .map(|(id, label, disabled, separator_before)| MenuItem {
            id: id.into(),
            label: t!(label).to_string(),
            disabled,
            separator_before,
        })
        .collect()
    }

    /// Open one native dropdown with the original panel retained across menu and lookup transitions.
    fn open_application_menu_from(
        &mut self,
        origin: ShortcutOrigin,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.project_menu.clear();
        self.application_menu.clear();
        // PopupMenu restores its previous focus before delivering a selection or dismissal.
        // Seed that previous handle with the actual panel, not the pointer-focused trigger.
        origin.focus.focus(window, cx);
        self.application_menu.origin = Some(origin);
        let generation = self.application_menu.generation;
        let owner = cx.entity().downgrade();
        let items = self.application_menu_items();
        self.application_menu.popup = Some(cx.new(|cx| {
            PopupMenu::new(
                items,
                MenuStyle::current(cx),
                position,
                move |action, window, cx| {
                    let owner = owner.clone();
                    let selected = match action {
                        MenuAction::Select(id) => Some(id),
                        _ => None,
                    };
                    // Removing an entity from inside its own menu event can cause a reentrant
                    // borrow; finish the menu's focus restoration before changing shell state.
                    window.defer(cx, move |window, cx| {
                        let _ = owner.update(cx, |app, cx| {
                            if app.application_menu.generation != generation {
                                return;
                            }
                            let origin = app.application_menu.origin.take();
                            app.application_menu.clear();
                            match selected.as_deref() {
                                Some("item-shortcuts") => {
                                    if let Some(origin) = origin {
                                        app.open_shortcuts_from(origin, window, cx);
                                    }
                                }
                                Some("project-open-another") => {
                                    if let Some(owner) = app.project_window.clone() {
                                        // Empty projects have no document focus path to bubble an action.
                                        // Release this editor lease before invoking the production owner.
                                        window.defer(cx, move |window, cx| {
                                            let _ = owner.update(cx, |host, cx| {
                                                host.choose_folder(window, cx)
                                            });
                                        });
                                    }
                                }
                                Some("project-show-home") => {
                                    if let Some(owner) = app.project_window.clone() {
                                        window.defer(cx, move |window, cx| {
                                            let _ = owner
                                                .update(cx, |host, cx| host.show_home(window, cx));
                                        });
                                    }
                                }
                                Some("project-quit-application") => {
                                    if app.project_window.is_some() {
                                        cx.defer(app::projects::quit_application);
                                    } else {
                                        app.shutdown_plugins(cx);
                                    }
                                }
                                _ => {}
                            }
                            cx.notify();
                        });
                    });
                },
                window,
                cx,
            )
            .width(230.)
            .below_anchor()
        }));
        cx.notify();
    }
}
