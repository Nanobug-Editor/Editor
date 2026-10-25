//! Built-in terminal sessions: native UI and upstream VT state, independent of installed packages.

pub(crate) mod configurations;
mod host;
mod interaction;
mod io;
mod navigation;
mod panes;
mod persistence;
mod presentations;
mod search;
pub(crate) mod task_view;
mod tasks;
#[cfg(test)]
pub(crate) mod tests;
pub(crate) mod upgrade;
mod view;

use crate::*;
use config::{Profile, Settings};
use editor_terminal::{config, engine, input, shell};
use editor_ui::text_grid::{GridDrawing, GridEvent, GridPaint, GridPointerPhase, TextGrid};
use engine::{Engine, GridSize};
use gpui_kit::ClipboardItem;
use plugin_runtime::{
    native_processes::{NativeLaunch, NativeProcessEvent, NativeProcessGroup},
    plugin_protocol::{
        self as protocol,
        process::{Transport, Update},
        ui::{Action, SideTab, SideTabs},
    },
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Native/OS details retain their diagnostic content inside a localized operation message.
enum FailureKind {
    Restore,
    Save,
    Settings,
    Process,
}

/// The library owns terminal state; this adapter supplies only host Run/Debug metadata.
type Session = editor_terminal::Session<tasks::Task>;

/// This window owns terminal policy; the process supervisor owns only controlled system resources.
pub(crate) struct TerminalPanel {
    parent: WeakEntity<EditorApp>,
    /// IO can be issued during the parent's update without re-borrowing that same EditorApp.
    io_host: Entity<extensions::ExtensionPanel>,
    workspace: PathBuf,
    storage: PathBuf,
    settings: Settings,
    sessions: Vec<Session>,
    active: Option<u64>,
    next_id: u64,
    visible: bool,
    trusted: bool,
    focus: FocusHandle,
    _focus_subscription: Subscription,
    group: Option<WeakEntity<gpui_base::dock::TabGroup>>,
    supervisor: NativeProcessGroup,
    grid: Option<Entity<TextGrid>>,
    /// Inspection entities are presentation only; hiding/switching a tab keeps their focus/scroll state.
    inspections: BTreeMap<String, Entity<crate::run::ui::panel::InspectionView>>,
    sidebar: Option<Entity<editor_ui::side_tabs::SideTabBar>>,
    width: f32,
    height: f32,
    cell_width: f32,
    cell_height: f32,
    tab_width: f32,
    preview_width: Rc<Cell<Option<f32>>>,
    popup: Option<Entity<editor_ui::menu::PopupMenu>>,
    requested_menu: Option<bool>,
    error: Option<String>,
    dirty: bool,
    last_save: Instant,
    /// A new tab needs a fresh measurement even when the preceding tab had identical geometry.
    measure_pending: bool,
    focus_pending: bool,
    settings_stamp: Option<std::time::SystemTime>,
    /// A rejected snapshot remains untouched until an explicit recovery/import replaces it.
    persistence_blocked: bool,
    pending_close: Option<u64>,
    /// A delayed public Locate cannot resurrect a task whose view the user already closed.
    closed_tasks: std::collections::BTreeSet<String>,
    /// One native find input follows the selected session without borrowing its terminal text.
    search_view: Option<Entity<editor_terminal::SearchBar>>,
    search_open: bool,
    search_focus_pending: bool,
    /// Last pointer candidate makes keyboard-accessible context-menu navigation possible.
    link: Option<engine::Link>,
    /// Fractional wheel motion is retained instead of turning every event into three lines.
    wheel_remainder: f32,
    /// Each leaf retains its own sub-line wheel remainder, even when scrolling without taking focus.
    wheel_remainders: BTreeMap<u64, f32>,
    cursor_blink: Instant,
    /// Native pointer modifiers supplement the deliberately smaller portable GridEvent.
    pointer_modifiers: Rc<Cell<gpui_kit::Modifiers>>,
    /// Terminal-only split layout; every leaf is an existing session with its own owned PTY.
    panes: Option<panes::PaneLayout>,
    /// Retained per-session input entities prevent cross-pane IME and selection ownership.
    grids: BTreeMap<u64, Entity<TextGrid>>,
    geometry: BTreeMap<u64, panes::Geometry>,
    /// Suppress repeated motion reports within one native screen cell, without suppressing clicks.
    last_mouse: Option<(u64, u8, usize, usize)>,
}

impl TerminalPanel {
    /// Preserve an interrupted import's destinations until startup can resume its durable write plan.
    pub(crate) fn upgrade_failed(&mut self, error: String) {
        self.report_error(FailureKind::Restore, error);
        self.persistence_blocked = true;
    }
    fn report_error(&mut self, kind: FailureKind, details: impl std::fmt::Display) {
        let key = match kind {
            FailureKind::Restore => "terminal.error.restore",
            FailureKind::Save => "terminal.error.save",
            FailureKind::Settings => "terminal.error.settings",
            FailureKind::Process => "terminal.error.process",
        };
        self.error = Some(t!(key, details = details.to_string()).to_string());
    }
    /// Prepare settings and logical history without launching anything in a restricted workspace.
    pub(crate) fn new(
        parent: WeakEntity<EditorApp>,
        io_host: Entity<extensions::ExtensionPanel>,
        workspace: PathBuf,
        profile: Option<&Path>,
        trusted: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let storage = profile
            .map(|profile| persistence::directory_in_profile(&workspace, profile))
            .unwrap_or_else(|| persistence::directory(&workspace));
        let focus = cx.focus_handle();
        // A dock can retain the pre-render handle; entering it must still land on the current input canvas.
        let focus_subscription = cx.on_focus(&focus, window, |panel, window, cx| {
            if let Some(canvas) = &panel.grid {
                canvas.read(cx).focus_handle().focus(window, cx);
            } else {
                panel.focus_pending = true;
                cx.notify();
            }
        });
        let mut panel = Self {
            parent,
            io_host,
            workspace,
            storage,
            settings: Settings::default(),
            sessions: vec![],
            active: None,
            next_id: 0,
            visible: false,
            trusted,
            focus,
            _focus_subscription: focus_subscription,
            group: None,
            supervisor: NativeProcessGroup::new(trusted),
            grid: None,
            inspections: Default::default(),
            sidebar: None,
            width: 640.,
            height: 240.,
            cell_width: 8.4,
            cell_height: 21.,
            tab_width: 180.,
            preview_width: Rc::new(Cell::new(None)),
            popup: None,
            requested_menu: None,
            error: None,
            dirty: false,
            last_save: Instant::now(),
            measure_pending: true,
            focus_pending: false,
            settings_stamp: None,
            persistence_blocked: false,
            pending_close: None,
            closed_tasks: Default::default(),
            search_view: None,
            search_open: false,
            search_focus_pending: false,
            link: None,
            wheel_remainder: 0.,
            wheel_remainders: Default::default(),
            cursor_blink: Instant::now(),
            pointer_modifiers: Default::default(),
            panes: None,
            grids: Default::default(),
            geometry: Default::default(),
            last_mouse: None,
        };
        if let Err(error) = panel.restore() {
            panel.report_error(FailureKind::Restore, format!("{error:#}"));
            panel.persistence_blocked = true;
        }
        panel.settings_stamp = std::fs::metadata(panel.storage.join("settings.json"))
            .and_then(|meta| meta.modified())
            .ok();
        // Poll regardless of visibility: hiding the dock must not block output or process cleanup.
        cx.spawn(async move |panel, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(30))
                    .await;
                if panel.update(cx, |panel, cx| panel.poll(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        panel
    }

    pub(crate) fn visible(&self) -> bool {
        self.visible
    }
    /// Hidden terminal tabs still own programs and participate in the project close decision.
    pub(crate) fn has_running_programs(&self) -> bool {
        self.sessions
            .iter()
            .any(|session| session.launched && !session.exited)
    }
    /// Startup restores panel layout without creating or restarting a shell.
    pub(crate) fn restore_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        self.visible = visible;
        self.focus_pending = false;
        cx.notify();
    }
    /// Commit logical history before joining native cleanup at the editor's close gate.
    pub(crate) fn shutdown(&mut self) -> anyhow::Result<std::sync::mpsc::Receiver<()>> {
        let saved = self.save();
        if let Err(error) = saved {
            self.report_error(FailureKind::Save, &error);
            anyhow::bail!("{error}");
        }
        self.supervisor.shutdown()
    }
    /// Showing an empty panel creates a shell; hiding it preserves all native sessions.
    pub(crate) fn set_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        self.visible = visible;
        self.focus_pending = visible;
        if visible && self.sessions.is_empty() {
            self.new_shell(self.settings.default_profile, cx);
        }
        if let Some(group) = self.group.as_ref().and_then(WeakEntity::upgrade) {
            group.update(cx, |_, cx| cx.notify());
        }
        cx.emit(PanelEvent::LayoutChanged);
        cx.notify();
    }

    /// Trust revocation reaches the supervisor before any delayed launch can execute.
    pub(crate) fn set_trusted(&mut self, trusted: bool, cx: &mut Context<Self>) {
        if self.trusted == trusted {
            return;
        }
        self.trusted = trusted;
        if let Err(error) = self.supervisor.set_trusted(trusted) {
            self.report_error(FailureKind::Process, error);
        }
        if trusted && self.visible {
            self.launch_active(false);
        }
        cx.notify();
    }

    fn active_index(&self) -> Option<usize> {
        self.sessions
            .iter()
            .position(|session| Some(session.id) == self.active)
    }
    fn grid_size(&self) -> GridSize {
        GridSize {
            columns: ((self.width - 16.) / self.cell_width)
                .floor()
                .clamp(2., 1000.) as usize,
            rows: ((self.height - 16.) / self.cell_height)
                .floor()
                .clamp(1., 500.) as usize,
        }
    }

    /// New tabs use the tool name, without an automatically increasing suffix.
    fn new_shell(&mut self, profile: usize, cx: &mut Context<Self>) {
        if self.sessions.len() >= 32 {
            self.error = Some(t!("terminal.session_limit").to_string());
            cx.notify();
            return;
        }
        let Some(profile) = self.settings.profiles.get(profile).cloned() else {
            return;
        };
        let Some(next_id) = self.next_id.checked_add(1) else {
            self.error = Some(t!("terminal.session_limit").to_string());
            cx.notify();
            return;
        };
        self.next_id = next_id;
        let name = Path::new(&profile.program)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let size = self.grid_size();
        let cwd = if self.settings.inherit_working_directory {
            self.active_index()
                .map(|index| self.sessions[index].cwd.clone())
        } else {
            None
        }
        .unwrap_or_else(|| self.workspace.to_string_lossy().into_owned());
        self.sessions.push(Session {
            id: self.next_id,
            name,
            profile,
            cwd,
            engine: Engine::new(size, self.settings.history),
            launched: false,
            exited: false,
            restored: false,
            task: None,
        });
        self.active = Some(self.next_id);
        self.dirty = true;
        self.measure_pending = true;
        self.focus_pending = true;
        // Canvas measurement starts this shell, avoiding an initial guessed PTY size and redraw.
        cx.notify();
    }

    /// Measurements may start newly requested shells, but restored transcripts require user input.
    fn launch_active(&mut self, from_user_input: bool) {
        let Some(index) = self.active_index() else {
            return;
        };
        if !self.trusted
            || self.sessions[index].task.is_some()
            || self.sessions[index].launched
            || self.sessions[index].exited
            || (self.sessions[index].restored && !from_user_input)
        {
            return;
        }
        let size = self.grid_size();
        let session = &mut self.sessions[index];
        let prompt = shell::default_prompt(&session.profile, &session.cwd);
        session
            .engine
            .begin_process(cfg!(windows), prompt.as_deref());
        let launch = NativeLaunch {
            program: session.profile.program.clone(),
            args: shell::arguments(&session.profile),
            cwd: session.cwd.clone(),
            env: self.settings.env.clone(),
            transport: Transport::Pty {
                columns: size.columns as u16,
                rows: size.rows as u16,
                inherit_cursor: cfg!(windows) && session.restored,
            },
        };
        match self.supervisor.launch(session.id, launch) {
            Ok(()) => {
                session.launched = true;
            }
            Err(error) => {
                self.report_error(FailureKind::Process, error);
            }
        }
    }

    fn write(&mut self, bytes: Vec<u8>, cx: &mut App) {
        if bytes.is_empty() {
            return;
        }
        // Sending text or a terminal key is an explicit request to use this retained Shell.
        // Restoring layout, selecting history, focus and resize alone never start a program.
        self.launch_active(true);
        self.cursor_blink = Instant::now();
        if let Some(index) = self.active_index() {
            let session = &mut self.sessions[index];
            session.engine.cursor_visible = true;
            session.engine.bell = false;
            if session.exited || !session.launched {
                return;
            }
            session.engine.set_offset(0);
            let id = session.id;
            self.send_input(id, bytes, cx);
        }
    }

    fn remove_session(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(index) = self.sessions.iter().position(|session| session.id == id) else {
            return;
        };
        if self.sessions[index].task.is_none()
            && let Err(error) = self.supervisor.close(id)
        {
            self.report_error(FailureKind::Process, error);
            return;
        }
        if let Some(task) = &self.sessions[index].task {
            self.closed_tasks.insert(task.key.clone());
            self.inspections.remove(&task.key);
        }
        self.sessions.remove(index);
        self.grids.remove(&id);
        self.geometry.remove(&id);
        self.wheel_remainders.remove(&id);
        if let Some(layout) = &mut self.panes {
            layout.sessions.retain(|session| *session != id);
            if layout.sessions.len() < 2 {
                self.panes = None;
            }
        }
        if self.pending_close == Some(id) {
            self.pending_close = None;
        }
        if self.active == Some(id) {
            self.active = self
                .sessions
                .get(index.min(self.sessions.len().saturating_sub(1)))
                .map(|session| session.id);
            // Automatic fallback is an identity switch too, even when canvas geometry is unchanged.
            self.measure_pending = true;
            self.focus_pending = true;
        }
        self.dirty = true;
        if self.sessions.is_empty() {
            self.set_visible(false, cx);
        }
        cx.notify();
    }

    fn tabs(&self) -> SideTabs {
        SideTabs {
            id: "native-terminal-sessions".into(),
            position: match self.settings.tab_position {
                editor_terminal::TabPosition::Left => protocol::ui::SideTabsPosition::Left,
                editor_terminal::TabPosition::Right => protocol::ui::SideTabsPosition::Right,
            },
            width: self.tab_width,
            min_width: 112.,
            max_width: 480.,
            items: self
                .sessions
                .iter()
                .map(|session| SideTab {
                    id: session.id.to_string(),
                    label: session.name.clone(),
                    status: if session.exited {
                        Some(t!("terminal.exited").to_string())
                    } else if self.settings.visual_bell && session.engine.bell {
                        Some(t!("terminal.bell").to_string())
                    } else if self.settings.show_titles {
                        session.engine.title.clone()
                    } else {
                        None
                    },
                    disabled: false,
                    closable: true,
                })
                .collect(),
            selected: self.active.map(|id| id.to_string()),
            rename: None,
        }
    }

    fn tab_action(&mut self, action: Action, cx: &mut Context<Self>) {
        match action {
            Action::Select(id) => {
                self.link = None;
                self.wheel_remainder = 0.;
                self.active = id
                    .parse()
                    .ok()
                    .filter(|id| self.sessions.iter().any(|session| session.id == *id));
                self.dirty = true;
                self.measure_pending = true;
            }
            Action::Close(id) => {
                if let Ok(id) = id.parse() {
                    self.close(id, cx);
                }
            }
            Action::Rename { id, value } => {
                if let Some(session) = self
                    .sessions
                    .iter_mut()
                    .find(|session| session.id.to_string() == id)
                    && !value.trim().is_empty()
                {
                    session.name = value.trim().chars().take(128).collect();
                    self.dirty = true;
                }
            }
            Action::Move { from, to } => {
                if let (Some(from), Some(to)) = (
                    self.sessions
                        .iter()
                        .position(|tab| tab.id.to_string() == from),
                    self.sessions
                        .iter()
                        .position(|tab| tab.id.to_string() == to),
                ) {
                    let item = self.sessions.remove(from);
                    self.sessions.insert(to, item);
                    self.dirty = true;
                }
            }
            Action::Resize(width) => {
                self.tab_width = width.clamp(112., 480.);
                self.dirty = true;
            }
            _ => {}
        }
        cx.notify();
    }
}

impl Drop for TerminalPanel {
    fn drop(&mut self) {
        let _ = self.save();
    }
}
impl EventEmitter<PanelEvent> for TerminalPanel {}
impl Focusable for TerminalPanel {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.grid
            .as_ref()
            .map(|canvas| canvas.read(cx).focus_handle())
            .unwrap_or_else(|| self.focus.clone())
    }
}
impl dock::BasePanel for TerminalPanel {
    fn panel_name(&self) -> &'static str {
        "NativeTerminal"
    }
    fn closable(&self, _: &App) -> bool {
        false
    }
    fn zoomable(&self, _: &App) -> bool {
        false
    }
    fn visible(&self, _: &App) -> bool {
        self.visible
    }
    fn on_added_to(
        &mut self,
        group: WeakEntity<gpui_base::dock::TabGroup>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        self.group = Some(group);
    }
    fn on_removed(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.group = None;
    }
}

/// One native icon retains the user's terminal artwork through light and dark theme projection.
pub(crate) fn icon(cx: &App) -> Icon {
    let bytes: &'static [u8] = if cx.theme().is_dark() {
        include_bytes!("icon_dark.svg")
    } else {
        include_bytes!("icon_light.svg")
    };
    Icon::default().data(bytes).small()
}
