//! Navigation snapshots come from activated window ownership and the shared recent-project authority.

use super::*;

/// History membership is independent of the opened/recent grouping and never borrows a live editor.
pub(in crate::app::projects) fn recent_paths(source: AnyWindowHandle, cx: &App) -> Vec<PathBuf> {
    let Some(state) = cx.try_global::<Windows>() else {
        return Vec::new();
    };
    let Some(source) = state.entries.get(&source.window_id()) else {
        return Vec::new();
    };
    state
        .recent
        .get(&source.profile)
        .map(|recent| {
            recent
                .borrow()
                .entries
                .iter()
                .map(|entry| entry.path.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// One canonical directory belongs to exactly one displayed group, independently of its name.
#[derive(Clone, PartialEq, Eq)]
pub(in crate::app::projects) struct ProjectChoice {
    pub(in crate::app::projects) path: PathBuf,
    pub(in crate::app::projects) opened: bool,
}

/// Record a successful activation, never a trust reservation or a simple window-focus change.
/// The canonical root is display metadata; the existing directory lease still owns deduplication.
pub(in crate::app::projects) fn mark_opened(handle: AnyWindowHandle, path: PathBuf, cx: &mut App) {
    let state = cx.global_mut::<Windows>();
    state.opened_order = state.opened_order.saturating_add(1);
    if let Some(entry) = state.entries.get_mut(&handle.window_id()) {
        entry.project = Some(path);
        entry.opened_order = state.opened_order;
    }
    cx.refresh_windows();
}

/// Read this source window's profile without borrowing any editor or creating a second registry.
/// Live projects precede unopened recent records; each group retains successful-open order.
pub(in crate::app::projects) fn project_choices(
    source: AnyWindowHandle,
    cx: &App,
) -> Vec<ProjectChoice> {
    let Some(state) = cx.try_global::<Windows>() else {
        return Vec::new();
    };
    let Some(source) = state.entries.get(&source.window_id()) else {
        return Vec::new();
    };
    let mut opened: Vec<_> = state
        .entries
        .values()
        .filter(|entry| {
            entry.profile == source.profile
                && entry.project.is_some()
                && entry.host.upgrade().is_some()
        })
        .collect();
    opened.sort_by_key(|entry| std::cmp::Reverse(entry.opened_order));
    let mut choices: Vec<_> = opened
        .iter()
        .filter_map(|entry| {
            entry
                .project
                .clone()
                .map(|path| ProjectChoice { path, opened: true })
        })
        .collect();
    if let Some(recent) = state.recent.get(&source.profile) {
        for entry in &recent.borrow().entries {
            if opened
                .iter()
                .any(|open| open.project.as_ref() == Some(&entry.path))
            {
                continue;
            }
            // Successful opens already persist canonical roots. Keep paints free of filesystem
            // probes, especially for offline network records; the open route rechecks identity.
            choices.push(ProjectChoice {
                path: entry.path.clone(),
                opened: false,
            });
        }
    }
    choices
}

/// Activate a still-open project directly, including an unavailable network directory.
/// Returns false when it has closed, so the caller can enter the ordinary validated open flow.
pub(in crate::app::projects) fn focus_project(
    source: &mut Window,
    path: &Path,
    cx: &mut App,
) -> bool {
    let entry = cx.try_global::<Windows>().and_then(|state| {
        let profile = &state
            .entries
            .get(&source.window_handle().window_id())?
            .profile;
        state
            .entries
            .values()
            .find(|entry| {
                &entry.profile == profile
                    && entry.project.as_deref() == Some(path)
                    && entry.host.upgrade().is_some()
            })
            .cloned()
    });
    if let Some(entry) = entry {
        if entry.handle.window_id() == source.window_handle().window_id() {
            // Updating this same leased Window by handle would fail; its caller already owns it.
            source.activate_window();
        } else {
            activate(entry, None, cx);
        }
        true
    } else {
        false
    }
}
