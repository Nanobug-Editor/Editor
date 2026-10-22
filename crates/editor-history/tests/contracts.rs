//! Public storage contracts cover byte fidelity, indexed navigation and shared-object ownership.

use editor_history::{Capture, Change, ClearScope, History, Policy, Query, Reason, Scope};

/// Original encodings and line endings are content, not normalization opportunities.
#[test]
fn original_utf8_bytes_round_trip_without_normalization() {
    let directory = tempfile::tempdir().unwrap();
    let history = History::open(directory.path().join("history"), Policy::default()).unwrap();
    for (index, bytes) in [
        "",
        "中文🙂",
        "\u{feff}BOM\r\n",
        "a\nb\n",
        "a\r\nb\r\n",
        "a\nb\r\nc",
        "no final newline",
    ]
    .into_iter()
    .enumerate()
    {
        let version = history
            .capture(Capture {
                file: None,
                path: directory.path().join("note.txt"),
                workspace: None,
                contents: bytes.as_bytes().to_vec(),
                reason: Reason::Saved,
                at_ms: index as i64,
            })
            .unwrap()
            .version()
            .unwrap();
        assert_eq!(history.read(version).unwrap(), bytes.as_bytes());
    }
    assert_eq!(history.stats().unwrap().objects, 7);
}

/// Content sharing never merges file identity or widens a workspace clear to other memberships.
#[test]
fn clearing_a_workspace_keeps_other_files_shared_content_readable() {
    let directory = tempfile::tempdir().unwrap();
    let history = History::open(directory.path().join("history"), Policy::default()).unwrap();
    let mut versions = Vec::new();
    for project in ["a", "b"] {
        let workspace = directory.path().join(project);
        versions.push(
            history
                .capture(Capture {
                    file: None,
                    path: workspace.join("note.txt"),
                    workspace: Some(workspace),
                    contents: b"shared".to_vec(),
                    reason: Reason::Saved,
                    at_ms: 1_000,
                })
                .unwrap()
                .version()
                .unwrap(),
        );
    }
    assert_eq!(history.stats().unwrap().objects, 1);
    let rows = history.list(&Query::default()).unwrap();
    assert_ne!(rows[0].file, rows[1].file);
    let scope = ClearScope::Workspace(directory.path().join("a"));
    let preview = history.preview_clear(&scope).unwrap();
    assert_eq!((preview.versions, preview.reclaimable_bytes), (1, 0));
    history.clear_confirmed(&scope, preview).unwrap();
    assert_eq!(history.read(versions[1]).unwrap(), b"shared");
    history.clear(&ClearScope::All).unwrap();
    assert_eq!(history.stats().unwrap().objects, 0);
}

/// Stable cursors disambiguate equal timestamps and literal path filters do not act as SQL wildcards.
#[test]
fn indexed_pages_and_scopes_have_no_duplicates_or_missing_rows() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("history");
    let history = History::open(&root, Policy::default()).unwrap();
    let workspace = directory.path().join("project");
    for index in 0..113 {
        history
            .capture(Capture {
                file: None,
                path: workspace.join("100%_中文.txt"),
                workspace: Some(workspace.clone()),
                contents: index.to_string().into_bytes(),
                reason: Reason::Saved,
                at_ms: 1_000,
            })
            .unwrap();
    }
    let file = history
        .file_at(&workspace.join("100%_中文.txt"))
        .unwrap()
        .unwrap();
    history
        .change(Change {
            file,
            path: workspace.join("renamed.txt"),
            workspace: Some(workspace.clone()),
            reason: Reason::Renamed,
            present: true,
            at_ms: 1_000,
        })
        .unwrap();
    let mut query = Query {
        scope: Scope::Workspace(workspace),
        path: "100%_中文".into(),
        limit: 50,
        ..Default::default()
    };
    let mut ids = Vec::new();
    loop {
        let page = history.list(&query).unwrap();
        if page.is_empty() {
            break;
        }
        query.before = page.last().map(|row| row.cursor());
        ids.extend(page.into_iter().map(|row| row.id.0));
    }
    assert_eq!(ids.len(), 114);
    assert!(ids.windows(2).all(|pair| pair[0] > pair[1]));
    assert!(
        history
            .list(&Query {
                scope: Scope::Standalone,
                ..Default::default()
            })
            .unwrap()
            .is_empty()
    );
    // Payload corruption is intentionally external: listing must continue using metadata only.
    let external = rusqlite::Connection::open(root.join("history.sqlite3")).unwrap();
    external
        .execute("UPDATE objects SET payload=x'00'", [])
        .unwrap();
    assert_eq!(history.list(&Query::default()).unwrap().len(), 50);
    assert!(history.read(editor_history::VersionId(ids[0])).is_err());
}

