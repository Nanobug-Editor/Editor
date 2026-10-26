//! Workbench nodes keep source authority and action quotas independent from supplied readonly text.
use super::*;
use crate::api::{DocumentVersion, TextRange};

/// Empty configured rails remain enabled so a withdrawn proposal cannot reflow the native source.
#[test]
fn configurable_editor_empty_line_actions_preserve_feature_configuration() {
    let kind: Kind = serde_json::from_value(serde_json::json!({
        "type":"editor", "source":{"type":"snapshot","text":"source"}, "read_only":true,
        "line_actions":{"left":[],"right":[]}
    }))
    .expect("optional complete edge feature");
    let document = Document::new(Node::new("snapshot", kind));
    document.validate().unwrap();
    assert!(
        document
            .required_capabilities()
            .contains_key("editor.actions")
    );
    assert!(
        serde_json::to_value(document).unwrap()["root"]["kind"]
            .get("line_actions")
            .is_some()
    );
}

/// A literal Editor is a bounded readonly presentation and never borrows document authority.
#[test]
fn configurable_editor_snapshot_requires_readonly_and_no_document_authority() {
    let wire = serde_json::json!({
        "type": "editor",
        "source": {"type": "snapshot", "text": "中😀\n"},
        "read_only": true,
        "changes": [{"start": 0, "end": 3}]
    });
    let kind: Kind = serde_json::from_value(wire.clone()).expect("public configurable Editor");
    let document = Document::new(Node::new("snapshot", kind));
    document.validate().unwrap();
    assert_eq!(document.active_native_editor(), None);
    assert_eq!(
        document.required_capabilities()["ui.code_editor"],
        semver::Version::new(2, 0, 0)
    );
    let mut writable = wire;
    writable["read_only"] = false.into();
    let kind: Kind = serde_json::from_value(writable).unwrap();
    assert!(
        Document::new(Node::new("snapshot", kind))
            .validate()
            .is_err()
    );
}

/// Scroll synchronization is selectable without manufacturing edge controls or stronger capability grants.
#[test]
fn configurable_editor_document_sync_and_readonly_do_not_require_actions() {
    let mut editor = Editor::document(source());
    editor.read_only = true;
    editor.sync_group = Some("comparison".into());
    let mut document = Document::new(Node::new("source", Kind::Editor(editor)));
    document.source = Some(source());
    document.editor_layout = true;
    document.validate().unwrap();
    assert_eq!(document.active_native_editor(), Some(&source()));
    assert!(
        !document
            .required_capabilities()
            .contains_key("editor.actions")
    );
    assert_eq!(
        document.required_capabilities()["editor.layout"],
        semver::Version::new(2, 0, 0)
    );
}

/// A replaced literal cannot reuse an already approved edge action's scene revision.
#[test]
fn configurable_editor_snapshot_actions_require_fresh_bytes_and_scene() {
    let mut editor = Editor::snapshot("before");
    editor.line_actions = Some(EditorActions {
        left: vec![EditorAction {
            offset: 0,
            content: Box::new(Node::button("review", "Review")),
        }],
        right: Vec::new(),
    });
    let original = Document::new(Node::new("snapshot", Kind::Editor(editor))).revision(4);
    original.validate().unwrap();
    let mut replacement = original.clone();
    let Kind::Editor(editor) = &mut replacement.root.kind else {
        unreachable!()
    };
    editor.source = EditorSource::Snapshot {
        text: "after".into(),
    };
    assert!(replacement.validate_editor_replacement(&original).is_err());
    replacement.revision += 1;
    replacement.validate_editor_replacement(&original).unwrap();
    assert!(
        replacement
            .validate_event(&UiEvent {
                revision: 4,
                node: "review".into(),
                action: Action::Click
            })
            .is_err()
    );
}

/// Removed editor shapes fail explicitly; supported SDK consumers must use the versioned Editor contract.
#[test]
fn configurable_editor_rejects_retired_node_shapes() {
    for legacy in ["native_editor", "editor_with_actions", "readonly_code"] {
        assert!(serde_json::from_value::<Kind>(serde_json::json!({"type":legacy})).is_err());
    }
}

/// Snapshot rails cannot smuggle the document input into an edge subtree outside the root mount.
#[test]
fn configurable_editor_snapshot_edges_cannot_borrow_a_document_mount() {
    let mut editor = Editor::snapshot("literal");
    editor.line_actions = Some(EditorActions {
        left: vec![EditorAction {
            offset: 0,
            content: Box::new(Node::new(
                "nested-document",
                Kind::Editor(Editor::document(source())),
            )),
        }],
        right: Vec::new(),
    });
    let mut document = Document::new(Node::new("snapshot", Kind::Editor(editor)));
    document.editor_layout = true;
    document.source = Some(source());
    assert!(document.validate().is_err());
}

/// Alignment is an explicit additive opt-in; an older pane does not acquire new scroll semantics.
#[test]
fn code_alignment_requires_its_own_capability_minor() {
    let ordinary = Document::new(Node::new(
        "snapshot",
        Kind::Editor(Editor {
            changes: vec![],
            sync_group: Some("comparison".into()),
            ..Editor::snapshot("source\n")
        }),
    ));
    assert_eq!(
        ordinary.required_capabilities()["ui.code_editor"],
        semver::Version::new(2, 0, 0)
    );
    let mut wire = serde_json::to_value(&ordinary).unwrap();
    wire["code_alignment"] = true.into();
    let aligned: Document = serde_json::from_value(wire).unwrap();
    aligned.validate().unwrap();
    assert_eq!(
        aligned.required_capabilities()["ui.code_editor"],
        semver::Version::new(2, 0, 0)
    );
    assert_eq!(
        serde_json::to_value(aligned).unwrap()["code_alignment"],
        true
    );
}

