//! Real native children retain their observation until the ordinary EOF/tree retirement receipt.
use super::*;
use crate::observability::{Outcome, Owner, RuntimeObservations, Stage};

/// A harmless child command uses Rust's own test harness; no project tool or shell is invoked.
#[test]
fn process_observation_finishes_after_native_exit() {
    let sink = RuntimeObservations::default();
    sink.start_capture();
    let mut processes = Processes::default();
    let id = processes
        .spawn_stdio(
            &std::env::current_exe().unwrap(),
            &["--list".into()],
            &std::env::current_dir().unwrap(),
            &Default::default(),
        )
        .unwrap();
    processes.observe(
        id,
        sink.begin(
            Owner::new("fixture", "native-owner", "private-scope"),
            Stage::Native,
            None,
        ),
    );
    let until = Instant::now() + Duration::from_secs(10);
    let exited = loop {
        let updates = processes.poll_native().unwrap();
        if let Some((_, update)) = updates
            .into_iter()
            .find(|(_, update)| matches!(update, Update::Exited { .. }))
        {
            break update;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(matches!(exited, Update::Exited { code: 0 }));
    let snapshot = sink.snapshot(None);
    assert_eq!(snapshot.records.len(), 1);
    assert_eq!(snapshot.records[0].outcome, Outcome::Completed);
    assert!(
        snapshot.records[0].bytes.is_none(),
        "unmeasured native bytes are unavailable, not fabricated zero"
    );
}

/// Joining and dropping failed cleanup work preserves its factual failure beyond the reaper slot.
#[test]
fn process_observation_retains_failed_reaper_evidence() {
    let mut processes = Processes::default();
    processes.reapers.push(std::thread::spawn(|| {
        anyhow::bail!("Controlled retirement failure")
    }));
    assert!(processes.wait_closed().is_err());
    assert_eq!(processes.cleanup_evidence(), (0, true));
    processes.reap_finished();
    assert_eq!(processes.cleanup_evidence(), (0, true));
}
