//! Host protocols borrow approved native services; retiring the package revokes every borrowed process.
use crate::process::{Spawned, spawn_piped};
mod completion;
mod consumers;
mod contexts;
mod files;
mod modules;
mod sources;
pub(crate) use completion::CompletionHook;
use plugin_protocol::language::Provider;
pub use sources::LanguageSource;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    process::{ChildStdin, ChildStdout},
    sync::{Arc, Condvar, Mutex, Weak},
    time::Duration,
};

/// Only the public Manager supplies negotiated contracts, original grants and immutable scope.
/// Optional WASM authority extends this native owner, never requires an empty guest component.
pub(crate) struct LanguagePolicy {
    pub(crate) capabilities: BTreeMap<String, semver::Version>,
    pub(crate) permissions: BTreeSet<String>,
    pub(crate) workspace: PathBuf,
    pub(crate) data: PathBuf,
    pub(crate) application: bool,
    pub(crate) inherited: Option<crate::EditorAuthority>,
}

/// Validated immutable plan. Consumers cannot mutate the command behind an approved lease.
pub struct LanguageService {
    pub owner: String,
    pub provider: Provider,
    pub root: PathBuf,
    /// The package's negotiated versions remain immutable for this service incarnation.
    policy: LanguagePolicy,
    /// Native feature actions must return to the sealed original instance, never a matching replacement ID.
    authority: crate::EditorAuthority,
    /// Resource-only packages have no WASM instance. Their immutable native plan issues one sealed
    /// lifetime that is revoked by the same preparation-cache retirement as its controlled processes.
    native_lifetime: Arc<std::sync::atomic::AtomicBool>,
    /// The common request/batch arena stores handles and receipts, never native text or Undo entries.
    native_requests: Mutex<crate::instance::NativeDocumentRequests>,
    pub(crate) program: PathBuf,
    pub(crate) args: Vec<String>,
    state: Mutex<Lease>,
    /// The host receives the same process-local sink as the package's WASM and native diagnostics.
    runtime_logs: crate::RuntimeLogs,
    /// Active plans pin files even if another workspace removes the package's installation record.
    pub(crate) dependencies: Vec<Arc<std::fs::File>>,
    /// Optional stateless worker uses the same immutable provider lease as the native service.
    pub(crate) completion: Option<CompletionHook>,
    /// Executable adapter bytes stay pinned through process termination, even after a source revokes.
    modules: Mutex<Vec<modules::AttachedModule>>,
    /// Manager-signed current document intents authorize native control without lending another project.
    document_contexts:
        Mutex<std::collections::BTreeSet<plugin_protocol::language::DocumentContext>>,
    /// Original consumers retain native context references independently of shared adapter code.
    consumers: Mutex<consumers::Consumers>,
    /// Opaque business context provenance never owns source text or an additional native process.
    registered_context: Mutex<
        Option<(
            crate::LanguageContexts,
            crate::language_contexts::RegisteredContext,
        )>,
    >,
}
#[derive(Default)]
struct Lease {
    retired: bool,
    children: Vec<LeasedChild>,
}

/// Each launch has its own reader cutoff; stopping one process cannot silence a later replacement.
struct LeasedChild {
    child: Weak<Mutex<Spawned>>,
    stderr: Arc<StderrPublication>,
}

/// A stopped provider cannot revive alerts, while a failed one can finish its final physical line.
#[derive(Default)]
struct StderrPublication {
    state: Mutex<StderrState>,
    drained: Condvar,
}

#[derive(Default)]
struct StderrState {
    accepting: bool,
    finished: bool,
}

impl StderrPublication {
    /// Hold the authority lock through append so a completed revocation cannot be followed by a record.
    fn publish(
        &self,
        logs: &crate::RuntimeLogs,
        owner: &str,
        level: crate::LogLevel,
        source: &str,
        message: String,
    ) {
        let state = self.state.lock().unwrap();
        if state.accepting {
            logs.append(owner, level, source, message);
        }
    }

    /// Explicit revocation takes precedence over any concurrent failure-drain wait.
    fn revoke(&self) {
        self.state.lock().unwrap().accepting = false;
        self.drained.notify_all();
    }

    /// Called after EOF has published an unterminated tail, not merely after the child exits.
    fn finish(&self) {
        let mut state = self.state.lock().unwrap();
        state.finished = true;
        state.accepting = false;
        self.drained.notify_all();
    }

