//! Behavior checks cross the same history interface used by the native application.

use editor_history::{
    Capture, CaptureOutcome, Change, ClearScope, History, Policy, Query, Reason, SkipReason,
};

/// Structural observations retain unsupported files without inventing an empty text snapshot.
#[test]
fn structural_only_files_keep_identity_and_have_no_recoverable_content() {
    let directory = tempfile::tempdir().unwrap();
    let history = History::open(directory.path().join("history"), Policy::default()).unwrap();
    let old = directory.path().join("image.png");
    let new = directory.path().join("nested/image.png");
    let file = history.track(&old, Some(directory.path()), 999).unwrap();
    history
        .change(Change {
            file,
            path: new.clone(),
            workspace: Some(directory.path().into()),
            reason: Reason::Moved,
            present: true,
            at_ms: 1_000,
        })
        .unwrap();
    let bindings = history.bindings_under(directory.path()).unwrap();
    assert_eq!(bindings, vec![(file, new)]);
    assert_eq!(history.list(&Query::default()).unwrap()[0].size, None);
    let row = &history.list(&Query::default()).unwrap()[0];
    assert_eq!(row.previous_path.as_deref(), Some(old.as_path()));
    history
        .change(Change {
            file,
            path: directory.path().join("nested/image.png"),
            workspace: Some(directory.path().into()),
            reason: Reason::Deleted,
            present: false,
            at_ms: 1_001,
        })
        .unwrap();
    assert_eq!(
        history
            .list(&Query {
                deleted_only: true,
                ..Default::default()
            })
            .unwrap()
            .len(),
        3
    );
    assert!(!history.location(file).unwrap().present);
    history
        .set_policy(Policy {
            enabled: false,
            ..Policy::default()
        })
        .unwrap();
    assert!(history.track(&old, None, 1_002).is_err());
}

/// Unrelated captures cannot silently discard a structural file's current identity.
#[test]
fn structural_binding_survives_other_file_capture() {
    let directory = tempfile::tempdir().unwrap();
    let history = History::open(directory.path().join("history"), Policy::default()).unwrap();
    let old = directory.path().join("image.png");
    let file = history.track(&old, None, 1_000).unwrap();
    history
        .capture(Capture {
            file: None,
            path: directory.path().join("note.txt"),
            workspace: None,
            contents: b"unrelated".to_vec(),
            reason: Reason::Saved,
            at_ms: 1_001,
        })
        .unwrap();
    history
        .change(Change {
            file,
            path: directory.path().join("renamed.png"),
            workspace: None,
            reason: Reason::Renamed,
            present: true,
            at_ms: 1_002,
        })
        .unwrap();
    assert_eq!(
        history.location(file).unwrap().path,
        directory.path().join("renamed.png")
    );
}

/// A successful capture remains readable after all connections have been closed.
#[test]
fn complete_version_survives_reopening() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("project").join("note.txt");
    let history = History::open(directory.path().join("history"), Policy::default()).unwrap();
    let version = history
        .capture(Capture {
            file: None,
            path,
            workspace: None,
            contents: "第一版\r\n".as_bytes().to_vec(),
            reason: Reason::Saved,
            at_ms: 1_000,
        })
        .unwrap()
        .version()
        .unwrap();
    drop(history);
    let history = History::open(directory.path().join("history"), Policy::default()).unwrap();
    assert_eq!(history.read(version).unwrap(), "第一版\r\n".as_bytes());
}

/// Duplicate saves reuse a point; revisiting old content keeps its new time but shares storage.
#[test]
fn duplicate_content_does_not_erase_time_points() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("note.txt");
    let history = History::open(directory.path().join("history"), Policy::default()).unwrap();
    let capture = |value: &str, at_ms| {
        history
            .capture(Capture {
                file: None,
                path: path.clone(),
                workspace: None,
                contents: value.as_bytes().to_vec(),
                reason: Reason::Saved,
                at_ms,
            })
            .unwrap()
    };
    let first = capture("A", 1_000).version().unwrap();
    assert_eq!(capture("A", 1_001), CaptureOutcome::Unchanged(first));
    capture("B", 1_002);
    capture("A", 1_003);
    let versions = history.list(&Query::default()).unwrap();
    let values: Vec<_> = versions
        .iter()
        .map(|version| history.read(version.id).unwrap())
        .collect();
    assert_eq!(
        (values, history.stats().unwrap().objects),
        (vec![b"A".to_vec(), b"B".to_vec(), b"A".to_vec()], 2)
    );
}

