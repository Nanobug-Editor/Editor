//! The public bounded worker runs the actual durable store rather than mocking its collaborators.

use editor_history::{Capture, History, Policy, Reason, Worker};

/// A host can await durable capture and later reading without performing storage on its UI thread.
#[test]
fn background_worker_commits_before_acknowledging_a_version() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("history");
    let path = directory.path().join("note.txt");
    let worker = Worker::start(move || History::open(root, Policy::default())).unwrap();
    let version = futures::executor::block_on(worker.call(5, move |history| {
        history
            .capture(Capture {
                file: None,
                path,
                workspace: None,
                contents: b"bytes".to_vec(),
                reason: Reason::Saved,
                at_ms: 1_000,
            })
            .map(|outcome| outcome.version().unwrap())
    }))
    .unwrap();
    let bytes =
        futures::executor::block_on(worker.call(5, move |history| history.read(version))).unwrap();
    assert_eq!(bytes, b"bytes");
}

/// Busy callers cannot restart the timer and indefinitely defer expiration/compaction.
#[test]
fn executor_requests_leave_the_existing_maintenance_deadline_running() {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("history");
    let timers = Arc::new(AtomicUsize::new(0));
    let created = timers.clone();
    let thread = Arc::new(Mutex::new(None));
    let spawned = thread.clone();
    let worker = Worker::on_executor(
        move || History::open(root, Policy::default()),
        move |future| {
            *spawned.lock().unwrap() = Some(std::thread::spawn(move || {
                futures::executor::block_on(future)
            }));
        },
        move |_| {
            created.fetch_add(1, Ordering::SeqCst);
            Box::pin(futures::future::pending())
        },
    );
    for _ in 0..50 {
        futures::executor::block_on(worker.call(0, |history| history.stats())).unwrap();
    }
    assert_eq!(timers.load(Ordering::SeqCst), 1);
    drop(worker);
    thread.lock().unwrap().take().unwrap().join().unwrap();
}

/// Dropping an awaiter does not free bytes still retained by its accepted queued operation.
#[test]
fn queued_payload_budget_survives_caller_cancellation() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("history");
    let (release, held) = std::sync::mpsc::channel();
    let worker = Worker::start(move || {
        held.recv().unwrap();
        History::open(root, Policy::default())
    })
    .unwrap();
    let first = worker.call(64 * 1024 * 1024, |_| Ok(()));
    assert!(matches!(
        futures::executor::block_on(worker.call(1, |_| Ok(()))),
        Err(editor_history::Error::Busy)
    ));
    drop(first);
    let still_full = futures::executor::block_on(worker.call(1, |_| Ok(())));
    release.send(()).unwrap();
    futures::executor::block_on(worker.call(0, |_| Ok(()))).unwrap();
    assert!(matches!(still_full, Err(editor_history::Error::Busy)));
    futures::executor::block_on(worker.call(1, |_| Ok(()))).unwrap();
}

/// Metadata-only work is bounded too, even before storage initialization finishes.
#[test]
fn metadata_queue_exhaustion_returns_busy_without_blocking() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("history");
    let (release, held) = std::sync::mpsc::channel();
    let worker = Worker::start(move || {
        held.recv().unwrap();
        History::open(root, Policy::default())
    })
    .unwrap();
    let queued = (0..16)
        .map(|index| worker.call(0, move |_| Ok(index)))
        .collect::<Vec<_>>();
    let full = futures::executor::block_on(worker.call(0, |_| Ok(())));
    release.send(()).unwrap();
    assert!(matches!(full, Err(editor_history::Error::Busy)));
    for (index, reply) in queued.into_iter().enumerate() {
        assert_eq!(futures::executor::block_on(reply).unwrap(), index);
    }
}
