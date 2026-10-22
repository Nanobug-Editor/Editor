//! Successful window closure and application exit choose the next primary launch's project.

use super::*;

/// Snapshot the most recently active real project in each profile before exit consent begins.
pub(super) fn exit_projects(cx: &App) -> Vec<(PathBuf, PathBuf)> {
    let mut entries = cx.global::<Windows>().entries.values().collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.activated_order);
    entries
        .into_iter()
        .filter_map(|entry| Some((entry.profile.clone(), entry.project.clone()?)))
        .collect::<BTreeMap<_, _>>()
        .into_iter()
        .collect()
}

/// Record this `handle`'s real project after its local cleanup succeeds, before retiring its owner.
pub(super) fn remember_closed(handle: AnyWindowHandle, cx: &App) {
    if let Some(entry) = cx.global::<Windows>().entries.get(&handle.window_id())
        && let Some(project) = &entry.project
    {
        remember(&entry.profile, project);
    }
}

/// Commit the consent round's snapshot only after the whole application has shut down successfully.
pub(super) fn remember_exit(cx: &App) {
    if let Some(round) = &cx.global::<Windows>().quit {
        for (profile, project) in &round.projects_to_restore {
            remember(profile, project);
        }
    }
}

/// Exit metadata is best effort; a broken preference file is retained for diagnosis.
fn remember(profile: &Path, project: &Path) {
    if let Err(error) = app::startup::preferences::remember_project(profile, project) {
        eprintln!("Could not remember the last project: {error}");
    }
}
