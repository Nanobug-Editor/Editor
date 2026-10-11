//! Source-boundary cases verify paired/unmatched renames, modification coalescing and explicit loss.
use super::*;

/// Existing rename tracking and an ordered observation batch share the same native event identity.
#[test]
fn workspace_transaction_observations_coalesce_and_preserve_paired_and_unmatched_renames() {
    let root = Path::new("/workspace");
    let mut pending = Vec::new();
    let mut sources = HashMap::new();
    for _ in 0..2 {
        record_file_observation(
            &Ok(Event::new(EventKind::Modify(ModifyKind::Data(
                notify::event::DataChange::Any,
            )))
            .add_path(root.join("data.txt"))),
            root,
            &sources,
            &mut pending,
        );
    }
    assert_eq!(
        pending,
        [FileEventKind::Modified {
            path: "data.txt".into()
        }]
    );
    let from = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::From)))
        .add_path(root.join("before.txt"))
        .set_tracker(17);
    record_file_observation(&Ok(from), root, &sources, &mut pending);
    sources.insert(17, root.join("before.txt"));
    let to = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::To)))
        .add_path(root.join("after.txt"))
        .set_tracker(17);
    record_file_observation(&Ok(to), root, &sources, &mut pending);
    assert_eq!(
        pending[1],
        FileEventKind::Renamed {
            source: "before.txt".into(),
            target: "after.txt".into()
        }
    );
    let lone = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::To)))
        .add_path(root.join("unpaired.txt"));
    record_file_observation(&Ok(lone), root, &sources, &mut pending);
    assert_eq!(
        pending[2],
        FileEventKind::Created {
            path: "unpaired.txt".into()
        }
    );
    for path in [
        root.join(".git/HEAD"),
        root.join("target/cache"),
        PathBuf::from("/outside/secret.txt"),
    ] {
        record_file_observation(
            &Ok(Event::new(EventKind::Create(notify::event::CreateKind::File)).add_path(path)),
            root,
            &sources,
            &mut pending,
        );
    }
    assert_eq!(
        pending.len(),
        3,
        "non-workspace or internal paths never enter the public stream"
    );
}

/// Overflow and native loss discard incomplete transition history and require explicit rediscovery.
#[test]
fn workspace_transaction_observation_overflow_and_native_loss_require_rescan() {
    let root = Path::new("/workspace");
    let mut pending = Vec::new();
    let sources = HashMap::new();
    for index in 0..129 {
        record_file_observation(
            &Ok(
                Event::new(EventKind::Create(notify::event::CreateKind::File))
                    .add_path(root.join(format!("file-{index}.txt"))),
            ),
            root,
            &sources,
            &mut pending,
        );
    }
    assert_eq!(pending, [FileEventKind::RescanRequired]);
    record_file_observation(
        &Ok(
            Event::new(EventKind::Remove(notify::event::RemoveKind::File))
                .add_path(root.join("after-loss.txt")),
        ),
        root,
        &sources,
        &mut pending,
    );
    assert_eq!(pending, [FileEventKind::RescanRequired]);
    pending.clear();
    record_file_observation(
        &Err(notify::Error::generic("native source lost")),
        root,
        &sources,
        &mut pending,
    );
    assert_eq!(pending, [FileEventKind::RescanRequired]);
}
