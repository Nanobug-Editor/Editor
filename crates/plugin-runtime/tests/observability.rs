//! Runtime diagnostics must remain bounded and never export scope paths or payload data.
use plugin_runtime::observability::{Outcome, Owner, RuntimeObservations, Stage};

/// Recording is opt-in, while aggregate observations remain available without detailed capture.
#[test]
fn observations_capture_only_metadata_and_preserve_incarnation() {
    let sink = RuntimeObservations::default();
    let old = Owner::new("query", "instance-old", "C:/private/token=secret");
    sink.begin(old.clone(), Stage::Guest, None)
        .finish(Outcome::Completed);
    assert!(sink.snapshot(Some("query")).records.is_empty());
    sink.start_capture();
    let current = Owner::new("query", "instance-new", "C:/private/token=secret");
    sink.begin(current, Stage::Guest, None)
        .finish(Outcome::Failed);
    let snapshot = sink.snapshot(Some("query"));
    assert_eq!(snapshot.owners.len(), 2);
    assert_eq!(snapshot.records.len(), 1);
    let encoded = serde_json::to_string(&snapshot).unwrap();
    assert!(!encoded.contains("private"));
    assert!(!encoded.contains("secret"));
    assert!(encoded.contains("instance-new"));
}

/// A noisy plugin cannot allocate unbounded traces or make truncation look like complete evidence.
#[test]
fn observations_bound_noisy_capture_and_keep_other_plugin_records() {
    let sink = RuntimeObservations::default();
    sink.start_capture();
    sink.begin(
        Owner::new("quiet", "quiet-instance", "scope"),
        Stage::Guest,
        None,
    )
    .finish(Outcome::Completed);
    for _ in 0..600 {
        sink.begin(
            Owner::new("noisy", "noisy-instance", "scope"),
            Stage::Guest,
            None,
        )
        .finish(Outcome::Completed);
    }
    let snapshot = sink.snapshot(None);
    assert!(snapshot.partial);
    assert_eq!(snapshot.dropped, 88);
    assert_eq!(snapshot.records.len(), 513);
    assert_eq!(sink.snapshot(Some("quiet")).records.len(), 1);
    sink.stop_capture();
    sink.begin(
        Owner::new("quiet", "quiet-instance", "scope"),
        Stage::Guest,
        None,
    )
    .finish(Outcome::Completed);
    assert_eq!(sink.snapshot(Some("quiet")).records.len(), 1);
}

/// Identical local incarnation labels never merge plugins or logical workspaces.
#[test]
fn observations_keep_owner_scope_and_plugin_independent() {
    let sink = RuntimeObservations::default();
    for (plugin, scope) in [("one", "scope-a"), ("one", "scope-b"), ("two", "scope-a")] {
        sink.begin(
            Owner::new(plugin, "same-local-label", scope),
            Stage::Guest,
            None,
        )
        .finish(Outcome::Completed);
    }
    assert_eq!(sink.snapshot(None).owners.len(), 3);
    assert_eq!(sink.snapshot(Some("one")).owners.len(), 2);
}

/// Losing an old owner's final receipt stays visible even after a later capture starts.
#[test]
fn observations_summary_loss_survives_capture_restart() {
    use plugin_runtime::observability::ResourceCounts;
    let sink = RuntimeObservations::default();
    for instance in 0..513 {
        sink.resources(
            Owner::new("fixture", &format!("owner-{instance}"), "scope"),
            ResourceCounts {
                retired: true,
                ..Default::default()
            },
        );
    }
    assert!(sink.snapshot(None).partial);
    sink.start_capture();
    assert!(sink.snapshot(None).partial);
    assert_eq!(sink.snapshot(None).owners.len(), 512);
}

/// An admitted long SemVer cannot turn disabled detailed capture into unbounded retained metadata.
#[test]
fn observations_omit_oversized_package_metadata_and_mark_it_incomplete() {
    let sink = RuntimeObservations::default();
    let mut owner = Owner::new("fixture", "instance", "scope");
    owner.package_version = Some(format!("1.0.0+{}", "v".repeat(1024 * 1024)));
    sink.resources(owner, Default::default());
    let snapshot = sink.snapshot(None);
    assert!(snapshot.partial);
    assert!(snapshot.owners[0].owner.package_version.is_none());
    assert!(serde_json::to_vec(&snapshot).unwrap().len() < 4096);
}

/// Compare the exact same metadata workload; report overhead without a flaky absolute-time assertion.
#[test]
#[ignore = "explicit observation-layer wall-time comparison; run serially with --nocapture"]
fn observations_capture_overhead_fixed_workload() {
    let mut results = [Vec::new(), Vec::new()];
    for round in 0..8 {
        let capture = round % 2 == 1;
        let sink = RuntimeObservations::default();
        if capture {
            sink.start_capture();
        }
        let owner = Owner::new("fixture", "same-instance", "same-scope");
        let started = std::time::Instant::now();
        for _ in 0..10_000 {
            sink.begin(owner.clone(), Stage::Guest, None)
                .finish(Outcome::Completed);
        }
        let elapsed = started.elapsed().as_micros();
        let snapshot = sink.snapshot(None);
        assert_eq!(snapshot.owners[0].calls["guest"], 10_000);
        assert!(snapshot.records.len() <= 512);
        if round >= 2 {
            results[usize::from(capture)].push(elapsed);
        }
    }
    for values in &mut results {
        values.sort_unstable();
    }
    eprintln!(
        "OBSERVATION_MICROBENCH iterations=10000 off_us={} on_us={} (median wall time; no guest/native CPU claim)",
        results[0][1], results[1][1]
    );
}