fn source() -> DocumentVersion {
    DocumentVersion {
        id: "source".into(),
        path: "file.txt".into(),
        revision: 7,
    }
}
fn action() -> EditorAction {
    EditorAction {
        offset: 4,
        content: Box::new(Node::button("accept", "Accept block")),
    }
}
#[test]
fn source_edge_controls_share_exact_native_authority_and_scene_events() {
    let mut document = Document::new(Node::new(
        "source",
        Kind::Editor(Editor {
            line_actions: Some(EditorActions {
                left: vec![action()],
                right: vec![],
            }),
            sync_group: None,
            ..Editor::document(source())
        }),
    ));
    assert!(document.validate().is_err());
    document.editor_layout = true;
    document.source = Some(source());
    document.revision = 9;
    document.validate().unwrap();
    assert_eq!(document.active_native_editor(), Some(&source()));
    assert!(
        document
            .required_capabilities()
            .contains_key("editor.actions")
    );
    document
        .validate_event(&UiEvent {
            revision: 9,
            node: "accept".into(),
            action: Action::Click,
        })
        .unwrap();
    assert!(
        document
            .validate_event(&UiEvent {
                revision: 8,
                node: "accept".into(),
                action: Action::Click
            })
            .is_err()
    );
    document.source.as_mut().unwrap().revision += 1;
    assert!(document.validate().is_err());
}
#[test]
fn readonly_bytes_have_no_document_authority_and_use_utf8_bounded_marks() {
    let mut document = Document::new(Node::new(
        "snapshot",
        Kind::Editor(Editor {
            changes: vec![TextRange { start: 0, end: 3 }],
            sync_group: None,
            ..Editor::snapshot("中😀\n")
        }),
    ));
    document.validate().unwrap();
    assert!(
        document
            .required_capabilities()
            .contains_key("ui.code_editor")
    );
    assert_eq!(document.active_native_editor(), None);
    if let Kind::Editor(editor) = &mut document.root.kind {
        editor.changes[0].end = 4;
    }
    assert!(document.validate().is_err());
    document.root = Node::new(
        "large",
        Kind::Editor(Editor {
            changes: vec![],
            sync_group: None,
            ..Editor::snapshot("x".repeat(256 * 1024))
        }),
    );
    document.validate().unwrap();
    if let Kind::Editor(Editor {
        source: EditorSource::Snapshot { text },
        ..
    }) = &mut document.root.kind
    {
        text.push('x');
    }
    assert!(document.validate().is_err());
}
#[test]
fn edge_controls_cannot_mount_another_native_editor() {
    let mut document = Document::new(Node::new(
        "source",
        Kind::Editor(Editor {
            line_actions: Some(EditorActions {
                left: vec![EditorAction {
                    offset: 0,
                    content: Box::new(Node::new(
                        "second",
                        Kind::Editor(Editor::document(source())),
                    )),
                }],
                right: vec![],
            }),
            sync_group: None,
            ..Editor::document(source())
        }),
    ));
    document.editor_layout = true;
    document.source = Some(source());
    assert!(document.validate().is_err());
}

/// Even empty literal panes allocate native selection/layout; byte quotas alone cannot bound them.
#[test]
fn readonly_code_panes_and_sync_groups_have_independent_resource_quotas() {
    let pane = |index: usize, group: Option<&str>| {
        Node::new(
            format!("code-{index}"),
            Kind::Editor(Editor {
                changes: Vec::new(),
                sync_group: group.map(str::to_owned),
                ..Editor::snapshot(String::new())
            }),
        )
    };
    let document = |nodes| Document::new(Node::row("panes", nodes));
    document((0..8).map(|index| pane(index, None)).collect())
        .validate()
        .unwrap();
    assert!(
        document((0..9).map(|index| pane(index, None)).collect())
            .validate()
            .is_err()
    );
    document(
        (0..3)
            .map(|index| pane(index, Some("comparison")))
            .collect(),
    )
    .validate()
    .unwrap();
    assert!(
        document(
            (0..4)
                .map(|index| pane(index, Some("comparison")))
                .collect()
        )
        .validate()
        .is_err()
    );
    assert!(
        document(
            (0..5)
                .map(|index| pane(index, Some(&format!("group-{index}"))))
                .collect()
        )
        .validate()
        .is_err()
    );
    for invalid in ["", "with space", "with/slash", "中文"] {
        assert!(document(vec![pane(0, Some(invalid))]).validate().is_err());
    }
}

/// Omitted optional features leave a readonly Editor independent and grant no document authority.
#[test]
fn code_sync_group_omission_keeps_the_old_wire_shape() {
    let kind: Kind =
        serde_json::from_value(serde_json::json!({"type":"editor","source":{"type":"snapshot","text":"text"},"read_only":true})).unwrap();
    assert!(matches!(
        &kind,
        Kind::Editor(Editor {
            sync_group: None,
            ..
        })
    ));
    assert!(
        serde_json::to_value(kind)
            .unwrap()
            .get("sync_group")
            .is_none()
    );
}