/// Size limits apply to original bytes, at the exact boundary, before compression savings.
#[test]
fn default_and_maximum_original_file_boundaries_are_enforced() {
    let directory = tempfile::tempdir().unwrap();
    let history = History::open(directory.path().join("history"), Policy::default()).unwrap();
    for maximum in [8 * 1024 * 1024, 32 * 1024 * 1024] {
        history
            .set_policy(Policy {
                max_file_bytes: maximum,
                ..Default::default()
            })
            .unwrap();
        for size in [maximum - 1, maximum, maximum + 1] {
            let result = history
                .capture(Capture {
                    file: None,
                    path: directory.path().join("large.txt"),
                    workspace: None,
                    contents: vec![b'x'; size as usize],
                    reason: Reason::Saved,
                    at_ms: size as i64,
                })
                .unwrap();
            assert_eq!(result.version().is_some(), size <= maximum);
        }
    }
}

/// Unsupported current bytes cannot borrow a previously stored text object's recovery badge.
#[test]
fn structural_only_observation_revokes_the_old_content_hint() {
    let directory = tempfile::tempdir().unwrap();
    let history = History::open(directory.path().join("history"), Policy::default()).unwrap();
    let path = directory.path().join("note.txt");
    let old = history
        .capture(Capture {
            file: None,
            path: path.clone(),
            workspace: None,
            contents: b"old text".to_vec(),
            reason: Reason::Saved,
            at_ms: 1_000,
        })
        .unwrap()
        .version()
        .unwrap();
    let file = history.file_at(&path).unwrap().unwrap();
    let unsupported = history
        .change_without_content(Change {
            file,
            path: path.clone(),
            workspace: None,
            reason: Reason::BeforeDelete,
            present: true,
            at_ms: 1_001,
        })
        .unwrap();
    let deleted = history
        .change(Change {
            file,
            path,
            workspace: None,
            reason: Reason::Deleted,
            present: false,
            at_ms: 1_002,
        })
        .unwrap();
    assert!(history.read(unsupported).is_err());
    assert!(history.read(deleted).is_err());
    assert_eq!(history.read(old).unwrap(), b"old text");
}

/// Saving a recovered original must revive its binding, even when the bytes equal its pre-delete head.
#[test]
fn saving_equal_content_after_deletion_records_a_live_recovery_point() {
    let directory = tempfile::tempdir().unwrap();
    let history = History::open(directory.path().join("history"), Policy::default()).unwrap();
    let path = directory.path().join("note.txt");
    let first = Capture {
        file: None,
        path: path.clone(),
        workspace: None,
        contents: b"same".to_vec(),
        reason: Reason::Saved,
        at_ms: 1_000,
    };
    history.capture(first).unwrap();
    let file = history.file_at(&path).unwrap().unwrap();
    history
        .change(Change {
            file,
            path: path.clone(),
            workspace: None,
            reason: Reason::Deleted,
            present: false,
            at_ms: 1_001,
        })
        .unwrap();
    let recovered = history
        .capture(Capture {
            file: Some(file),
            path: path.clone(),
            workspace: None,
            contents: b"same".to_vec(),
            reason: Reason::Saved,
            at_ms: 1_002,
        })
        .unwrap();
    assert!(matches!(
        recovered,
        editor_history::CaptureOutcome::Recorded(_)
    ));
    assert_eq!(history.file_at(&path).unwrap(), Some(file));
    assert!(history.location(file).unwrap().present);
}
