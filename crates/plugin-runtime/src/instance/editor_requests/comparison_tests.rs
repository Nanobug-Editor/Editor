//! Comparison admission checks added source semantics before native resources are allocated.
use super::*;

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
