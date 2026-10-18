//! An empty editor shell exposes application controls without creating a Workspace or its tools.

use super::*;
use crate::ui::controls::{
    menu::{MenuStyle, PopupMenu},
    window::{editor_title_bar, native_window_controls},
};
use gpui_kit::ExternalPaths;
use plugin_runtime::plugin_protocol::ui::{Action as MenuAction, MenuItem};

impl ProjectWindow {
    /// Empty startup retains ordinary editor chrome and an idle document area.
    pub(super) fn empty_shell(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .debug_selector(|| "empty-editor-shell".into())
            .size_full()
            .child(
                editor_title_bar(cx)
                    .child(
                        h_flex()
                            .h_full()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .child(self.empty_application_trigger(cx))
                            .child(self.render_project_menu_trigger(cx)),
                    )
                    .child(
                        div()
                            .id("title-bar-drag-region")
                            .debug_selector(|| "title-bar-drag-region".into())
                            .flex_1()
                            .h_full()
                            .window_control_area(WindowControlArea::Drag)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, _| this.titlebar_should_move = true),
                            )
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, _, _| this.titlebar_should_move = false),
                            )
                            .on_mouse_move(cx.listener(|this, _, window, _| {
                                if std::mem::take(&mut this.titlebar_should_move) {
                                    window.start_window_move();
                                }
                            })),
                    )
                    .child(
                        div()
                            .debug_selector(|| "plugin-manager-trigger".into())
                            .child(
                                Button::new("open-extensions")
                                    .icon(Icon::default().data(include_bytes!(
                                        "../../../assets/plugin-status/plugins.svg"
                                    )))
                                    .small()
                                    .compact()
                                    .ghost()
                                    .tooltip(t!("plugins.manager_title").to_string())
                                    .on_click(
                                        cx.listener(|this, _, _, cx| {
                                            this.open_empty_extensions(cx)
                                        }),
                                    ),
                            ),
                    )
                    .child(app::settings::empty_trigger(cx.entity()))
                    .child(native_window_controls(window, cx)),
            )
            .child(
                v_flex()
                    .id("empty-editor-drop-area")
                    .debug_selector(|| "empty-editor-drop-area".into())
                    .flex_1()
                    .min_h_0()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .drag_over::<ExternalPaths>(|style, _, _, cx| style.bg(cx.theme().list_hover))
                    .on_drag_move(cx.listener(
                        |_, event: &gpui_kit::DragMoveEvent<ExternalPaths>, window, cx| {
                            if event.bounds.contains(&event.event.position)
                                && window.last_input_was_keyboard()
                            {
                                // GPUI accounts modality before converting FileDrop to a pointer event.
                                // Its public mouse route restores hover/drop hit testing after keyboard input.
                                window.dispatch_event(
                                    gpui_kit::PlatformInput::MouseMove(event.event.clone()),
                                    cx,
                                );
                            }
                        },
                    ))
                    .on_drop(cx.listener(Self::open_dropped_path))
                    .child(gpui_kit::img("branding/nanobug.png").size(px(52.)))
                    .child(
                        div()
                            .text_lg()
                            .text_color(cx.theme().muted_foreground)
                            .child(app::APP_NAME),
                    )
                    .child(
                        div()
                            .text_sm()
                            .max_w(px(600.))
                            .px_4()
                            .text_center()
                            .text_color(cx.theme().muted_foreground)
                            .child(t!("projects.empty_editor_hint").to_string()),
                    ),
            )
            .when_some(self.application_popup.as_ref(), |shell, menu| {
                shell.child(menu.clone())
            })
            .when_some(self.render_project_menu(), |shell, menu| shell.child(menu))
    }

    /// A native drop selects one startup target, sharing folder/file validation and trust consent.
    /// GPUI hitboxes exclude visible overlays; a replaced empty surface cannot redirect the current project.
    fn open_dropped_path(
        &mut self,
        paths: &ExternalPaths,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editor.is_some() {
            return;
        }
        // The startup route owns one project; preserve the platform's first offered target.
        // open_path retains its existing close, clone, consent and directory-identity gates.
        if let Some(path) = paths.paths().first() {
            self.open_path(path.clone(), window, cx);
        }
    }

    /// Application-only menu commands never dispatch document or workspace actions.
    fn empty_application_trigger(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let anchor = self.application_anchor.clone();
        div()
            .relative()
            .h_full()
            .flex()
            .items_center()
            .child(
                Button::new("application-menu-trigger")
                    .debug_selector(|| "application-menu-trigger".into())
                    .icon(Icon::default().path("icons/menu.svg"))
                    .small()
                    .compact()
                    .ghost()
                    .tooltip(t!("shortcut_menu.application").to_string())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.project_menu.clear();
                        let owner = cx.entity().downgrade();
                        let items = [
                            ("project-open-another", "projects.open_folder"),
                            ("project-show-home", "projects.home"),
                            ("project-quit-application", "projects.quit_application"),
                        ]
                        .into_iter()
                        .map(|(id, label)| MenuItem {
                            id: id.into(),
                            label: t!(label).to_string(),
                            disabled: false,
                            separator_before: id == "project-quit-application",
                        })
                        .collect();
                        let anchor = this.application_anchor.get();
                        this.application_popup = Some(cx.new(|cx| {
                            PopupMenu::new(
                                items,
                                MenuStyle::current(cx),
                                anchor,
                                move |action, window, cx| {
                                    let owner = owner.clone();
                                    window.defer(cx, move |window, cx| {
                                        let _ = owner.update(cx, |this, cx| {
                                            this.application_popup = None;
                                            match action {
                                                MenuAction::Select(id)
                                                    if id == "project-open-another" =>
                                                {
                                                    this.choose_folder(window, cx)
                                                }
                                                MenuAction::Select(id)
                                                    if id == "project-show-home" =>
                                                {
                                                    this.show_home(window, cx)
                                                }
                                                MenuAction::Select(id)
                                                    if id == "project-quit-application" =>
                                                {
                                                    controller::quit(cx)
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
                            .below_anchor()
                        }));
                        cx.notify();
                    })),
            )
            .child(
                gpui_kit::canvas(
                    move |bounds, _, _| anchor.set(bounds.bottom_left()),
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
    }

    /// The profile manager is created lazily and never registers a project or language providers.
    pub(in crate::app) fn empty_extensions(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Entity<extensions::ExtensionPanel> {
        self.extensions
            .get_or_insert_with(|| {
                cx.new(|cx| extensions::ExtensionPanel::new_application(&self.profile, cx))
            })
            .clone()
    }

    /// Reuse the established manager view and native dialog instead of reproducing management UI.
    pub(super) fn open_empty_extensions(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.extensions_window {
            if handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
            {
                return;
            }
        }
        let manager = self.empty_extensions(cx);
        manager.update(cx, |panel, cx| panel.open_manager(cx));
        let (_, handle) = app_dialog::open_dialog_sized(
            t!("plugins.manager_title").to_string(),
            1080.,
            680.,
            move |content, _, _| content.h_full().child(manager.clone()),
            cx,
        );
        let id = handle.window_id();
        let owner = cx.entity().downgrade();
        self._extensions_closed_subscription = Some(cx.on_window_closed(move |cx, closed| {
            if closed == id {
                let _ = owner.update(cx, |this, cx| {
                    this.extensions_window = None;
                    if let Some(panel) = &this.extensions {
                        panel.update(cx, |panel, cx| panel.close_manager(cx));
                    }
                    cx.notify();
                });
            }
        }));
        self.extensions_window = Some(handle);
        cx.notify();
    }

    /// Dialog closures can retain their root; close them only after close consent is accepted.
    pub(super) fn close_global_dialogs(&mut self, cx: &mut App) {
        for handle in [self.dialog_window.take(), self.extensions_window.take()]
            .into_iter()
            .flatten()
        {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
        self.dialog = None;
        self.plugin_settings = None;
    }
}
