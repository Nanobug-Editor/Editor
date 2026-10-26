//! An actual independent timeline owns a bounded actor; UI painting never calls into WASM synchronously.
use super::*;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc,
};

/// Input follows the public Manager, without an injected frame-opportunity message.
pub(in crate::ui::plugin::canvas::scene_native_tests) enum Input {
    Ui(ui::UiEvent),
    Command(&'static str, serde_json::Value),
}
/// The adapter has one bounded ingress queue; failures remain observable rather than silently dropped.
#[derive(Clone)]
pub(in crate::ui::plugin::canvas::scene_native_tests) struct Port {
    tx: mpsc::SyncSender<Input>,
    queued: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
    overflow: Arc<AtomicUsize>,
}
impl Port {
    /// Admission only queues real UI envelopes or declared guest commands.
    pub fn send(&self, input: Input) -> anyhow::Result<()> {
        let count = self.queued.fetch_add(1, Ordering::AcqRel) + 1;
        self.peak.fetch_max(count, Ordering::Relaxed);
        if self.tx.try_send(input).is_err() {
            self.queued.fetch_sub(1, Ordering::AcqRel);
            self.overflow.fetch_add(1, Ordering::Relaxed);
            anyhow::bail!("timeline native actor ingress is full or closed");
        }
        Ok(())
    }
}
/// A complete accepted publication and its ordinary production renderer observations.
pub(in crate::ui::plugin::canvas::scene_native_tests) struct Snapshot {
    pub document: Arc<Document>,
    pub images: super::super::super::super::images::SceneImages,
    pub frames: Vec<Arc<plugin_runtime::CanvasFrameResource>>,
    pub state: serde_json::Value,
    pub bytes: u64,
    pub publications: u64,
    pub expected_publications: u64,
    pub missing_publication_bytes: u64,
    pub observation_dropped: u64,
    pub resources: usize,
    pub queue_peak: usize,
    pub queue_overflow: usize,
    pub error: Option<String>,
}
/// Only the newest accepted render snapshot waits for the UI, while all publication bytes are observed.
pub(in crate::ui::plugin::canvas::scene_native_tests) struct Actor {
    pub port: Port,
    latest: Arc<Mutex<Option<Snapshot>>>,
    running: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Actor {
    /// Compile/activate the actual package before opening the measured window, using its declared grants.
    pub fn start() -> anyhow::Result<Self> {
        let package = std::env::var_os("NANOBUG_CANVAS_TIMELINE_PACKAGE").map_or_else(
            || Ok(test_packages::package("canvas-timeline-1.0.0")),
            |path| plugin_runtime::Package::read(std::path::Path::new(&path)),
        )?;
        let mut owner = super::open(package, "canvas-timeline", "timeline");
        let observations = owner.manager.runtime_observations();
        observations.start_capture();
        let publication_base = observations
            .snapshot(Some("canvas-timeline"))
            .owners
            .iter()
            .map(|owner| owner.calls.get("publication").copied().unwrap_or(0))
            .sum::<u64>();
        owner
            .manager
            .invoke_command("canvas-timeline", "resync", serde_json::Value::Null)?;
        let (tx, rx) = mpsc::sync_channel(128);
        let port = Port {
            tx,
            queued: Default::default(),
            peak: Default::default(),
            overflow: Default::default(),
        };
        let latest = Arc::new(Mutex::new(None));
        let published = latest.clone();
        let running = Arc::new(AtomicBool::new(true));
        let live = running.clone();
        let queue = port.clone();
        let thread = std::thread::spawn(move || {
            let mut renderer = super::super::super::super::images::VectorRenderer::default();
            let mut sampled = Instant::now() - Duration::from_secs(1);
            let mut capture = Instant::now();
            let mut last_publication = 0;
            let mut bytes = 0_u64;
            let mut publications = 0_u64;
            let mut missing_publication_bytes = 0_u64;
            let mut observation_dropped = 0_u64;
            let mut error = None;
            let mut previous = None::<Arc<Document>>;
            let mut native_events = std::collections::VecDeque::new();
            while live.load(Ordering::Acquire) {
                match rx.recv_timeout(Duration::from_millis(16)) {
                    Ok(input) => {
                        queue.queued.fetch_sub(1, Ordering::AcqRel);
                        let result = match input {
                            Input::Ui(event) => {
                                // A bounded original-envelope history distinguishes missing/reordered
                                // actual presentations; no event is rewritten or replayed.
                                native_events.push_back(event.clone());
                                if native_events.len() > 8 {
                                    native_events.pop_front();
                                }
                                owner.manager.event(
                                    "canvas-timeline",
                                    Some("timeline".into()),
                                    protocol::api::Notification::Ui(event),
                                ).map_err(|failure| failure.context(format!(
                                    "original native events={native_events:?}; accepted UI={}; current root viewport={:?}",
                                    owner.manager.live["canvas-timeline"].views["timeline"].revision,
                                    owner.manager.live["canvas-timeline"].views["timeline"].root.viewport)))
                            }
                            Input::Command(id, args) => owner
                                .manager
                                .invoke_command("canvas-timeline", id, args)
                                .map(|_| ()),
                        };
                        if let Err(failure) = result {
                            error = Some(format!("timeline public Manager: {failure:#}"));
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
                owner.manager.poll();
                let document = owner.manager.live["canvas-timeline"].views["timeline"].clone();
                let changed = previous
                    .as_ref()
                    .is_none_or(|old| !Arc::ptr_eq(old, &document));
                if changed || sampled.elapsed() >= Duration::from_millis(100) || error.is_some() {
                    let observed = observations.snapshot(Some("canvas-timeline"));
                    let expected_publications = observed
                        .owners
                        .iter()
                        .map(|owner| owner.calls.get("publication").copied().unwrap_or(0))
                        .sum::<u64>()
                        .saturating_sub(publication_base);
                    for record in observed.records.iter().filter(|record| {
                        record.stage == plugin_runtime::observability::Stage::Publication
                            && record.id > last_publication
                    }) {
                        match record.bytes {
                            Some(size) => bytes = bytes.saturating_add(size),
                            None => missing_publication_bytes += 1,
                        }
                        publications += 1;
                    }
                    last_publication = observed
                        .records
                        .iter()
                        .filter(|record| {
                            record.stage == plugin_runtime::observability::Stage::Publication
                        })
                        .map(|record| record.id)
                        .max()
                        .unwrap_or(last_publication);
                    let total_dropped = observation_dropped + observed.dropped;
                    if capture.elapsed() >= Duration::from_secs(45) {
                        observation_dropped += observed.dropped;
                        observations.start_capture();
                        capture = Instant::now();
                    }
                    let images = renderer.prepare(&BTreeMap::from([(
                        "canvas-timeline/timeline".into(),
                        document.clone(),
                    )]));
                    let state = match owner
                        .manager
                        .live
                        .get_mut("canvas-timeline")
                        .unwrap()
                        .snapshot()
                        .and_then(|snapshot| {
                            serde_json::from_str(&snapshot.data).map_err(Into::into)
                        }) {
                        Ok(state) => state,
                        Err(failure) => {
                            error = Some(format!("timeline public state snapshot: {failure:#}"));
                            serde_json::Value::Null
                        }
                    };
                    let frames = owner
                        .manager
                        .take_canvas_frame_requests()
                        .into_iter()
                        .filter(|(id, _)| id == "canvas-timeline")
                        .map(|(_, request)| request)
                        .collect();
                    let resources = owner.manager.live["canvas-timeline"].resource_count();
                    *published.lock().unwrap() = Some(Snapshot {
                        document: document.clone(),
                        images,
                        frames,
                        state,
                        bytes,
                        publications,
                        expected_publications,
                        missing_publication_bytes,
                        observation_dropped: total_dropped,
                        resources,
                        queue_peak: queue.peak.load(Ordering::Relaxed),
                        queue_overflow: queue.overflow.load(Ordering::Relaxed),
                        error: error.clone(),
                    });
                    previous = Some(document);
                    sampled = Instant::now();
                }
            }
        });
        Ok(Self {
            port,
            latest,
            running,
            thread: Some(thread),
        })
    }
    /// Consume a complete latest snapshot without retaining an unbounded UI publication queue.
    pub fn take(&self) -> Option<Snapshot> {
        self.latest.lock().unwrap().take()
    }
    /// Retirement precedes temporary-profile deletion and never depends on a GUI callback.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
impl Drop for Actor {
    fn drop(&mut self) {
        self.stop();
    }
}
