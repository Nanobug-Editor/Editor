//! Comparison admission checks added source semantics before native resources are allocated.
use super::*;

/// Filename hints are additive metadata, bounded before admission and never a filesystem lookup.
#[test]
fn virtual_source_language_paths_require_1_2_and_bounded_metadata() {
    let mut operation = EditorOperation::OpenVirtualSource {
        title: "snapshot".into(),
        language: None,
        language_path: None,
        text: "source".into(),
    };
    let mut negotiated = api::Negotiated {
        base: semver::Version::new(1, 0, 0),
        capabilities: [("editor.virtual".into(), semver::Version::new(1, 1, 0))].into(),
    };
    validate_virtual_source_request(&operation, &negotiated).unwrap();
    if let EditorOperation::OpenVirtualSource { language_path, .. } = &mut operation {
        *language_path = Some("deleted/path/source.novel".into());
    }
    assert_eq!(
        validate_virtual_source_request(&operation, &negotiated)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnavailable
    );
    negotiated
        .capabilities
        .insert("editor.virtual".into(), semver::Version::new(1, 2, 0));
    validate_virtual_source_request(&operation, &negotiated).unwrap();
    // Historic paths may contain tabs/newlines even when this platform cannot check them out.
    for valid in ["old\nsource.novel", "old\tsource.novel"] {
        if let EditorOperation::OpenVirtualSource { language_path, .. } = &mut operation {
            *language_path = Some(valid.into());
        }
        validate_virtual_source_request(&operation, &negotiated).unwrap();
    }
    for invalid in [String::new(), "a\0b".into(), "文".repeat(1366)] {
        if let EditorOperation::OpenVirtualSource { language_path, .. } = &mut operation {
            *language_path = Some(invalid);
        }
        assert_eq!(
            validate_virtual_source_request(&operation, &negotiated)
                .unwrap_err()
                .code,
            ErrorCode::InvalidRequest
        );
    }
    let prior: EditorOperation = serde_json::from_value(serde_json::json!({
        "kind": "open_virtual_source", "title": "snapshot", "text": "plain"
    }))
    .unwrap();
    assert!(matches!(
        prior,
        EditorOperation::OpenVirtualSource {
            language_path: None,
            ..
        }
    ));
}

/// Runtime-issued direct commands own private sources; nested, foreign and revoked calls do not.
#[test]
fn direct_editor_owner_keeps_typed_host_commands_inside_the_original_instance() {
    use crate::plugin_services::{Context, InvocationOrigin};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let alive = Arc::new(AtomicBool::new(true));
    let mut context = Context {
        lifetimes: vec![alive.clone()],
        native_waits: Vec::new(),
        menu: None,
        origin: InvocationOrigin::HostCommand {
            target: "original".into(),
        },
        caller: plugin_protocol::service::Caller {
            plugin: "provider".into(),
            instance: "original".into(),
            scope: "workspace".into(),
            permissions: Default::default(),
        },
        ancestry: vec!["original".into()],
        permissions: Default::default(),
    };
    assert!(direct_editor_owner(None, "original"));
    assert!(
        direct_editor_owner(Some(&context), "original"),
        "a native typed command must be able to open its own readonly comparison sources"
    );
    assert!(!direct_editor_owner(Some(&context), "replacement"));
    context.ancestry.push("nested".into());
    assert!(!direct_editor_owner(Some(&context), "original"));
    context.ancestry.pop();
    context.origin = InvocationOrigin::Delegated;
    assert!(!direct_editor_owner(Some(&context), "original"));
    context.origin = InvocationOrigin::HostCommand {
        target: "original".into(),
    };
    alive.store(false, Ordering::Release);
    assert!(!direct_editor_owner(Some(&context), "original"));
}

/// Keep local identities separate even when content and revisions happen to match.
fn version(id: &str) -> api::DocumentVersion {
    api::DocumentVersion {
        id: id.into(),
        path: format!("{id}.txt"),
        revision: 1,
    }
}

/// Build the public request shape rather than a separate test-only admission interface.
fn comparison(third: Option<api::DocumentVersion>) -> EditorOperation {
    EditorOperation::OpenDocumentComparison {
        key: "file-comparison".into(),
        title: "Comparison".into(),
        left: version("left"),
        right: version("result"),
        third,
        navigation: None,
        folding: None,
        preview: true,
    }
}

/// Additive third-source requests cannot silently execute against an older negotiated contract.
#[test]
fn third_source_requires_diff_1_2_while_two_sources_keep_1_1() {
    let mut negotiated = api::Negotiated {
        base: semver::Version::new(1, 0, 0),
        capabilities: [("editor.diff".into(), semver::Version::new(1, 1, 0))].into(),
    };
    validate_comparison_request(&comparison(None), &negotiated).unwrap();
    assert_eq!(
        validate_comparison_request(&comparison(Some(version("third"))), &negotiated)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnavailable
    );
    negotiated
        .capabilities
        .insert("editor.diff".into(), semver::Version::new(1, 2, 0));
    validate_comparison_request(&comparison(Some(version("third"))), &negotiated).unwrap();
}

