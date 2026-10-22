//! Host-only, bounded metadata observations. They never own resources or determine request outcomes.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Safe diagnostic identity; the actual scope path is replaced before anything is stored.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Owner {
    /// Identity metadata was omitted at the diagnostics boundary; completeness cannot be inferred.
    pub metadata_partial: bool,
    /// True for an actual runtime incarnation; false for queue/demand metadata with no resource authority.
    pub resource_owner: bool,
    /// Validated manifest identifier, bounded independently of any guest-provided payload.
    pub plugin: String,
    /// One actual incarnation, including preparation candidates and later restored instances.
    pub instance: String,
    /// Opaque digest of the logical scope; never an OS path.
    pub scope: String,
    /// Exact Package digest when attached by the package-owning manager.
    pub package_digest: Option<String>,
    /// Manifest package version; separate from executable, SDK and capability versions.
    pub package_version: Option<String>,
}
impl Owner {
    /// Build an identity for `plugin` and `instance`; hash `scope` before retention/export.
    pub fn new(plugin: &str, instance: &str, scope: &str) -> Self {
        Self {
            metadata_partial: false,
            resource_owner: true,
            plugin: bounded(plugin),
            instance: bounded(instance),
            scope: {
                static SALT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
                let salt = SALT.get_or_init(session_identity);
                format!(
                    "{:x}",
                    Sha256::digest(format!("{salt}\0{scope}").as_bytes())
                )
            },
            package_digest: None,
            package_version: None,
        }
    }
    /// Administrative work before an incarnation exists; this identity must not participate in leak checks.
    pub fn host_operation(plugin: &str, scope: &str) -> Self {
        let mut owner = Self::new(plugin, "", scope);
        owner.instance = format!(
            "host-{:x}",
            Sha256::digest(format!("{}\0{}", owner.plugin, owner.scope).as_bytes())
        );
        owner.resource_owner = false;
        owner
    }

    /// Keep diagnostic identities bounded even when a trusted host caller supplies oversized metadata.
    fn bounded_metadata(mut self) -> Self {
        if self
            .package_version
            .as_ref()
            .is_some_and(|version| version.len() > 128)
        {
            self.package_version = None;
            self.metadata_partial = true;
        }
        if self.package_digest.as_ref().is_some_and(|digest| {
            digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        }) {
            self.package_digest = None;
            self.metadata_partial = true;
        }
        self
    }
}

/// Static demand categories explain activation without retaining commands, arguments or document paths.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivationTrigger {
    Command,
    View,
    Service,
    Language,
    Background,
}

/// Only host-selected stages can be recorded; no guest strings become arbitrary metric labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Package preparation before business activation has actually succeeded.
    Preparation,
    /// Time between actor admission and dispatch, including waits behind other actor work.
    Queue,
    /// Demand-triggered preparation or commit; a trigger is not a claim that a service is ready.
    Activation,
    /// Whole guest callback wall-clock time, including nested synchronous host imports.
    Guest,
    /// Synchronous imported host work, nested within Guest and not additive to it.
    HostCall,
    /// An accepted editor/service request's actual completion gate.
    Request,
    /// Network waiting; not remote server CPU time.
    Http,
    /// Native process or dependency preparation; not process CPU/RSS.
    Native,
    /// Host UI validation/publication, not frame drawing.
    Publication,
}

/// A diagnostic outcome never changes the business completion gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Actual operation completion, not mere acceptance.
    Completed,
    /// The existing operation reported failure.
    Failed,
    /// Waiting or ownership was cancelled; external effects may have happened.
    Cancelled,
    /// Deadline expired without a successful completion.
    TimedOut,
    /// Observer ended without enough evidence; never reported as successful cleanup.
    Unknown,
}

/// Actual owner counts copied from resource maps; these values never retain resource Arcs.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ResourceCounts {
    /// This incarnation reached the normal activation commit point at least at this sample.
    pub committed: bool,
    /// All live authority slots, including file roots and process references.
    pub handles: usize,
    /// Outstanding editor and service result waits.
    pub requests: usize,
    /// Streams still owned by this incarnation.
    pub http: usize,
    /// Disposable timers still retained by the owner.
    pub timers: usize,
    /// Document/workspace/preference subscriptions.
    pub subscriptions: usize,
    /// Owned trees, virtual documents, diagnostics and decorations.
    pub projections: usize,
    /// Handles referring to native processes; not proof of native exit after revocation.
    pub processes: usize,
    /// Actual native EOF/tree reapers still running after guest authority was revoked.
    pub native_pending: usize,
    /// A native reaper failed or panicked; empty handle maps cannot certify OS cleanup afterwards.
    pub native_cleanup_unconfirmed: bool,
    /// Instance authority has been revoked; this alone does not prove OS work has exited.
    pub retired: bool,
}

