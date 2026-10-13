//! The application owns native window routing; project views retain their own documents and resources.
use super::*;
use gpui_kit::{AnyWindowHandle, Global, WindowId};
use platform_windows::directory_identity::DirectoryIdentity;
use std::cell::RefCell;
use std::collections::BTreeMap;

/// A pending trust prompt reserves the same directory identity as an already active project.
#[derive(Clone)]
pub(super) struct Entry {
    pub(super) host: WeakEntity<ProjectWindow>,
    pub(super) handle: AnyWindowHandle,
    pub(super) directory: Option<DirectoryIdentity>,
}
#[derive(Default)]
struct Windows {
    entries: BTreeMap<WindowId, Entry>,
    recent: BTreeMap<PathBuf, Rc<RefCell<RecentProjects>>>,
    quit: Option<QuitRound>,
}
impl Global for Windows {}

/// Consent retains immutable document identity, not another text buffer or an undo history.
#[derive(Clone)]
pub(super) struct Consent {
    pub(super) documents: DocumentVersions,
    pub(super) running: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum QuitPhase {
    Collecting,
    Cleaning,
    Shared,
    Failed,
}
struct QuitRound {
    members: BTreeMap<WindowId, (Option<Consent>, bool)>,
    waiting: Option<WindowId>,
    phase: QuitPhase,
    failed: bool,
}

pub(super) fn is_quitting(cx: &App) -> bool {
    cx.try_global::<Windows>()
        .is_some_and(|state| state.quit.is_some())
}

/// Explicit application quit begins one consent round and seals new project opens until it resolves.
pub(super) fn quit(cx: &mut App) {
    ensure(cx);
    if cx
        .global::<Windows>()
        .quit
        .as_ref()
        .is_some_and(|round| round.phase != QuitPhase::Failed)
    {
        return;
    }
    let members = cx
        .global::<Windows>()
        .entries
        .keys()
        .map(|id| (*id, (None, false)))
        .collect();
    cx.global_mut::<Windows>().quit = Some(QuitRound {
        members,
        waiting: None,
        phase: QuitPhase::Collecting,
        failed: false,
    });
    cx.defer(advance_quit);
}

/// Cancelling any prompt abandons all earlier consent without destroying documents or stopping work.
pub(super) fn cancel_quit(cx: &mut App) {
    if !is_quitting(cx) {
        return;
    }
    if cx
        .global::<Windows>()
        .quit
        .as_ref()
        .is_some_and(|round| round.phase != QuitPhase::Collecting)
    {
        return;
    }
    cx.global_mut::<Windows>().quit = None;
    let entries = cx
        .global::<Windows>()
        .entries
        .values()
        .cloned()
        .collect::<Vec<_>>();
    // Defer updates because cancellation can originate while one ProjectWindow is already borrowed.
    cx.defer(move |cx| {
        for entry in entries {
            let _ = entry.handle.update(cx, |_, window, cx| {
                let _ = entry.host.update(cx, |host, cx| {
                    if matches!(
                        host.decision,
                        Some(Decision::Leave {
                            intent: CloseIntent::Application,
                            ..
                        })
                    ) {
                        host.decision = None;
                    }
                    if let Some(editor) = &host.editor {
                        editor.read(cx).editor.focus_handle(cx).focus(window, cx);
                    }
                    cx.notify();
                });
            });
        }
    });
}

/// Accept only identity captured after an explicit save or discard decision; cleanup remains deferred.
pub(super) fn approve(handle: AnyWindowHandle, consent: Consent, cx: &mut App) {
    if let Some(round) = cx
        .global_mut::<Windows>()
        .quit
        .as_mut()
        .filter(|round| round.phase == QuitPhase::Collecting)
    {
        if let Some(member) = round.members.get_mut(&handle.window_id()) {
            member.0 = Some(consent);
        }
        round.waiting = None;
        cx.defer(advance_quit);
    }
}

/// A local cleanup authorized before Quit can finish or fail while this round waits; inspect its resulting view afresh.
pub(super) fn local_cleanup_settled(handle: AnyWindowHandle, cx: &mut App) {
    let Some(round) = cx.global_mut::<Windows>().quit.as_mut() else {
        return;
    };
    if round.phase == QuitPhase::Collecting && round.waiting == Some(handle.window_id()) {
        round.waiting = None;
        // No old document consent is inferred from local cleanup: a home, editable failure or remaining window is reviewed again.
        cx.defer(advance_quit);
    }
}

/// One prompt at a time; a final pass detects new native input before committing any retirement.
fn advance_quit(cx: &mut App) {
    let Some(round) = cx.global::<Windows>().quit.as_ref() else {
        return;
    };
    if round.phase != QuitPhase::Collecting || round.waiting.is_some() {
        return;
    }
    let pending = round
        .members
        .iter()
        .find_map(|(id, (consent, _))| consent.is_none().then_some(*id));
    if let Some(id) = pending {
        let entry = cx.global::<Windows>().entries.get(&id).cloned();
        if let Some(entry) = entry {
            cx.global_mut::<Windows>().quit.as_mut().unwrap().waiting = Some(id);
            let _ = entry.handle.update(cx, |_, window, cx| {
                window.activate_window();
                let _ = entry.host.update(cx, |host, cx| {
                    host.request_close(CloseIntent::Application, window, cx);
                });
            });
        } else {
            cancel_quit(cx);
        }
        return;
    }
    let proofs = round
        .members
        .iter()
        .map(|(id, (consent, _))| (*id, consent.clone().unwrap()))
        .collect::<Vec<_>>();
    for (id, proof) in proofs {
        let current = cx.global::<Windows>().entries.get(&id).cloned();
        let valid = current.is_some_and(|entry| {
            entry
                .handle
                .update(cx, |_, window, cx| {
                    entry
                        .host
                        .update(cx, |host, cx| host.consent_is_current(&proof, window, cx))
                        .unwrap_or(false)
                })
                .unwrap_or(false)
        });
        if !valid {
            if let Some(member) = cx
                .global_mut::<Windows>()
                .quit
                .as_mut()
                .unwrap()
                .members
                .get_mut(&id)
            {
                member.0 = None;
            }
            cx.defer(advance_quit);
            return;
        }
    }
    cx.global_mut::<Windows>().quit.as_mut().unwrap().phase = QuitPhase::Cleaning;
    let entries = cx
        .global::<Windows>()
        .entries
        .values()
        .cloned()
        .collect::<Vec<_>>();
    for entry in entries {
        let _ = entry.handle.update(cx, |_, window, cx| {
            let _ = entry.host.update(cx, |host, cx| {
                host.start_application_cleanup(window, cx);
            });
        });
    }
}

/// Successful project cleanup keeps its view mounted until every other project and shared owner succeeds.
pub(super) fn cleaned(handle: AnyWindowHandle, result: Result<(), String>, cx: &mut App) {
    let Some(round) = cx.global_mut::<Windows>().quit.as_mut() else {
        return;
    };
    round.failed |= result.is_err();
    if let Some(member) = round.members.get_mut(&handle.window_id()) {
        member.1 = true;
    }
    if round.phase != QuitPhase::Cleaning || !round.members.values().all(|(_, cleaned)| *cleaned) {
        return;
    }
    if round.failed {
        round.phase = QuitPhase::Failed;
        let entries = cx
            .global::<Windows>()
            .entries
            .values()
            .cloned()
            .collect::<Vec<_>>();
        cx.defer(move |cx| {
            for entry in entries {
                let _ = entry.host.update(cx, |host, cx| {
                    host.close_state = CloseState::Failed;
                    cx.notify();
                });
            }
        });
        return;
    }
    round.phase = QuitPhase::Shared;
    let pool = extensions::runtime_owners::for_application(cx);
    cx.spawn(async move |cx| {
        let result = cx
            .background_executor()
            .spawn(async move { pool.shutdown() })
            .await;
        cx.update(move |cx| match result {
            Ok(()) => {
                let entries = cx
                    .global::<Windows>()
                    .entries
                    .values()
                    .cloned()
                    .collect::<Vec<_>>();
                cx.global_mut::<Windows>().quit = None;
                for entry in entries {
                    let _ = entry
                        .handle
                        .update(cx, |_, window, _| window.remove_window());
                }
                cx.quit();
            }
            Err(error) => {
                if let Some(round) = cx.global_mut::<Windows>().quit.as_mut() {
                    round.phase = QuitPhase::Failed;
                }
                let entries = cx
                    .global::<Windows>()
                    .entries
                    .values()
                    .cloned()
                    .collect::<Vec<_>>();
                for entry in entries {
                    let _ = entry.host.update(cx, |host, cx| {
                        host.close_state = CloseState::Failed;
                        host.error = Some(error.clone());
                        cx.notify();
                    });
                }
            }
        });
    })
    .detach();
}

fn ensure(cx: &mut App) {
    if !cx.has_global::<Windows>() {
        cx.set_global(Windows::default());
    }
}

/// Register a native root once; auxiliary settings/plugin windows never count as project windows.
pub(super) fn register(host: WeakEntity<ProjectWindow>, handle: AnyWindowHandle, cx: &mut App) {
    ensure(cx);
    cx.global_mut::<Windows>().entries.insert(
        handle.window_id(),
        Entry {
            host,
            handle,
            directory: None,
        },
    );
}

/// Records use one application authority per profile; stale home snapshots cannot replace recent opens.
pub(super) fn recent(profile: &Path, cx: &mut App) -> Rc<RefCell<RecentProjects>> {
    ensure(cx);
    let key = profile.canonicalize().unwrap_or_else(|_| profile.into());
    cx.global_mut::<Windows>()
        .recent
        .entry(key)
        .or_insert_with(|| Rc::new(RefCell::new(RecentProjects::load(profile))))
        .clone()
}

pub(super) fn reserve(handle: AnyWindowHandle, identity: Option<DirectoryIdentity>, cx: &mut App) {
    if let Some(entry) = cx
        .global_mut::<Windows>()
        .entries
        .get_mut(&handle.window_id())
    {
        entry.directory = identity;
    }
}

/// Consent rechecks the leased identity, including a directory replaced at the same pathname.
pub(super) fn reserved(handle: AnyWindowHandle, cx: &App) -> Option<DirectoryIdentity> {
    cx.global::<Windows>()
        .entries
        .get(&handle.window_id())
        .and_then(|entry| entry.directory.clone())
}

pub(super) fn owner(identity: &DirectoryIdentity, cx: &App) -> Option<Entry> {
    cx.try_global::<Windows>()?
        .entries
        .values()
        .find(|entry| entry.directory.as_ref() == Some(identity) && entry.host.upgrade().is_some())
        .cloned()
}

/// Closing a view releases its reservation, without quitting peers or retaining a dead editor.
pub(super) fn unregister(handle: AnyWindowHandle, cx: &mut App) {
    if cx.has_global::<Windows>() {
        let state = cx.global_mut::<Windows>();
        state.entries.remove(&handle.window_id());
        if let Some(round) = &mut state.quit
            && round.phase == QuitPhase::Collecting
        {
            // A previously authorized local window close has completed, so no consent can be requested from its retired view.
            round.members.remove(&handle.window_id());
            if round.waiting == Some(handle.window_id()) {
                round.waiting = None;
            }
            cx.defer(advance_quit);
        }
    }
}

/// Activate an existing real directory owner; optional files use that owner's ordinary document route.
pub(super) fn activate(entry: Entry, file: Option<PathBuf>, cx: &mut App) {
    let _ = entry.handle.update(cx, |_, window, cx| {
        window.activate_window();
        let _ = entry.host.update(cx, |host, cx| {
            if let (Some(editor), Some(file)) = (&host.editor, file) {
                editor.update(cx, |app, cx| app.open_file(file, window, cx));
            }
        });
    });
}

/// A project-origin open always obtains another native window; an existing directory is routed before this call.
pub(super) fn open(profile: PathBuf, path: Option<PathBuf>, cx: &mut App) -> anyhow::Result<()> {
    let mut options = TitleBar::window_options();
    options.window_background = theme::window_background(cx);
    options.window_bounds = Some(WindowBounds::Windowed(Bounds::centered(
        None,
        size(px(1280.), px(820.)),
        cx,
    )));
    options.inactive_frame_interval = None;
    gpui_kit::open_window(options, cx, move |window, cx| {
        window.set_window_title(app::APP_NAME);
        cx.new(|cx| {
            let mut host = ProjectWindow::new(profile, window, cx);
            if let Some(path) = path {
                host.open_path(path, window, cx);
            }
            host
        })
    })?;
    Ok(())
}

/// Home requests reuse an unopened home or create one while every project window stays mounted.
pub(super) fn home(profile: PathBuf, cx: &mut App) -> anyhow::Result<()> {
    ensure(cx);
    if let Some(entry) = cx
        .global::<Windows>()
        .entries
        .values()
        .find(|entry| entry.directory.is_none() && entry.host.upgrade().is_some())
        .cloned()
    {
        activate(entry, None, cx);
        Ok(())
    } else {
        open(profile, None, cx)
    }
}

/// Keep the last native root visible until the shared application runtime acknowledges retirement.
pub(super) fn close_window(handle: AnyWindowHandle, window: &mut Window, cx: &mut App) {
    let last = cx.global::<Windows>().entries.len() == 1;
    if !last {
        unregister(handle, cx);
        window.remove_window();
        return;
    }
    // Last-window exit shares the application's open seal, failure reporting and retry path.
    quit(cx);
}