    /// Wait only for this launch's reader; an inherited pipe in another process cannot stall teardown.
    fn finish_failure(&self, timeout: Duration) {
        let state = self.state.lock().unwrap();
        let (mut state, _) = self
            .drained
            .wait_timeout_while(state, timeout, |state| state.accepting && !state.finished)
            .unwrap();
        state.accepting = false;
    }
}

/// A transport may outlive a UI task, but it cannot outlive its package authority.
pub struct ServiceProcess {
    child: Arc<Mutex<Spawned>>,
    stderr: Arc<StderrPublication>,
}
impl ServiceProcess {
    /// Revoke output before stopping a deliberately cancelled or retired provider.
    /// Later buffered bytes are drained without producing records or new anomaly reminders.
    pub fn stop(&self) {
        // Serialize stopping against stderr publication before terminating the underlying process.
        self.stderr.revoke();
        self.terminate();
    }

    /// Preserve an actual failure's last stderr line before revoking its output authority.
    /// The reader has at most 200 ms after child reaping; an earlier user/package stop still wins.
    pub fn stop_after_failure(&self) {
        self.terminate();
        self.stderr.finish_failure(Duration::from_millis(200));
    }

    /// Both stop reasons terminate the same approved job and reap it before releasing the lease.
    fn terminate(&self) {
        let mut child = self.child.lock().unwrap();
        #[cfg(windows)]
        child.job.terminate();
        let _ = child.child.kill();
        let _ = child.child.wait();
    }
}
impl Drop for ServiceProcess {
    fn drop(&mut self) {
        self.stop();
    }
}

