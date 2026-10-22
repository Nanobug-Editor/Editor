//! Project management shares one popup between empty and active editor windows.
use super::*;
use editor_ui::menu::{MenuRowContent, MenuStyle, PopupMenu};
use plugin_runtime::plugin_protocol::ui::{Action as MenuAction, MenuItem};
use std::collections::BTreeMap;

/// Immutable choices and popup focus never own a project session.
#[derive(Default)]
pub(crate) struct ProjectMenuState {
    popup: Option<Entity<PopupMenu>>,
    anchor: Rc<Cell<Point<Pixels>>>,
    pointer_focus: Option<FocusHandle>,
    choices: Vec<controller::ProjectChoice>,
    recent_paths: Vec<PathBuf>,
    generation: u64,
}
impl ProjectMenuState {
    /// Superseded callbacks cannot operate a replacement popup.
    pub(crate) fn clear(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.popup = None;
        self.pointer_focus = None;
        self.choices.clear();
        self.recent_paths.clear();
    }
}
/// The whole OS path gives a stable identity even when two projects share a name.
fn choice_id(path: &Path) -> String {
    use std::fmt::Write as _;
    let mut id = String::from("project-choice-");
    for byte in path.as_os_str().as_encoded_bytes() {
        let _ = write!(id, "{byte:02x}");
    }
    id
}
/// Filesystem roots retain a readable full-path name.
fn project_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| editor_core::display_path(path).to_string())
}
/// History removal remains independent of closing the open window.
fn project_menu_rows(
    current: Option<&Path>,
    choices: &[controller::ProjectChoice],
    recent: &[PathBuf],
) -> (
    Vec<MenuItem>,
    BTreeMap<String, MenuRowContent>,
    BTreeMap<String, MenuItem>,
) {
    let mut items = Vec::new();
    let mut content = BTreeMap::new();
    let mut actions = BTreeMap::new();
    for (opened, id, label) in [
        (true, "projects-opened", "projects.opened"),
        (false, "projects-recent", "projects.recent"),
    ] {
        items.push(MenuItem {
            id: id.into(),
            label: t!(label).to_string(),
            disabled: true,
            separator_before: !opened,
        });
        content.insert(id.into(), MenuRowContent::Heading);
        for choice in choices.iter().filter(|choice| choice.opened == opened) {
            let id = choice_id(&choice.path);
            items.push(MenuItem {
                id: id.clone(),
                label: project_name(&choice.path),
                disabled: false,
                separator_before: false,
            });
            content.insert(
                id.clone(),
                MenuRowContent::Detail {
                    secondary: editor_core::display_path(&choice.path).to_string(),
                    checked: current == Some(choice.path.as_path()),
                },
            );
            if recent.contains(&choice.path) {
                actions.insert(
                    id.clone(),
                    MenuItem {
                        id: format!("project-remove-{id}"),
                        label: t!("projects.remove_recent").to_string(),
                        disabled: false,
                        separator_before: false,
                    },
                );
            }
        }
    }
    for (id, label) in [
        ("project-open-folder", "projects.open"),
        ("project-clone", "projects.clone"),
    ] {
        items.push(MenuItem {
            id: id.into(),
            label: t!(label).to_string(),
            disabled: false,
            separator_before: id == "project-open-folder",
        });
    }
    if current.is_some() {
        items.push(MenuItem {
            id: "project-close-project".into(),
            label: t!("projects.close_project").to_string(),
            disabled: false,
            separator_before: true,
        });
    }
    (items, content, actions)
}
/// Both shell states supply navigation metadata; the native owner handles every operation.
pub(crate) trait ProjectMenuHost: Sized + 'static {
    fn menu(&self) -> &ProjectMenuState;
    fn menu_mut(&mut self) -> &mut ProjectMenuState;
    fn project_root(&self) -> Option<&Path>;
    fn project_owner(&self, cx: &Context<Self>) -> Option<WeakEntity<ProjectWindow>>;
    fn project_menu_focus(&self, cx: &Context<Self>) -> FocusHandle;
    fn prepare_project_menu(&mut self) {}

    /// Keep the title button outside the native drag region with one shared appearance.
    fn render_project_menu_trigger(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let anchor = self.menu().anchor.clone();
        let label = self
            .project_root()
            .map(project_name)
            .unwrap_or_else(|| t!("projects.select_folder").to_string());
        let tooltip = self
            .project_root()
            .map(|root| editor_core::display_path(root).to_string())
            .unwrap_or_else(|| t!("projects.select_folder").to_string());
        div()
            .relative()
            .h_full()
            .flex()
            .items_center()
            .min_w_0()
            .capture_any_mouse_down(cx.listener(|app, event: &MouseDownEvent, window, cx| {
                if event.button == MouseButton::Left {
                    app.menu_mut().pointer_focus = window.focused(cx);
                }
            }))
            .child(
                Button::new("project-switcher-trigger")
                    .debug_selector(|| "project-switcher-trigger".into())
                    .small()
                    .ghost()
                    .content_full_width()
                    .max_w(px(240.))
                    .min_w_0()
                    .accessibility_label(format!("{}: {}", t!("projects.home"), tooltip))
                    .tooltip(tooltip)
                    .expanded(self.menu().popup.is_some())
                    .disabled(self.project_owner(cx).is_none())
                    .child(
                        h_flex()
                            .min_w_0()
                            .gap_1()
                            .child(div().min_w_0().truncate().font_semibold().child(label))
                            .child(Icon::new(IconName::ChevronDown).xsmall()),
                    )
                    .on_click(cx.listener(|app, event: &ClickEvent, window, cx| {
                        let focus = if matches!(event, ClickEvent::Mouse(_)) {
                            app.menu_mut().pointer_focus.take()
                        } else {
                            None
                        }
                        .or_else(|| window.focused(cx))
                        .unwrap_or_else(|| app.project_menu_focus(cx));
                        app.open_project_menu(focus, window, cx);
                    })),
            )
            .child(
                gpui_kit::canvas(
                    move |bounds, _, _| anchor.set(bounds.bottom_left() + point(px(0.), px(1.))),
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
    }
    /// Only explicit opening acquires popup focus.
    fn open_project_menu(
        &mut self,
        focus: FocusHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(host) = self.project_owner(cx) else {
            return;
        };
        self.prepare_project_menu();
        self.menu_mut().clear();
        focus.focus(window, cx);
        let choices = controller::project_choices(window.window_handle(), cx);
        let recent = controller::recent_paths(window.window_handle(), cx);
        let (items, content, actions) = project_menu_rows(self.project_root(), &choices, &recent);
        self.menu_mut().choices = choices;
        self.menu_mut().recent_paths = recent;
        let generation = self.menu().generation;
        let owner = cx.entity().downgrade();
        let position = self.menu().anchor.get();
        let footer = if self.project_root().is_some() { 3 } else { 2 };
        let input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("projects.search").to_string()));
        let popup = cx.new(|cx| {
            let mut menu = PopupMenu::new(
                items,
                MenuStyle::current(cx),
                position,
                move |action, window, cx| {
                    let owner = owner.clone();
                    let host = host.clone();
                    window.defer(cx, move |window, cx| {
                        let _ = owner.update(cx, |app, cx| {
                            if app.menu().generation != generation {
                                return;
                            }
                            let id = match action {
                                MenuAction::Select(id) => Some(id),
                                _ => None,
                            };
                            let removal = id
                                .as_deref()
                                .and_then(|id| id.strip_prefix("project-remove-"));
                            let path = id.as_deref().and_then(|id| {
                                app.menu()
                                    .choices
                                    .iter()
                                    .find(|choice| choice_id(&choice.path) == removal.unwrap_or(id))
                                    .map(|choice| choice.path.clone())
                            });
                            let removing = removal.is_some();
                            if !removing {
                                app.menu_mut().clear();
                            }
                            // Release the shell lease before invoking its outer native owner.
                            window.defer(cx, move |window, cx| {
                                let _ = host.update(cx, |host, cx| {
                                    if removing {
                                        if let Some(path) = path {
                                            host.error = host
                                                .recent
                                                .borrow_mut()
                                                .remove(&path)
                                                .err()
                                                .map(|error| error.to_string());
                                            cx.refresh_windows();
                                        }
                                    } else if let Some(path) = path {
                                        if !controller::focus_project(window, &path, cx) {
                                            host.open_path(path, window, cx);
                                        }
                                    } else {
                                        match id.as_deref() {
                                            Some("project-open-folder") => {
                                                host.choose_folder(window, cx)
                                            }
                                            Some("project-clone") => host.show_clone(window, cx),
                                            Some("project-close-project") => {
                                                host.request_close(CloseIntent::Window, window, cx);
                                            }
                                            _ => {}
                                        }
                                    }
                                    cx.notify();
                                });
                            });
                            cx.notify();
                        });
                    });
                },
                window,
                cx,
            )
            .row_content(content)
            .width(420.)
            .max_height(460.)
            .below_anchor()
            .fixed_footer(footer)
            .searchable(input, window, cx);
            menu.row_actions(actions, cx);
            menu
        });
        self.menu_mut().popup = Some(popup);
        cx.notify();
    }
    /// Peer lifecycle and history updates retain the popup's selected directory.
    fn sync_project_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.menu().popup.clone() else {
            return;
        };
        let choices = controller::project_choices(window.window_handle(), cx);
        let recent = controller::recent_paths(window.window_handle(), cx);
        if choices != self.menu().choices || recent != self.menu().recent_paths {
            let (items, content, actions) =
                project_menu_rows(self.project_root(), &choices, &recent);
            self.menu_mut().choices = choices;
            self.menu_mut().recent_paths = recent;
            menu.update(cx, |menu, cx| {
                menu.replace_rows(items, content, cx);
                menu.row_actions(actions, cx);
            });
        }
    }
    fn render_project_menu(&self) -> Option<Entity<PopupMenu>> {
        self.menu().popup.clone()
    }
}
impl ProjectMenuHost for EditorApp {
    fn menu(&self) -> &ProjectMenuState {
        &self.project_menu
    }
    fn menu_mut(&mut self) -> &mut ProjectMenuState {
        &mut self.project_menu
    }
    fn project_root(&self) -> Option<&Path> {
        Some(self.workspace.root())
    }
    fn project_owner(&self, _: &Context<Self>) -> Option<WeakEntity<ProjectWindow>> {
        self.project_window.clone()
    }
    fn project_menu_focus(&self, cx: &Context<Self>) -> FocusHandle {
        self.editor.focus_handle(cx)
    }
    fn prepare_project_menu(&mut self) {
        self.application_menu.clear();
    }
}
impl ProjectMenuHost for ProjectWindow {
    fn menu(&self) -> &ProjectMenuState {
        &self.project_menu
    }
    fn menu_mut(&mut self) -> &mut ProjectMenuState {
        &mut self.project_menu
    }
    fn project_root(&self) -> Option<&Path> {
        None
    }
    fn project_owner(&self, cx: &Context<Self>) -> Option<WeakEntity<ProjectWindow>> {
        Some(cx.entity().downgrade())
    }
    fn project_menu_focus(&self, _: &Context<Self>) -> FocusHandle {
        self.focus.clone()
    }
    fn prepare_project_menu(&mut self) {
        self.application_popup = None;
    }
}
