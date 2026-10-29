//! Byte projection, admission and real project-sized inputs exercise the shared production path.
use super::*;

/// Repeated comparisons must keep direction, exact byte offsets and same-length replacements distinct.
#[test]
fn reused_comparisons_do_not_confuse_versions_or_line_endings() {
    for _ in 0..3 {
        for (before, after) in [
            ("α\nold\n", "α\nnew\n"),
            ("α\nnew\n", "α\nold\n"),
            ("α\nnew\n", "α\nNEW\n"),
            ("α\nnew\n", "α\nnew"),
            ("α\r\nold\r\n", "α\r\nnew\r\n"),
        ] {
            assert_reconstructs(before, after);
        }
    }
}

/// Apply original-byte hunks and verify unchanged context without assuming a particular edit script.
fn assert_reconstructs(before: &str, after: &str) {
    let hunks = compare(before, after).unwrap();
    let (mut left, mut right) = (0, 0);
    let mut rebuilt = String::new();
    for h in hunks {
        assert!(left <= h.left.start && right <= h.right.start);
        assert_eq!(&before[left..h.left.start], &after[right..h.right.start]);
        rebuilt.push_str(&before[left..h.left.start]);
        rebuilt.push_str(&after[h.right.clone()]);
        left = h.left.end;
        right = h.right.end;
    }
    rebuilt.push_str(&before[left..]);
    assert_eq!(rebuilt, after);
}

/// Source limits still bound metadata, while ordinary >2,000-line sources are admitted.
#[test]
fn source_limits_and_empty_inputs() {
    assert_eq!(
        compare(&"x".repeat(MAX_SOURCE_BYTES + 1), ""),
        Err(ComparisonError::SourceBytes)
    );
    assert_eq!(
        compare(&"\n".repeat(MAX_SOURCE_LINES + 1), ""),
        Err(ComparisonError::SourceLines)
    );
    assert!(
        compare(
            &"x\n".repeat(MAX_SOURCE_LINES),
            &"x\n".repeat(MAX_SOURCE_LINES)
        )
        .unwrap()
        .is_empty()
    );
    for (before, after) in [
        ("", ""),
        ("", "新😀"),
        ("旧", ""),
        ("a\n", "a"),
        ("a", "a\n"),
        ("a\na\nb\na\n", "a\nb\na\na\n"),
    ] {
        assert_reconstructs(before, after);
    }
}

/// Equality keys ignore platform EOL, while byte ranges continue to address each source exactly.
#[test]
fn unicode_and_platform_line_endings_preserve_native_offsets() {
    assert!(
        compare("same\nlast\n", "same\r\nlast\r\n")
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        compare("same\nold\nlast\n", "same\r\n新😀\r\nlast\r\n").unwrap(),
        vec![Hunk {
            left: 5..9,
            right: 6..15
        }]
    );
    assert_eq!(
        compare("同\r\n旧\r\n尾", "同\r\n新\r\n尾").unwrap(),
        vec![Hunk {
            left: 5..10,
            right: 5..10
        }]
    );
    assert!(!compare("same\n", "same").unwrap().is_empty());
    assert!(!compare("same\r", "same").unwrap().is_empty());
}

/// Use actual checked-in YAML, Rust and lockfile contents instead of testing only short synthetic lines.
#[test]
fn real_project_sources_above_the_old_limit_reconstruct() {
    for text in [
        include_str!("../../../editor-ui/locales/app.yml"),
        include_str!("../../../editor-app/src/run/tests.rs"),
        include_str!("../../../../Cargo.lock"),
    ] {
        assert!(text.lines().count() > 2000);
        let at = text.find('\n').unwrap() + 1;
        let changed = format!("{}新增😀\n{}", &text[..at], &text[at..]);
        assert_reconstructs(text, &changed);
    }
}

/// A scene cannot evade the shared ledger with identical inputs; failed admission spends nothing.
#[test]
fn row_budget_is_linear_and_charged_before_interning() {
    let mut budget = 5;
    assert!(line_changes(&["a", "b"], &["c", "d"], &mut budget).is_ok());
    assert_eq!(budget, 1);
    assert_eq!(
        line_changes(&["same"], &["same"], &mut budget),
        Err(ComparisonError::WorkBudget)
    );
    assert_eq!(budget, 1);
}
