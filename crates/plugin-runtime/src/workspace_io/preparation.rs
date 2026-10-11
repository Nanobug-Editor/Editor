//! Ordered preparation validates expected versions and projects immutable proposed disk states.
use super::*;

impl FilePlan {
    /// Validate an ordered file subset off the UI thread. Opened document writes are removed by the host.
    /// Expected FromPlan stamps must refer backwards to the object produced at that path.
    pub fn prepare(
        changes: &[(u16, Change)],
        authority: &WorkspaceRequest,
        buffers: &BTreeSet<u16>,
        stop: &impl Fn() -> bool,
    ) -> Result<Self, Failure> {
        let mut files = BTreeMap::new();
        let mut mutated = BTreeSet::new();
        for (index, change) in changes {
            for (target, write) in targets(change) {
                let file = authority.file(target, write)?.clone();
                let _guard = file.guard()?;
                if write && !buffers.contains(index) {
                    mutated.insert(file.path().to_owned());
                }
                if write || !files.contains_key(file.path()) {
                    files.insert(file.path().to_owned(), file);
                }
            }
        }
        let mut budget = Budget::default();
        let mut original = BTreeMap::new();
        for path in files.keys() {
            original.insert(path.clone(), Snapshot::capture(path, &mut budget, 0, stop)?);
        }
        let original_bytes = snapshot::projection_bytes(original.values(), stop)?;
        let mut projected_peak = original_bytes;
        let mut planned = original.clone();
        let mut producers = BTreeMap::<PathBuf, u16>::new();
        let mut actions = BTreeMap::new();
        for (index, change) in changes {
            ensure_running(stop)?;
            let get = |target: &FileTarget, write| authority.file(target, write).cloned();
            let action = match change {
                Change::CreateDirectory { target } => {
                    let target = get(target, true)?;
                    expect_missing(&planned[target.path()])?;
                    check_parent(&target, &planned)?;
                    project(
                        &mut planned,
                        target.path(),
                        Snapshot::directory(target.path())?,
                    );
                    producers.insert(target.path().to_owned(), *index);
                    Action::Create(target)
                }
                Change::Write {
                    target,
                    expected,
                    bytes,
                } => {
                    let target = get(target, true)?;
                    if let Some(expected) = expected {
                        check_expected(
                            expected,
                            &planned[target.path()],
                            producers.get(target.path()),
                            *index,
                        )?;
                    } else if !target.is_save_intent() {
                        return Err(conflict("Expected version is required outside Save intent"));
                    }
                    if matches!(planned[target.path()].node, Node::Directory { .. })
                        || planned[target.path()].readonly()
                    {
                        return Err(conflict("Write target is a directory or readonly"));
                    }
                    check_parent(&target, &planned)?;
                    if buffers.contains(index) {
                        actions.insert(*index, Action::Buffer(target));
                        continue;
                    }
                    let mut next = Snapshot::file(bytes.clone());
                    if let Node::File { permissions, .. } = &planned[target.path()].node {
                        if let Node::File {
                            permissions: next_permissions,
                            ..
                        } = &mut next.node
                        {
                            *next_permissions = permissions.clone();
                        }
                    }
                    project(&mut planned, target.path(), next.clone());
                    producers.insert(target.path().to_owned(), *index);
                    Action::Write(target, next)
                }
                Change::Copy {
                    source,
                    target,
                    expected_source,
                }
                | Change::Rename {
                    source,
                    target,
                    expected_source,
                } => {
                    let rename = matches!(change, Change::Rename { .. });
                    let source = get(source, rename)?;
                    let target = get(target, true)?;
                    check_expected(
                        expected_source,
                        &planned[source.path()],
                        producers.get(source.path()),
                        *index,
                    )?;
                    if matches!(planned[source.path()].node, Node::Missing) {
                        return Err(conflict("Source does not exist in planned state"));
                    }
                    if source.path() == target.path()
                        || target.path().starts_with(source.path())
                        || source.path().starts_with(target.path())
                    {
                        return Err(conflict("Source and destination overlap"));
                    }
                    if rename && planned[source.path()].readonly() {
                        return Err(conflict("Rename source is readonly"));
                    }
                    expect_missing(&planned[target.path()])?;
                    check_parent(&target, &planned)?;
                    let next = planned[source.path()].clone();
                    project(&mut planned, target.path(), next.clone());
                    producers.insert(target.path().to_owned(), *index);
                    if rename {
                        project(&mut planned, source.path(), Snapshot::missing());
                        producers.insert(source.path().to_owned(), *index);
                        Action::Rename(source, target)
                    } else {
                        Action::Copy(source, target, next)
                    }
                }
                Change::Delete { target, expected } => {
                    let target = get(target, true)?;
                    check_expected(
                        expected,
                        &planned[target.path()],
                        producers.get(target.path()),
                        *index,
                    )?;
                    if matches!(planned[target.path()].node, Node::Missing)
                        || planned[target.path()].readonly()
                    {
                        return Err(conflict("Delete target is missing or readonly"));
                    }
                    project(&mut planned, target.path(), Snapshot::missing());
                    producers.insert(target.path().to_owned(), *index);
                    Action::Delete(target)
                }
                Change::EditDocument { .. } => {
                    return Err(Failure::new(
                        ErrorCode::InvalidRequest,
                        "Native text is not a disk operation",
                    ));
                }
            };
            // Every intermediate state must fit actual post-state capture, including copy/rename tree growth.
            projected_peak =
                projected_peak.max(snapshot::projection_bytes(planned.values(), stop)?);
            actions.insert(*index, action);
        }
        Ok(Self {
            files,
            current: original.clone(),
            original,
            actions,
            mutated,
            observed: true,
            reserved_bytes: original_bytes + projected_peak,
        })
    }
}

