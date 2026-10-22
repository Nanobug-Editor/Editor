//! Controller reports use safe child checkpoints, never parse stdout to infer activation or cleanup.
use super::*;
use plugin_runtime::{
    Manager,
    development::report::{Checkpoint, DevelopmentReport, ReloadOutcome, Termination},
    validation::ToolIdentity,
};
use std::{
    io::Read,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

/// One process's report cadence and bounded candidate outcomes, scoped by its immutable descriptor.
#[derive(Default)]
struct Writer {
    last: Option<Instant>,
    reloads: Vec<ReloadOutcome>,
    partial: bool,
}
/// No child can choose a different controller session or write outside its selected profile.
static WRITER: OnceLock<Mutex<Writer>> = OnceLock::new();

/// Replace stale output before fallible project preparation; no previous run is ever a fallback.
pub(super) fn initialize(path: &Path) -> anyhow::Result<()> {
    let session = format!(
        "pending-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    );
    let mut report = DevelopmentReport::new(&session, "", "", "", true, tool());
    report.admission = plugin_runtime::development::report::CheckStatus::Inconclusive;
    plugin_runtime::observability::write_report(path, &report)?;
    Ok(())
}

/// Preserve this run's bounded evidence; known admission rejection is failure, tooling errors are unknown.
pub(super) fn failure(path: &Path, error: &anyhow::Error) -> anyhow::Result<i32> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() <= 8 * 1024 * 1024,
        "Development report exceeds read bound"
    );
    let mut report: DevelopmentReport = serde_json::from_slice(&bytes)?;
    // anyhow downcasting also finds typed context values; std::error::Error::chain does not.
    if error
        .downcast_ref::<plugin_runtime::validation::Diagnostic>()
        .is_some()
        || error
            .downcast_ref::<plugin_runtime::plugin_protocol::api::NegotiationFailure>()
            .is_some()
    {
        report.admission = plugin_runtime::development::report::CheckStatus::Failed;
    }
    report.finish(Termination::Crashed);
    plugin_runtime::observability::write_report(path, &report)?;
    Ok(report.exit_code())
}

/// Record an actual candidate outcome; installation acceptance is not a claim that WASM activated.
pub(crate) fn candidate(passed: bool) {
    if !instance::context().is_some_and(|descriptor| descriptor.reporting) {
        return;
    }
    let mut writer = WRITER.get_or_init(Default::default).lock().unwrap();
    if writer.reloads.len() >= 64 {
        writer.reloads.remove(0);
        writer.partial = true;
    }
    let generation = writer
        .reloads
        .last()
        .map_or(0, |entry| entry.generation.saturating_add(1));
    writer.reloads.push(ReloadOutcome { generation, passed });
    writer.last = None;
}

/// Save at most one checkpoint per second; final snapshots bypass cadence after real shutdown.
pub(crate) fn checkpoint(manager: &Manager, stopped: bool) {
    let Some(descriptor) = instance::context().filter(|descriptor| descriptor.reporting) else {
        return;
    };
    if stopped {
        // Revoked streams can leave a DNS/OS producer briefly finishing. Wait only in report-mode
        // shutdown, with a finite grace bound; a remaining producer is explicitly inconclusive.
        let deadline = Instant::now() + Duration::from_millis(500);
        while manager.http_producer_count() != 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    let mut writer = WRITER.get_or_init(Default::default).lock().unwrap();
    if !stopped
        && writer
            .last
            .is_some_and(|last| last.elapsed() < Duration::from_secs(1))
    {
        return;
    }
    writer.last = Some(Instant::now());
    let checkpoint = Checkpoint {
        schema_version: 1,
        session: descriptor.session.clone(),
        stopped,
        http_producers: manager.http_producer_count(),
        reloads: writer.reloads.clone(),
        partial: writer.partial,
        observations: manager
            .runtime_observations()
            .snapshot(Some(&descriptor.development)),
    };
    if instance::atomic_json(
        &checkpoint_path(&descriptor.profile, &descriptor.session),
        &checkpoint,
    )
    .is_err()
    {
        writer.partial = true;
    }
}

/// Install only in an isolated child; stop requests enter the same native consent/retirement path as Quit.
pub(crate) fn attach_stop_listener(
    editor: &gpui_kit::Entity<crate::EditorApp>,
    cx: &mut gpui_kit::App,
) {
    let Some(descriptor) = instance::context()
        .filter(|descriptor| descriptor.reporting)
        .cloned()
    else {
        return;
    };
    let editor = editor.downgrade();
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let path = descriptor
                .profile
                .join(format!("stop-{}.json", descriptor.session));
            if path.is_file() {
                // Development uses a standalone EditorApp, outside the project-window router.
                // Its existing close gate retains consent and waits for owned cleanup before Quit.
                cx.update(|cx| {
                    let _ = editor.update(cx, |editor, cx| editor.should_close_window(cx));
                });
                break;
            }
        }
    })
    .detach();
}

/// Exact executable and SDK inputs; hashing never creates a second application/runtime instance.
pub(crate) fn tool() -> ToolIdentity {
    ToolIdentity {
        version: env!("CARGO_PKG_VERSION").into(),
        executable_sha256: std::env::current_exe()
            .ok()
            .and_then(|path| super::check::hash_file(&path).ok()),
        sdk_sha256: Some(crate::sdk_export::digest()),
    }
}

/// Refresh bounded, matching evidence and atomically save a final lifecycle report.
pub(super) fn finish(
    path: &Path,
    report: &mut DevelopmentReport,
    profile: &Path,
    termination: Termination,
) -> anyhow::Result<i32> {
    if let Some(checkpoint) = read_checkpoint(profile, &report.session) {
        report.observe(checkpoint);
    }
    report.finish(termination);
    plugin_runtime::observability::write_report(path, report)?;
    Ok(report.exit_code())
}

/// Read only the expected private checkpoint, bounded even if a stale file is unexpectedly large.
pub(super) fn read_checkpoint(profile: &Path, session: &str) -> Option<Checkpoint> {
    let mut bytes = Vec::new();
    std::fs::File::open(checkpoint_path(profile, session))
        .ok()?
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 8 * 1024 * 1024 {
        return None;
    }
    let checkpoint: Checkpoint = serde_json::from_slice(&bytes).ok()?;
    (checkpoint.session == session && checkpoint.schema_version == 1).then_some(checkpoint)
}

/// Session-qualified filenames reject all prior controller receipts without deleting user data.
fn checkpoint_path(profile: &Path, session: &str) -> PathBuf {
    profile.join(format!("observations-{session}.json"))
}