/// Limits use original bytes; excluded, binary and disabled captures never silently enter history.
#[test]
fn user_policy_controls_eligible_content() {
    let directory = tempfile::tempdir().unwrap();
    let history = History::open(
        directory.path().join("history"),
        Policy {
            max_file_bytes: 4,
            ..Policy::default()
        },
    )
    .unwrap();
    let capture = |name: &str, contents: &[u8]| {
        history
            .capture(Capture {
                file: None,
                path: directory.path().join(name),
                workspace: None,
                contents: contents.to_vec(),
                reason: Reason::Saved,
                at_ms: 1_000,
            })
            .unwrap()
    };
    assert!(capture("note.txt", b"1234").version().is_some());
    assert_eq!(
        capture("note.txt", b"12345"),
        CaptureOutcome::Skipped(SkipReason::TooLarge)
    );
    assert_eq!(
        capture(".git/config", b"abc"),
        CaptureOutcome::Skipped(SkipReason::Excluded)
    );
    assert_eq!(
        capture("binary.txt", b"\0ab"),
        CaptureOutcome::Skipped(SkipReason::Unsupported)
    );
    assert_eq!(
        capture("other.txt", &[0xff]),
        CaptureOutcome::Skipped(SkipReason::Unsupported)
    );
    history
        .set_policy(Policy {
            enabled: false,
            ..Policy::default()
        })
        .unwrap();
    assert_eq!(
        capture("note.txt", b"abc"),
        CaptureOutcome::Skipped(SkipReason::Disabled)
    );
}

/// Renaming preserves identity while an independent creation at a deleted path starts a new one.
#[test]
fn deleted_path_reuse_does_not_inherit_the_old_file() {
    let directory = tempfile::tempdir().unwrap();
    let history = History::open(directory.path().join("history"), Policy::default()).unwrap();
    history
        .capture(Capture {
            file: None,
            path: directory.path().join("a.txt"),
            workspace: None,
            contents: b"old".to_vec(),
            reason: Reason::Saved,
            at_ms: 1_000,
        })
        .unwrap();
    let file = history.list(&Query::default()).unwrap()[0].file;
    let path = directory.path().join("renamed.txt");
    history
        .change(Change {
            file,
            path: path.clone(),
            workspace: None,
            reason: Reason::Renamed,
            present: true,
            at_ms: 2_000,
        })
        .unwrap();
    let deleted = history
        .change(Change {
            file,
            path: path.clone(),
            workspace: None,
            reason: Reason::Deleted,
            present: false,
            at_ms: 3_000,
        })
        .unwrap();
    history
        .capture(Capture {
            file: None,
            path,
            workspace: None,
            contents: b"new".to_vec(),
            reason: Reason::Saved,
            at_ms: 4_000,
        })
        .unwrap();
    let all = history.list(&Query::default()).unwrap();
    assert_ne!(all[0].file, file);
    let old = history
        .list(&Query {
            file: Some(file),
            ..Query::default()
        })
        .unwrap();
    assert_eq!(
        (old.len(), old[0].present, history.read(deleted).unwrap()),
        (3, false, b"old".to_vec())
    );
}

/// A restore protection is durable and survives cleanup until the pending native operation ends.
#[test]
fn restore_protection_is_independent_and_pinned_until_release() {
    let directory = tempfile::tempdir().unwrap();
    let history = History::open(directory.path().join("history"), Policy::default()).unwrap();
    let capture = |value: &str, at_ms| Capture {
        file: None,
        path: directory.path().join("note.txt"),
        workspace: None,
        contents: value.as_bytes().to_vec(),
        reason: Reason::Saved,
        at_ms,
    };
    let saved = history
        .capture(capture("disk", 1_000))
        .unwrap()
        .version()
        .unwrap();
    let protected = history.protect(capture("unsaved edit", 2_000)).unwrap();
    assert_eq!(
        history.capture(capture("disk", 3_000)).unwrap(),
        CaptureOutcome::Unchanged(saved)
    );
    history.clear(&ClearScope::All).unwrap();
    assert_eq!(history.read(protected.version()).unwrap(), b"unsaved edit");
    drop(protected);
    history.clear(&ClearScope::All).unwrap();
    assert_eq!(
        (
            history.stats().unwrap().versions,
            history.stats().unwrap().objects
        ),
        (0, 0)
    );
}

