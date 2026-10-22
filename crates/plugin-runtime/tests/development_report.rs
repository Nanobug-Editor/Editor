//! A development session must never certify an executable plugin that was left dormant.
use plugin_runtime::{
    development::report::{CheckStatus, DevelopmentReport, Termination},
    validation::ToolIdentity,
};

/// A clean child exit is insufficient without activation and authoritative retirement evidence.
#[test]
fn development_report_does_not_pass_without_observed_activation() {
    let mut report = DevelopmentReport::new(
        "session-a",
        "query",
        "1.0.0",
        "digest",
        true,
        ToolIdentity::default(),
    );
    report.finish(Termination::Normal);
    assert_eq!(report.activation, CheckStatus::NotExercised);
    assert_eq!(report.status, CheckStatus::Inconclusive);
    assert_eq!(report.exit_code(), 2);
    let encoded = serde_json::to_string(&report).unwrap();
    assert!(!encoded.contains("workspace_path"));
}

/// A committed instance's actual retired maps can pass lifecycle checks while business remains untested.
#[test]
fn development_report_requires_matching_session_and_final_owner_receipt() {
    use plugin_runtime::{
        RuntimeObservations,
        development::report::Checkpoint,
        observability::{Owner, ResourceCounts},
    };
    let sink = RuntimeObservations::default();
    let owner = Owner::new("query", "real-instance", "private-workspace");
    sink.resources(
        owner.clone(),
        ResourceCounts {
            committed: true,
            handles: 2,
            ..Default::default()
        },
    );
    sink.resources(
        owner,
        ResourceCounts {
            retired: true,
            ..Default::default()
        },
    );
    let mut report = DevelopmentReport::new(
        "session-a",
        "query",
        "1.0.0",
        "digest",
        true,
        ToolIdentity::default(),
    );
    let mut checkpoint = Checkpoint {
        schema_version: 1,
        session: "old-session".into(),
        stopped: true,
        http_producers: 0,
        reloads: vec![],
        partial: false,
        observations: sink.snapshot(Some("query")),
    };
    assert!(!report.observe(checkpoint.clone()));
    checkpoint.session = "session-a".into();
    assert!(report.observe(checkpoint));
    report.finish(Termination::Normal);
    assert_eq!(report.status, CheckStatus::Passed);
    assert_eq!(report.business, CheckStatus::NotExercised);
    assert_eq!(report.exit_code(), 0);
}

/// A lost native confirmation cannot become successful cleanup merely because authority maps are empty.
#[test]
fn development_report_keeps_failed_native_retirement_inconclusive() {
    use plugin_runtime::{
        RuntimeObservations,
        development::report::Checkpoint,
        observability::{Owner, ResourceCounts},
    };
    let sink = RuntimeObservations::default();
    sink.resources(
        Owner::new("query", "instance", "scope"),
        ResourceCounts {
            committed: true,
            retired: true,
            native_cleanup_unconfirmed: true,
            ..Default::default()
        },
    );
    let mut report = DevelopmentReport::new(
        "current",
        "query",
        "1.0.0",
        "digest",
        true,
        ToolIdentity::default(),
    );
    report.observe(Checkpoint {
        schema_version: 1,
        session: "current".into(),
        stopped: true,
        http_producers: 0,
        reloads: vec![],
        partial: false,
        observations: sink.snapshot(Some("query")),
    });
    report.finish(Termination::Normal);
    assert_eq!(report.cleanup, CheckStatus::Inconclusive);
    assert_eq!(report.exit_code(), 2);
}

/// Omitted identity metadata cannot grow periodic checkpoints or turn incomplete evidence into pass.
#[test]
fn development_report_bounds_valid_oversized_versions_and_marks_omission() {
    let mut report = DevelopmentReport::new(
        "current",
        "query",
        &format!("1.0.0-{}", "a".repeat(1024 * 1024)),
        "digest",
        false,
        ToolIdentity::default(),
    );
    report.observe(plugin_runtime::development::report::Checkpoint {
        schema_version: 1,
        session: "current".into(),
        stopped: true,
        http_producers: 0,
        reloads: vec![],
        partial: false,
        observations: plugin_runtime::RuntimeObservations::default().snapshot(None),
    });
    assert_eq!(report.cleanup, CheckStatus::Passed);
    assert!(report.version.is_empty());
    assert!(report.metadata_partial);
    assert_eq!(report.version_sha256.as_deref().map(str::len), Some(64));
    report.finish(Termination::Normal);
    assert_eq!(report.exit_code(), 2);
    assert!(serde_json::to_vec(&report).unwrap().len() < 8192);
}