/// One bounded metadata record. No payload, argv, HTTP target or original error message is stored.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    /// Actual demand category when this is an activation wait; otherwise unavailable.
    pub trigger: Option<ActivationTrigger>,
    /// Host-generated identity; guest request IDs are not globally unique.
    pub id: u64,
    /// Parent host operation if a concrete relationship is known.
    pub parent: Option<u64>,
    /// Original authenticated owner, never the manager's later selected workspace.
    pub owner: Owner,
    /// Non-additive stage described by Stage.
    pub stage: Stage,
    /// Microseconds since the process-local observation sink was created.
    pub started_us: u64,
    /// Monotonic elapsed wall-clock microseconds.
    pub elapsed_us: u64,
    /// The actual observed terminal category.
    pub outcome: Outcome,
    /// Guest instructions consumed when measured; never presented as CPU time.
    pub fuel: Option<u64>,
    /// Already encoded/observed data length; None when unmeasured. Never serialize UI again to count it.
    pub bytes: Option<u64>,
}

/// Lightweight counters plus a live resource snapshot for one incarnation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Summary {
    /// Sticky evidence of a committed activation, retained after final resource retirement.
    pub ever_committed: bool,
    /// Owner identity; old and restored versions remain separate.
    pub owner: Owner,
    /// Completed observations by stage, even when detailed capture is off.
    pub calls: BTreeMap<String, u64>,
    /// Wall-clock sums by stage, which can overlap and must not be summed as total CPU time.
    pub elapsed_us: BTreeMap<String, u64>,
    /// Actual failures observed, distinct from normal cancellation.
    pub failures: u64,
    /// Stage-local failures distinguish an unhandled guest fault from a handled host-call refusal.
    pub failed_calls: BTreeMap<String, u64>,
    /// Current authoritative owner-map projection; None means not sampled.
    pub resources: Option<ResourceCounts>,
}

/// Safe immutable export, independent from ordinary runtime logs and their read state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    /// Stable namespace for this running observer, distinct from every previous host session.
    pub session: String,
    /// Monotonic start/end microseconds within the host session, absent before any capture.
    pub capture_range_us: Option<(u64, u64)>,
    /// Report format version, not the guest wire protocol.
    pub schema_version: u32,
    /// Whether a bounded detailed capture is currently accepting records.
    pub capture_active: bool,
    /// Records/owners were omitted; absence cannot prove that work did not happen.
    pub partial: bool,
    /// Number of omitted or evicted detailed observations.
    pub dropped: u64,
    /// Captured incarnations, optionally filtered to one plugin.
    pub owners: Vec<Summary>,
    /// Recent detailed records; empty while no capture has ever been started.
    pub records: Vec<Record>,
    /// Stages for which this sink has no observation, not fabricated zero measurements.
    pub unobserved: Vec<Stage>,
    /// Finer seams not yet instrumented; a broad stage measurement does not certify these details.
    pub unobserved_details: Vec<String>,
}
impl Snapshot {
    /// Atomically export only safe metadata to the caller-selected `path`; IO failures preserve old bytes.
    pub fn write(&self, path: &std::path::Path) -> std::io::Result<()> {
        write_report(path, self)
    }
}

/// Write a host report atomically beside `path`; callers establish destination consent and schema safety.
pub fn write_report(path: &std::path::Path, report: &impl Serialize) -> std::io::Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut file, report).map_err(std::io::Error::other)?;
    file.flush()?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

/// Mutable observation storage has strict record, byte and historical-owner bounds.
struct State {
    /// An opaque diagnostics namespace, with no execution authority or persisted user identifier.
    session: String,
    /// Actual monotonic capture window, retained after explicit stop.
    capture_range: Option<(Instant, Instant)>,
    /// Summary loss is session-wide evidence loss, not reset by starting a fresh detailed capture.
    history_partial: bool,
    origin: Instant,
    next: u64,
    capture: Option<Instant>,
    records: VecDeque<(Record, usize)>,
    bytes: usize,
    dropped: u64,
    owners: BTreeMap<(String, String, String), Summary>,
}

/// Shared host observer; no resource ownership or cross-plugin execution authority is exposed.
#[derive(Clone)]
pub struct RuntimeObservations(Arc<Mutex<State>>);
impl Default for RuntimeObservations {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(State {
            session: session_identity(),
            capture_range: None,
            history_partial: false,
            origin: Instant::now(),
            next: 0,
            capture: None,
            records: VecDeque::new(),
            bytes: 0,
            dropped: 0,
            owners: BTreeMap::new(),
        })))
    }
}
impl std::fmt::Debug for RuntimeObservations {
    /// Debug formatting never includes original source data or locks the full observation stream.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RuntimeObservations")
    }
}
impl RuntimeObservations {
    /// Start a fresh detailed capture, automatically ending after 60 seconds; aggregates remain.
    pub fn start_capture(&self) {
        let mut state = self.lock();
        let started = Instant::now();
        let until = started + Duration::from_secs(60);
        state.capture = Some(until);
        state.capture_range = Some((started, until));
        state.records.clear();
        state.bytes = 0;
        state.dropped = 0;
    }