impl LanguageService {
    pub(crate) fn new(
        owner: String,
        provider: Provider,
        root: PathBuf,
        program: PathBuf,
        args: Vec<String>,
        runtime_logs: crate::RuntimeLogs,
        policy: LanguagePolicy,
    ) -> Self {
        let native_lifetime = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let authority = policy.inherited.as_ref().map_or_else(
            || crate::EditorAuthority::new(native_lifetime.clone(), None),
            |original| original.with_native_owner(native_lifetime.clone()),
        );
        let native_requests = Mutex::new(crate::instance::NativeDocumentRequests::new(
            &policy.workspace,
            policy.data.clone(),
            policy.application,
            authority.clone(),
        ));
        Self {
            owner,
            provider,
            root,
            policy,
            authority,
            native_lifetime,
            native_requests,
            program,
            args,
            state: Mutex::new(Lease::default()),
            runtime_logs,
            dependencies: vec![],
            completion: None,
            modules: Default::default(),
            document_contexts: Default::default(),
            consumers: Default::default(),
            registered_context: Default::default(),
        }
    }
    /// Return a negotiated public capability version for this plan, or `None` when unavailable.
    /// This read-only lookup grants neither execution nor additional file access; consumers still
    /// validate their provider declaration, current lease and operation-specific permissions.
    pub fn capability(&self, name: &str) -> Option<&semver::Version> {
        self.policy.capabilities.get(name)
    }
    /// Borrow original instance authority without extending its permissions or lifetime.
    pub fn authority(&self) -> &crate::EditorAuthority {
        &self.authority
    }
    /// A reference path is data; opening it additionally requires this original read grant.
    pub fn can_read_workspace(&self) -> bool {
        self.policy.permissions.contains("workspace.read")
    }
    /// Admit provider content using negotiated contracts and the original live service authority.
    /// The host alone supplies source metadata; untrusted URIs never become filesystem paths.
    /// Invalid schemes, excessive content and revoked owners fail before allocating resources.
    pub fn open_content(
        &self,
        source: plugin_protocol::api::DocumentVersion,
        location: plugin_protocol::language_content::Location,
        title: String,
        text: String,
    ) -> Result<(), plugin_protocol::api::Failure> {
        use plugin_protocol::api::{ErrorCode, Failure};
        let state = self.state.lock().unwrap();
        if state.retired || !self.authority.is_live() {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Language content owner retired",
            ));
        }
        let content = self
            .provider
            .content
            .as_ref()
            .filter(|content| content.accepts(&location.uri))
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::UnsupportedOperation,
                    "Undeclared language content scheme",
                )
            })?;
        if !self.policy.permissions.contains("editor.read")
            || !self.policy.capabilities.contains_key("language.content")
            || !self.policy.capabilities.contains_key("editor.virtual")
        {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Readonly language content was not granted",
            ));
        }
        self.native_requests.lock().unwrap().open_content(
            source,
            location,
            title,
            content.language.clone(),
            text,
            self.analysis_conditions(),
        )
    }
    /// The current Manager-published incarnation alone enters this original policy. The lease lock
    /// serializes issuance with retirement; preview, revision checks and Undo remain native host work.
    pub(crate) fn submit_language_fix(
        &self,
        document: plugin_protocol::api::DocumentVersion,
        fix: plugin_protocol::diagnostics::Fix,
        mut guards: Vec<Arc<std::sync::atomic::AtomicBool>>,
    ) -> Result<plugin_protocol::api::Value, plugin_protocol::api::Failure> {
        let state = self.state.lock().unwrap();
        if state.retired || !self.authority.is_live() {
            return Err(plugin_protocol::api::Failure::new(
                plugin_protocol::api::ErrorCode::InvalidHandle,
                "Native language service retired",
            ));
        }
        let plan = crate::workspace_resources::language_plan(document, fix.plan)?;
        // Registered configuration has the same pre-effect phase as native source/connection seals;
        // neither can permanently narrow recovery of an already committed native document edit.
        guards.extend(self.analysis_conditions());
        self.native_requests.lock().unwrap().submit(
            plan,
            &self.policy.capabilities,
            &self.policy.permissions,
            guards,
        )
    }
    /// Transfer calls through the public Manager and its existing serialized native dispatcher.
    pub(crate) fn take_editor_requests(&self) -> Vec<crate::EditorRequest> {
        self.native_requests.lock().unwrap().take()
    }
    /// Reclaim transient terminal slots without discarding retained native recovery receipts.
    pub(crate) fn poll_requests(&self) {
        self.native_requests.lock().unwrap().poll();
    }
    /// Lifecycle diagnostics count this real owner alongside its controlled process.
    pub(crate) fn request_resource_count(&self) -> usize {
        self.native_requests.lock().unwrap().resource_count()
    }
    /// Borrow the shared run history without transferring authority to launch or modify this plan.
    pub fn runtime_logs(&self) -> crate::RuntimeLogs {
        self.runtime_logs.clone()
    }
    /// Publish a host-owned transport message only while both package and view authority are live.
    /// The lease lock serializes this append with retirement; rejected late callbacks return `None`.
    pub fn append_runtime_log(
        &self,
        retired: &std::sync::atomic::AtomicBool,
        level: crate::LogLevel,
        source: &str,
        message: impl Into<String>,
    ) -> Option<u64> {
        let state = self.state.lock().unwrap();
        if state.retired || retired.load(std::sync::atomic::Ordering::Acquire) {
            return None;
        }
        Some(
            self.runtime_logs
                .append(&self.owner, level, source, message),
        )
    }
    /// This lock serializes launch against revocation; no child can escape the retirement boundary.
    pub fn spawn(&self) -> anyhow::Result<(ServiceProcess, ChildStdin, ChildStdout)> {
        self.spawn_for_owner(&std::sync::atomic::AtomicBool::new(false))
    }
    /// A view's retirement flag shares this launch lock with stop_processes, preventing late starts.
    pub fn spawn_for_owner(
        &self,
        retired: &std::sync::atomic::AtomicBool,
    ) -> anyhow::Result<(ServiceProcess, ChildStdin, ChildStdout)> {
        let mut state = self.state.lock().unwrap();
        anyhow::ensure!(
            !state.retired
                && !retired.load(std::sync::atomic::Ordering::Acquire)
                && self.authority.is_live()
                && Self::modules_live(&self.modules.lock().unwrap()),
            "LSP provider has been retired"
        );
        state
            .children
            .retain(|lease| lease.child.strong_count() > 0);
        anyhow::ensure!(
            state.children.is_empty(),
            "LSP provider already has a live process"
        );
        let mut child = spawn_piped(&self.program, &self.args, &self.root, &Default::default())?;
        let input = child.child.stdin.take().expect("piped stdin");
        let output = child.child.stdout.take().expect("piped stdout");
        let mut errors = child.child.stderr.take().expect("piped stderr");
        // stderr has no JSON-RPC frames: capture bounded lines without ever treating stdout as log data.
        let logs = self.runtime_logs.clone();
        let owner = self.owner.clone();
        let source = format!("lsp/{}/stderr", self.provider.id);
        let stderr = Arc::new(StderrPublication {
            state: Mutex::new(StderrState {
                accepting: true,
                finished: false,
            }),
            drained: Condvar::new(),
        });
        let reader_stderr = stderr.clone();
        std::thread::spawn(move || {
            crate::logs::drain(
                &mut errors,
                |level, source, message| {
                    // Continue draining after stop, but old buffered bytes cannot revive an anomaly reminder.
                    reader_stderr.publish(&logs, &owner, level, source, message);
                },
                crate::LogLevel::Warning,
                &source,
            );
            // stdout EOF can reach the transport first; completion includes stderr's own final flush.
            reader_stderr.finish();
        });
        let child = Arc::new(Mutex::new(child));
        state.children.push(LeasedChild {
            child: Arc::downgrade(&child),
            stderr: stderr.clone(),
        });
        Ok((ServiceProcess { child, stderr }, input, output))
    }
    /// Consent and lifecycle diagnostics include host-owned transports as well as guest-created processes.
    pub fn process_count(&self) -> usize {
        self.state
            .lock()
            .unwrap()
            .children
            .iter()
            .filter(|lease| lease.child.strong_count() > 0)
            .count()
    }
    /// Actual plan equality permits same-owner hot revalidation to retain the existing transport.
    /// A matching package name never substitutes for inherited exact authority or active native seals.
    pub(crate) fn same_plan(&self, other: &Self) -> bool {
        let same_inherited = match (&self.policy.inherited, &other.policy.inherited) {
            (Some(original), Some(candidate)) => original.same_owner(candidate),
            (None, None) => true,
            _ => false,
        };
        self.is_active()
            && other.is_active()
            && same_inherited
            && self.owner == other.owner
            && self.provider == other.provider
            && self.root == other.root
            && self.program == other.program
            && self.args == other.args
            && self.policy.capabilities == other.policy.capabilities
            && self.policy.permissions == other.policy.permissions
            && self.policy.workspace == other.policy.workspace
            && self.policy.data == other.policy.data
            && self.policy.application == other.policy.application
            && self.completion.as_ref().map(|hook| &hook.settings)
                == other.completion.as_ref().map(|hook| &hook.settings)
    }
    pub(crate) fn retire(&self) {
        let mut state = self.state.lock().unwrap();
        state.retired = true;
        self.native_lifetime
            .store(false, std::sync::atomic::Ordering::Release);
        for lease in state.children.drain(..) {
            if let Some(child) = lease.child.upgrade() {
                ServiceProcess {
                    child,
                    stderr: lease.stderr,
                }
                .stop();
            }
        }
        // Mark retired before waiting for a bounded pure call; its eventual result is rejected.
        drop(state);
        self.native_requests.lock().unwrap().retire();
        if let Some(hook) = &self.completion
            && let Some(mut instance) = hook.instance.lock().unwrap().take()
        {
            instance.stop();
        }
    }
    /// Selection changes stop the current transport while keeping the installed startup plan reusable.
    pub fn stop_processes(&self) {
        let mut state = self.state.lock().unwrap();
        for lease in state.children.drain(..) {
            if let Some(child) = lease.child.upgrade() {
                ServiceProcess {
                    child,
                    stderr: lease.stderr,
                }
                .stop();
            }
        }
    }
}