fn targets(change: &Change) -> Vec<(&FileTarget, bool)> {
    match change {
        Change::CreateDirectory { target }
        | Change::Write { target, .. }
        | Change::Delete { target, .. } => vec![(target, true)],
        Change::Copy { source, target, .. } => vec![(source, false), (target, true)],
        Change::Rename { source, target, .. } => vec![(source, true), (target, true)],
        Change::EditDocument { .. } => Vec::new(),
    }
}

fn check_expected(
    expected: &FileStamp,
    current: &Snapshot,
    producer: Option<&u16>,
    index: u16,
) -> Result<(), Failure> {
    let valid = match expected {
        FileStamp::FromPlan { change } => *change < index && producer == Some(change),
        _ => expected == &current.stamp,
    };
    if valid {
        Ok(())
    } else {
        Err(conflict("Expected disk/planned version does not match"))
    }
}
fn expect_missing(snapshot: &Snapshot) -> Result<(), Failure> {
    if matches!(snapshot.node, Node::Missing) {
        Ok(())
    } else {
        Err(conflict("Destination already exists"))
    }
}
fn check_parent(
    file: &AuthorizedFile,
    planned: &BTreeMap<PathBuf, Snapshot>,
) -> Result<(), Failure> {
    let parent = file
        .path()
        .parent()
        .ok_or_else(|| conflict("Missing parent"))?;
    if let Some(parent) = planned.get(parent) {
        if !matches!(parent.node, Node::Directory { .. }) {
            return Err(conflict("Planned parent is not a directory"));
        }
    } else if !parent.is_dir() {
        return Err(conflict(
            "Parent directory must exist or be created earlier in the plan",
        ));
    }
    Ok(())
}

/// Updating the simulated tree handles ancestor overlap and ordered rename chains without any IO.
fn project(states: &mut BTreeMap<PathBuf, Snapshot>, path: &Path, snapshot: Snapshot) {
    for (other, state) in states.iter_mut() {
        if other == path {
            *state = snapshot.clone();
        } else if let Ok(relative) = other.strip_prefix(path) {
            *state = child(&snapshot, relative).unwrap_or_else(Snapshot::missing);
        } else if let Ok(relative) = path.strip_prefix(&*other) {
            set_child(state, relative, snapshot.clone());
        }
    }
}
fn child(snapshot: &Snapshot, relative: &Path) -> Option<Snapshot> {
    let mut current = snapshot;
    for name in relative.iter() {
        let Node::Directory { entries, .. } = &current.node else {
            return None;
        };
        current = entries.get(name.to_str()?)?;
    }
    Some(current.clone())
}
fn set_child(snapshot: &mut Snapshot, relative: &Path, replacement: Snapshot) {
    let Some(name) = relative.iter().next().and_then(|name| name.to_str()) else {
        return;
    };
    let Node::Directory { entries, .. } = &mut snapshot.node else {
        return;
    };
    if relative.components().count() == 1 {
        if matches!(replacement.node, Node::Missing) {
            entries.remove(name);
        } else {
            entries.insert(name.into(), replacement);
        }
    } else if let Some(child) = entries.get_mut(name) {
        set_child(
            child,
            &relative.iter().skip(1).collect::<PathBuf>(),
            replacement,
        );
    }
}
