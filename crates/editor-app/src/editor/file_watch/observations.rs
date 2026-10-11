//! Convert the shared native watcher into bounded portable observations, including unopened files.
use super::*;
use plugin_runtime::plugin_protocol::{
    api::ResourceHandle,
    workspace::{FileEventKind, FileTarget},
};

/// The existing paired-rename tracker supplies identity; external paths never enter workspace events.
pub(super) fn record_file_observation(
    result: &notify::Result<Event>,
    root: &Path,
    sources: &HashMap<usize, PathBuf>,
    pending: &mut Vec<FileEventKind>,
) {
    let event = match result {
        Ok(event) if !event.need_rescan() && !event.paths.is_empty() => event,
        _ => {
            rescan(pending);
            return;
        }
    };
    let relative = |path: &Path| -> Option<String> {
        let path = path.strip_prefix(root).ok()?.to_str()?.replace('\\', "/");
        if path.is_empty()
            || path
                .split('/')
                .any(|part| part == ".git" || part == "target")
        {
            return None;
        }
        let target = FileTarget {
            root: ResourceHandle {
                instance: String::new(),
                scope: String::new(),
                resource: 0,
            },
            path,
        };
        target.validate().ok()?;
        Some(target.path)
    };
    match event.kind {
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
            if let [source, target] = event.paths.as_slice() {
                rename(relative(source), relative(target), pending);
            } else {
                rescan(pending);
            }
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
            if let [target] = event.paths.as_slice() {
                let source = event
                    .attrs
                    .tracker()
                    .and_then(|tracker| sources.get(&tracker))
                    .and_then(|source| relative(source));
                if let Some(source) = &source {
                    if let Some(index) = pending.iter().rposition(
                        |event| matches!(event, FileEventKind::Deleted { path } if path == source),
                    ) {
                        pending.remove(index);
                    }
                }
                rename(source, relative(target), pending);
            }
        }
        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(_) => {
            for path in &event.paths {
                let Some(path) = relative(path) else {
                    continue;
                };
                if path.ends_with(".gitignore") || path.ends_with(".ignore") {
                    rescan(pending);
                    return;
                }
                let kind = match event.kind {
                    EventKind::Create(_) => FileEventKind::Created { path },
                    EventKind::Remove(_)
                    | EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
                        FileEventKind::Deleted { path }
                    }
                    _ => FileEventKind::Modified { path },
                };
                push(pending, kind);
            }
        }
        EventKind::Access(_) => {}
        _ => rescan(pending),
    }
}

fn rename(source: Option<String>, target: Option<String>, pending: &mut Vec<FileEventKind>) {
    match (source, target) {
        (Some(source), Some(target)) => push(pending, FileEventKind::Renamed { source, target }),
        (Some(path), None) => push(pending, FileEventKind::Deleted { path }),
        (None, Some(path)) => push(pending, FileEventKind::Created { path }),
        _ => {}
    }
}

fn push(pending: &mut Vec<FileEventKind>, event: FileEventKind) {
    if pending.contains(&FileEventKind::RescanRequired) {
        return;
    }
    if let FileEventKind::Modified { path } = &event {
        if let Some(index) = pending
            .iter()
            .position(|old| matches!(old, FileEventKind::Modified { path: old } if old == path))
        {
            pending.remove(index);
        }
    }
    if pending.len() >= 128 {
        rescan(pending);
    } else {
        pending.push(event);
    }
}

fn rescan(pending: &mut Vec<FileEventKind>) {
    pending.clear();
    pending.push(FileEventKind::RescanRequired);
}

#[cfg(test)]
mod tests;