impl Drop for LanguageService {
    /// Last-owner drop follows the same unconditional cleanup as replacement and disable.
    fn drop(&mut self) {
        self.retire();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LogLevel, RuntimeLogs};

    /// A fault teardown waits for the reader's EOF flush, then rejects further buffered callbacks.
    #[test]
    fn failure_wait_preserves_stderr_tail_then_revokes_publication() {
        let logs = RuntimeLogs::default();
        let publication = Arc::new(StderrPublication {
            state: Mutex::new(StderrState {
                accepting: true,
                finished: false,
            }),
            drained: Condvar::new(),
        });
        let waiting = publication.clone();
        let (started, begin) = std::sync::mpsc::channel();
        let (finished, done) = std::sync::mpsc::channel();
        let stop = std::thread::spawn(move || {
            started.send(()).unwrap();
            waiting.finish_failure(Duration::from_secs(2));
            finished.send(()).unwrap();
        });
        begin.recv().unwrap();
        // The unterminated line is published before the reader reports completion.
        publication.publish(
            &logs,
            "owner",
            LogLevel::Warning,
            "lsp/analysis/stderr",
            "crash tail".into(),
        );
        publication.finish();
        done.recv_timeout(Duration::from_secs(1)).unwrap();
        stop.join().unwrap();
        publication.publish(
            &logs,
            "owner",
            LogLevel::Warning,
            "lsp/analysis/stderr",
            "late tail".into(),
        );
        assert_eq!(logs.records("owner").len(), 1);
        assert_eq!(logs.records("owner")[0].message, "crash tail");
    }