/// Calendar retention keeps the exact boundary and removes only observations older than it.
#[test]
fn calendar_retention_reclaims_unreferenced_content() {
    let directory = tempfile::tempdir().unwrap();
    let history = History::open(
        directory.path().join("history"),
        Policy {
            retention_days: 1,
            ..Policy::default()
        },
    )
    .unwrap();
    for (value, at_ms) in [("old", 1_000), ("new", 86_401_000)] {
        history
            .capture(Capture {
                file: None,
                path: directory.path().join("note.txt"),
                workspace: None,
                contents: value.as_bytes().to_vec(),
                reason: Reason::Saved,
                at_ms,
            })
            .unwrap();
    }
    history.maintain(86_401_000).unwrap();
    assert_eq!(history.stats().unwrap().versions, 2);
    history.maintain(86_401_001).unwrap();
    let retained = history.list(&Query::default()).unwrap();
    assert_eq!(
        (
            retained.len(),
            history.read(retained[0].id).unwrap(),
            history.stats().unwrap().objects
        ),
        (1, b"new".to_vec(), 1)
    );
}

/// A shared byte budget evicts the oldest versions and actually shrinks persistent database space.
#[test]
fn capacity_eviction_reclaims_real_space_and_keeps_the_latest_version() {
    let directory = tempfile::tempdir().unwrap();
    let history = History::open(
        directory.path().join("history"),
        Policy {
            max_total_bytes: 128 * 1024,
            ..Policy::default()
        },
    )
    .unwrap();
    let mut state = 123_u32;
    let mut last = Vec::new();
    for index in 0..20 {
        let contents: Vec<u8> = (0..16 * 1024)
            .map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                32 + ((state >> 16) % 95) as u8
            })
            .collect();
        history
            .capture(Capture {
                file: None,
                path: directory.path().join("note.txt"),
                workspace: None,
                contents: contents.clone(),
                reason: Reason::Saved,
                at_ms: 1_000 + index,
            })
            .unwrap();
        history.maintain(1_000 + index).unwrap();
        last = contents;
    }
    let usage = history.stats().unwrap();
    assert!(usage.disk_bytes <= 128 * 1024);
    assert!(usage.versions < 20);
    let newest = history.list(&Query::default()).unwrap()[0].id;
    assert_eq!(history.read(newest).unwrap(), last);
}

/// An unretainable new object fails atomically rather than erasing old history to discover it cannot fit.
#[test]
fn impossible_capture_rolls_back_without_losing_prior_versions() {
    let directory = tempfile::tempdir().unwrap();
    let history = History::open(
        directory.path().join("history"),
        Policy {
            max_total_bytes: 128 * 1024,
            ..Policy::default()
        },
    )
    .unwrap();
    let path = directory.path().join("note.txt");
    let old = history
        .capture(Capture {
            file: None,
            path: path.clone(),
            workspace: None,
            contents: b"old".to_vec(),
            reason: Reason::Saved,
            at_ms: 1_000,
        })
        .unwrap()
        .version()
        .unwrap();
    let mut state = 987_u32;
    let contents = (0..256 * 1024)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            32 + ((state >> 16) % 95) as u8
        })
        .collect();
    let result = history.capture(Capture {
        file: None,
        path,
        workspace: None,
        contents,
        reason: Reason::Saved,
        at_ms: 2_000,
    });
    assert!(matches!(result, Err(editor_history::Error::Quota)));
    assert_eq!(
        (
            history.stats().unwrap().versions,
            history.read(old).unwrap()
        ),
        (1, b"old".to_vec())
    );
}

/// Independent project owners using the same profile must see the same in-flight protection roots.
#[test]
fn simultaneous_profile_owners_share_restore_protection() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("history");
    let first = History::open(&root, Policy::default()).unwrap();
    let second = History::open(&root, Policy::default()).unwrap();
    let protected = first
        .protect(Capture {
            file: None,
            path: directory.path().join("note.txt"),
            workspace: None,
            contents: b"unsaved".to_vec(),
            reason: Reason::BeforeRestore,
            at_ms: 1_000,
        })
        .unwrap();
    second.clear(&ClearScope::All).unwrap();
    assert_eq!(first.read(protected.version()).unwrap(), b"unsaved");
}

/// A destructive confirmation describes the previewed set, not records created while it was open.
#[test]
fn stale_cleanup_confirmation_cannot_delete_newer_history() {
    let directory = tempfile::tempdir().unwrap();
    let history = History::open(directory.path().join("history"), Policy::default()).unwrap();
    let capture = |value: &str, at_ms| Capture {
        file: None,
        path: directory.path().join("note.txt"),
        workspace: None,
        contents: value.as_bytes().to_vec(),
        reason: Reason::Saved,
        at_ms,
    };
    history.capture(capture("A", 1_000)).unwrap();
    let preview = history.preview_clear(&ClearScope::All).unwrap();
    history.capture(capture("B", 2_000)).unwrap();
    assert!(matches!(
        history.clear_confirmed(&ClearScope::All, preview),
        Err(editor_history::Error::Stale)
    ));
    assert_eq!(history.stats().unwrap().versions, 2);
}