/// Revisions do not make repeated document sessions independent comparison inputs.
#[test]
fn comparison_rejects_a_repeated_session_in_any_source_position() {
    let negotiated = api::Negotiated {
        base: semver::Version::new(1, 0, 0),
        capabilities: [("editor.diff".into(), semver::Version::new(1, 2, 0))].into(),
    };
    for duplicate in ["left", "result"] {
        let mut third = version(duplicate);
        third.revision = 2;
        assert_eq!(
            validate_comparison_request(&comparison(Some(third)), &negotiated)
                .unwrap_err()
                .code,
            ErrorCode::InvalidRequest
        );
    }
    let mut operation = comparison(None);
    if let EditorOperation::OpenDocumentComparison { left, right, .. } = &mut operation {
        *right = left.clone();
    }
    assert_eq!(
        validate_comparison_request(&operation, &negotiated)
            .unwrap_err()
            .code,
        ErrorCode::InvalidRequest
    );
}

/// Adjacent commands cannot be sent under a two/three-source-only negotiated contract.
#[test]
fn adjacent_actions_require_diff_1_3_without_changing_older_request_admission() {
    let mut operation = comparison(None);
    if let EditorOperation::OpenDocumentComparison { navigation, .. } = &mut operation {
        *navigation = Some(api::ComparisonNavigation::default());
    }
    for (minor, allowed) in [(1, false), (2, false), (3, true)] {
        let negotiated = api::Negotiated {
            base: semver::Version::new(1, 0, 0),
            capabilities: [("editor.diff".into(), semver::Version::new(1, minor, 0))].into(),
        };
        assert_eq!(
            validate_comparison_request(&operation, &negotiated).is_ok(),
            allowed
        );
        validate_comparison_request(&comparison(None), &negotiated).unwrap();
    }
}

/// Folding is explicitly negotiated and cannot smuggle host/foreign paths or an unbounded context.
#[test]
fn unchanged_folding_requires_diff_1_4_and_valid_owned_artwork_descriptors() {
    let mut operation = comparison(None);
    if let EditorOperation::OpenDocumentComparison { folding, .. } = &mut operation {
        *folding = Some(api::ComparisonFolding {
            collapse: ui::ToolIcon::new("icons/collapse.svg", "icons/collapse.svg"),
            expand: ui::ToolIcon::new("icons/expand.svg", "icons/expand.svg"),
            context_lines: 3,
        });
    }
    let mut negotiated = api::Negotiated {
        base: semver::Version::new(1, 0, 0),
        capabilities: [("editor.diff".into(), semver::Version::new(1, 3, 0))].into(),
    };
    assert_eq!(
        validate_comparison_request(&operation, &negotiated)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnavailable
    );
    negotiated
        .capabilities
        .insert("editor.diff".into(), semver::Version::new(1, 4, 0));
    validate_comparison_request(&operation, &negotiated).unwrap();
    if let EditorOperation::OpenDocumentComparison {
        folding: Some(folding),
        ..
    } = &mut operation
    {
        folding.context_lines = 0;
    }
    assert_eq!(
        validate_comparison_request(&operation, &negotiated)
            .unwrap_err()
            .code,
        ErrorCode::InvalidRequest
    );
    if let EditorOperation::OpenDocumentComparison {
        folding: Some(folding),
        ..
    } = &mut operation
    {
        folding.context_lines = 3;
        folding.expand.light = "../foreign.svg".into();
    }
    assert_eq!(
        validate_comparison_request(&operation, &negotiated)
            .unwrap_err()
            .code,
        ErrorCode::InvalidPath
    );
}

/// UI publication is not a way around typed-command declaration, capability, schema or size checks.
#[test]
fn adjacent_actions_reject_undeclared_commands_untyped_arguments_and_oversized_context() {
    let mut operation = comparison(None);
    if let EditorOperation::OpenDocumentComparison { navigation, .. } = &mut operation {
        *navigation = Some(api::ComparisonNavigation {
            next: Some(api::ComparisonCommand {
                command: "neighbor".into(),
                arguments: serde_json::json!({"path":"next.txt"}),
            }),
            ..Default::default()
        });
    }
    let mut negotiated = api::Negotiated {
        base: semver::Version::new(1, 0, 0),
        capabilities: Default::default(),
    };
    let mut commands = BTreeMap::new();
    assert_eq!(
        validate_comparison_navigation(&operation, &negotiated, &commands)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnavailable
    );
    negotiated
        .capabilities
        .insert("plugin.commands".into(), semver::Version::new(1, 0, 0));
    assert_eq!(
        validate_comparison_navigation(&operation, &negotiated, &commands)
            .unwrap_err()
            .code,
        ErrorCode::InvalidRequest
    );
    commands.insert("neighbor".into(), serde_json::from_value(serde_json::json!({
        "permissions":[], "parameters":{"type":"record", "fields":{"path":{"type":"string", "max_bytes":100000}}}, "result":{"type":"null"}
    })).unwrap());
    validate_comparison_navigation(&operation, &negotiated, &commands).unwrap();
    if let EditorOperation::OpenDocumentComparison {
        navigation: Some(navigation),
        ..
    } = &mut operation
    {
        navigation.next.as_mut().unwrap().arguments = serde_json::json!({"path":false});
    }
    assert!(validate_comparison_navigation(&operation, &negotiated, &commands).is_err());
    if let EditorOperation::OpenDocumentComparison {
        navigation: Some(navigation),
        ..
    } = &mut operation
    {
        navigation.next.as_mut().unwrap().arguments = serde_json::json!({"path":"a".repeat(65536)});
    }
    assert_eq!(
        validate_comparison_navigation(&operation, &negotiated, &commands)
            .unwrap_err()
            .code,
        ErrorCode::InvalidRequest
    );
}
