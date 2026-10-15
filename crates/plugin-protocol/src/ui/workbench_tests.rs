//! Workbench nodes keep source authority and action quotas independent from supplied readonly text.
use super::*;
use crate::api::{DocumentVersion, TextRange};

/// Alignment is an explicit additive opt-in; an older pane does not acquire new scroll semantics.
#[test]
fn code_alignment_requires_its_own_capability_minor() {
    let ordinary = Document::new(Node::new(
        "snapshot",
        Kind::ReadonlyCode {
            text: "source\n".into(),
            changes: vec![],
            sync_group: Some("comparison".into()),
        },
    ));
    assert_eq!(
        ordinary.required_capabilities()["ui.code_editor"],
        semver::Version::new(1, 0, 0)
    );
    let mut wire = serde_json::to_value(&ordinary).unwrap();
    wire["code_alignment"] = true.into();
    let aligned: Document = serde_json::from_value(wire).unwrap();
    aligned.validate().unwrap();
    assert_eq!(
        aligned.required_capabilities()["ui.code_editor"],
        semver::Version::new(1, 1, 0)
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
        Kind::EditorWithActions {
            document: source(),
            left: vec![action()],
            right: vec![],
            sync_group: None,
        },
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
        Kind::ReadonlyCode {
            text: "中😀\n".into(),
            changes: vec![TextRange { start: 0, end: 3 }],
            sync_group: None,
        },
    ));
    document.validate().unwrap();
    assert!(
        document
            .required_capabilities()
            .contains_key("ui.code_editor")
    );
    assert_eq!(document.active_native_editor(), None);
    if let Kind::ReadonlyCode { changes, .. } = &mut document.root.kind {
        changes[0].end = 4;
    }
    assert!(document.validate().is_err());
    document.root = Node::new(
        "large",
        Kind::ReadonlyCode {
            text: "x".repeat(256 * 1024),
            changes: vec![],
            sync_group: None,
        },
    );
    document.validate().unwrap();
    if let Kind::ReadonlyCode { text, .. } = &mut document.root.kind {
        text.push('x');
    }
    assert!(document.validate().is_err());
}
#[test]
fn edge_controls_cannot_mount_another_native_editor() {
    let mut document = Document::new(Node::new(
        "source",
        Kind::EditorWithActions {
            document: source(),
            left: vec![EditorAction {
                offset: 0,
                content: Box::new(Node::new(
                    "second",
                    Kind::NativeEditor { document: source() },
                )),
            }],
            right: vec![],
            sync_group: None,
        },
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
            Kind::ReadonlyCode {
                text: String::new(),
                changes: Vec::new(),
                sync_group: group.map(str::to_owned),
            },
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

/// Older literals remain independent without declaring or serializing the optional group field.
#[test]
fn code_sync_group_omission_keeps_the_old_wire_shape() {
    let kind: Kind =
        serde_json::from_value(serde_json::json!({"type":"readonly_code","text":"old"})).unwrap();
    assert!(matches!(
        &kind,
        Kind::ReadonlyCode {
            sync_group: None,
            ..
        }
    ));
    assert!(
        serde_json::to_value(kind)
            .unwrap()
            .get("sync_group")
            .is_none()
    );
}