    /// A user/package revocation wakes failure teardown and wins over a pending final line.
    #[test]
    fn controlled_revocation_wakes_failure_wait_without_reviving_a_reminder() {
        let logs = RuntimeLogs::default();
        let publication = Arc::new(StderrPublication {
            state: Mutex::new(StderrState {
                accepting: true,
                finished: false,
            }),
            drained: Condvar::new(),
        });
        let waiting = publication.clone();
        let (started, begin) = std::sync::mpsc::channel();
        let (finished, done) = std::sync::mpsc::channel();
        let stop = std::thread::spawn(move || {
            started.send(()).unwrap();
            waiting.finish_failure(Duration::from_secs(10));
            finished.send(()).unwrap();
        });
        begin.recv().unwrap();
        publication.revoke();
        done.recv_timeout(Duration::from_secs(1)).unwrap();
        stop.join().unwrap();
        publication.publish(
            &logs,
            "owner",
            LogLevel::Warning,
            "lsp/analysis/stderr",
            "old buffered tail".into(),
        );
        publication.finish();
        assert!(logs.records("owner").is_empty());
        assert!(logs.pending_reminders().is_empty());
    }

    /// Descendants retaining a pipe cannot keep a failed reader authorized after its finite drain budget.
    #[test]
    fn failure_timeout_revokes_an_unfinished_reader() {
        let publication = StderrPublication {
            state: Mutex::new(StderrState {
                accepting: true,
                finished: false,
            }),
            drained: Condvar::new(),
        };
        publication.finish_failure(Duration::ZERO);
        let logs = RuntimeLogs::default();
        publication.publish(
            &logs,
            "owner",
            LogLevel::Warning,
            "lsp/analysis/stderr",
            "outlived failure drain".into(),
        );
        assert!(logs.records("owner").is_empty());
    }

    /// Both host-view retirement and package revocation reject late transport messages in the owner sink.
    #[test]
    fn runtime_log_publication_respects_view_and_package_retirement() {
        let logs = RuntimeLogs::default();
        let provider = serde_json::from_value(serde_json::json!({
            "id":"analysis", "language":"fixture-language", "service":"analysis"
        }))
        .unwrap();
        let plan = LanguageService::new(
            "owner".into(),
            provider,
            PathBuf::new(),
            PathBuf::new(),
            vec![],
            logs.clone(),
            LanguagePolicy {
                capabilities: BTreeMap::new(),
                permissions: BTreeSet::new(),
                workspace: PathBuf::new(),
                data: PathBuf::new(),
                application: false,
                inherited: Some(crate::EditorAuthority::new(
                    Arc::new(std::sync::atomic::AtomicBool::new(true)),
                    None,
                )),
            },
        );
        let retired = std::sync::atomic::AtomicBool::new(false);
        let id = plan
            .append_runtime_log(&retired, LogLevel::Info, "lsp/analysis", "ready")
            .unwrap();
        assert_eq!(plan.runtime_logs().records("owner")[0].id, id);
        retired.store(true, std::sync::atomic::Ordering::Release);
        assert!(
            plan.append_runtime_log(
                &retired,
                LogLevel::Error,
                "lsp/analysis",
                "late host result"
            )
            .is_none()
        );
        retired.store(false, std::sync::atomic::Ordering::Release);
        plan.retire();
        assert!(
            plan.append_runtime_log(
                &retired,
                LogLevel::Error,
                "lsp/analysis",
                "late package result"
            )
            .is_none()
        );
        assert_eq!(logs.records("owner").len(), 1);
        assert!(logs.pending_reminders().is_empty());
    }
}