    /// Stop accepting detailed records without deleting captured evidence or aggregate counters.
    pub fn stop_capture(&self) {
        let mut state = self.lock();
        state.capture = None;
        if let Some((_, end)) = &mut state.capture_range {
            *end = (*end).min(Instant::now());
        }
    }

    /// Begin `stage` for the original `owner`; optional `parent` must originate from this host.
    pub fn begin(&self, owner: Owner, stage: Stage, parent: Option<u64>) -> Observation {
        let owner = owner.bounded_metadata();
        let mut state = self.lock();
        let id = state
            .next
            .checked_add(1)
            .map(|next| {
                state.next = next;
                next
            })
            .unwrap_or(0);
        Observation {
            sink: self.clone(),
            record: Some(Record {
                trigger: None,
                id,
                parent,
                owner,
                stage,
                started_us: micros(state.origin.elapsed()),
                elapsed_us: 0,
                outcome: Outcome::Unknown,
                fuel: None,
                bytes: None,
            }),
            started: Instant::now(),
        }
    }

    /// Copy authoritative `resources` for this actual incarnation, without retaining a resource.
    pub fn resources(&self, owner: Owner, resources: ResourceCounts) {
        let owner = owner.bounded_metadata();
        let mut state = self.lock();
        if let Some(summary) = summary(&mut state, owner) {
            summary.ever_committed |= resources.committed;
            summary.resources = Some(resources);
        }
    }

    /// Read a safe snapshot for `plugin`, or all observed plugins when None; does no guest work.
    pub fn snapshot(&self, plugin: Option<&str>) -> Snapshot {
        let state = self.lock();
        let matches = |owner: &Owner| plugin.is_none_or(|id| id == owner.plugin);
        let stages = [
            Stage::Queue,
            Stage::Preparation,
            Stage::Activation,
            Stage::Guest,
            Stage::HostCall,
            Stage::Request,
            Stage::Http,
            Stage::Native,
            Stage::Publication,
        ];
        Snapshot {
            session: state.session.clone(),
            capture_range_us: state.capture_range.map(|(start, end)| {
                (
                    micros(start.duration_since(state.origin)),
                    micros(end.min(Instant::now()).duration_since(state.origin)),
                )
            }),
            unobserved_details: [
                "activation_shared_waiter_count",
                "demand_queue",
                "queue_merge_and_discard",
                "request_begin_and_late_receipts",
                "worker_publication_and_stale_discard",
                "native_dependency_preparation",
                "native_cpu_rss",
                "frame_drawing",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            schema_version: 1,
            capture_active: state.capture.is_some_and(|until| until > Instant::now()),
            partial: state.history_partial
                || state.dropped != 0
                || state
                    .owners
                    .values()
                    .any(|owner| owner.owner.metadata_partial),
            dropped: state.dropped,
            owners: state
                .owners
                .values()
                .filter(|item| matches(&item.owner))
                .cloned()
                .collect(),
            records: state
                .records
                .iter()
                .filter(|(record, _)| matches(&record.owner))
                .map(|(record, _)| record.clone())
                .collect(),
            unobserved: stages
                .into_iter()
                .filter(|stage| {
                    !state.owners.values().any(|item| {
                        matches(&item.owner) && item.calls.contains_key(stage_name(*stage))
                    })
                })
                .collect(),
        }
    }

    /// Poisoned diagnostics recover their own storage; never propagate a diagnostic panic into business work.
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// Preserve aggregates and bound captures without invoking guest code, disk or network IO.
    fn record(&self, mut record: Record) {
        let mut state = self.lock();
        if let Some(summary) = summary(&mut state, record.owner.clone()) {
            record.owner = summary.owner.clone();
            let key = stage_name(record.stage).to_owned();
            let count = summary.calls.entry(key.clone()).or_default();
            *count = count.saturating_add(1);
            let elapsed = summary.elapsed_us.entry(key).or_default();
            *elapsed = elapsed.saturating_add(record.elapsed_us);
            summary.failures = summary
                .failures
                .saturating_add(u64::from(record.outcome == Outcome::Failed));
            if record.outcome == Outcome::Failed {
                let failures = summary
                    .failed_calls
                    .entry(stage_name(record.stage).into())
                    .or_default();
                *failures = failures.saturating_add(1);
            }
        }
        if !state.capture.is_some_and(|until| until > Instant::now()) {
            return;
        }
        let bytes = serde_json::to_vec(&record).map_or(4097, |bytes| bytes.len());
        if bytes > 4096 || record.id == 0 {
            state.dropped = state.dropped.saturating_add(1);
            return;
        }
        let plugin_count = state
            .records
            .iter()
            .filter(|(item, _)| item.owner.plugin == record.owner.plugin)
            .count();
        if plugin_count >= 512 {
            if let Some(index) = state
                .records
                .iter()
                .position(|(item, _)| item.owner.plugin == record.owner.plugin)
            {
                if let Some((_, bytes)) = state.records.remove(index) {
                    state.bytes -= bytes;
                    state.dropped = state.dropped.saturating_add(1);
                }
            }
        }
        while state.records.len() >= 4096 || state.bytes + bytes > 4 * 1024 * 1024 {
            if let Some((_, bytes)) = state.records.pop_front() {
                state.bytes -= bytes;
                state.dropped = state.dropped.saturating_add(1);
            } else {
                break;
            }
        }
        state.bytes += bytes;
        state.records.push_back((record, bytes));
    }
}

/// One host observation; dropping without a finish reports Unknown rather than successful completion.
pub struct Observation {
    sink: RuntimeObservations,
    record: Option<Record>,
    started: Instant,
}
impl Observation {
    /// Attach the closed host demand category, never its arbitrary arguments or provider configuration.
    pub fn trigger(&mut self, trigger: ActivationTrigger) {
        if let Some(record) = &mut self.record {
            record.trigger = Some(trigger);
        }
    }
    /// Correlation identity within this sink; zero means the finite identity budget was exhausted.
    pub fn id(&self) -> u64 {
        self.record.as_ref().map_or(0, |record| record.id)
    }
    /// Attach measured instruction consumption; None explicitly means unavailable.
    pub fn fuel(&mut self, fuel: Option<u64>) {
        if let Some(record) = &mut self.record {
            record.fuel = fuel;
        }
    }
    /// Count bytes already encoded or transferred; never inspect or retain their payload.
    pub fn bytes(&mut self, bytes: usize) {
        if let Some(record) = &mut self.record {
            record.bytes = Some(bytes as u64);
        }
    }
    /// Finish exactly once with the actual operation result; diagnostic errors never escape.
    pub fn finish(mut self, outcome: Outcome) {
        if let Some(mut record) = self.record.take() {
            record.outcome = outcome;
            record.elapsed_us = micros(self.started.elapsed());
            self.sink.record(record);
        }
    }
}
impl Drop for Observation {
    fn drop(&mut self) {
        if let Some(mut record) = self.record.take() {
            record.elapsed_us = micros(self.started.elapsed());
            self.sink.record(record);
        }
    }
}

/// Bounded safe labels are manifest/host identity metadata, not arbitrary error text.
fn bounded(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_control())
        .take(128)
        .collect()
}
/// Overflow-safe monotonic duration conversion; counters saturate rather than wrap.
fn micros(value: Duration) -> u64 {
    value.as_micros().min(u128::from(u64::MAX)) as u64
}

/// A per-process nonce plus creation time namespaces diagnostics without exposing either input.
fn session_identity() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let nonce = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |time| time.as_nanos());
    format!(
        "{:x}",
        Sha256::digest(format!("{}:{time}:{nonce}", std::process::id()).as_bytes())
    )
}
/// Small closed label inventory, independent from external request names or payloads.
fn stage_name(stage: Stage) -> &'static str {
    match stage {
        Stage::Queue => "queue",
        Stage::Preparation => "preparation",
        Stage::Activation => "activation",
        Stage::Guest => "guest",
        Stage::HostCall => "host_call",
        Stage::Request => "request",
        Stage::Http => "http",
        Stage::Native => "native",
        Stage::Publication => "publication",
    }
}
/// Bound historical owners; overflowing diagnostic history cannot evict a business resource.
fn summary(state: &mut State, owner: Owner) -> Option<&mut Summary> {
    let key = (
        owner.plugin.clone(),
        owner.scope.clone(),
        owner.instance.clone(),
    );
    if !state.owners.contains_key(&key) && state.owners.len() >= 512 {
        let retired = state
            .owners
            .iter()
            .find(|(_, item)| {
                item.resources
                    .as_ref()
                    .is_some_and(|resources| resources.retired)
            })
            .map(|(key, _)| key.clone());
        state.dropped = state.dropped.saturating_add(1);
        state.history_partial = true;
        if let Some(retired) = retired {
            state.owners.remove(&retired);
        } else {
            return None;
        }
    }
    Some(state.owners.entry(key).or_insert_with(|| Summary {
        ever_committed: false,
        owner,
        calls: BTreeMap::new(),
        elapsed_us: BTreeMap::new(),
        failures: 0,
        failed_calls: BTreeMap::new(),
        resources: None,
    }))
}
