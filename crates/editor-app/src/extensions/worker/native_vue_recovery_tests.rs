//! Independent packages recover on the real Windows scheduler, with source semantics as evidence.
//! Every profile, process and HWND belongs to this bounded child; project files remain untouched.
use super::*;
use crate::*;
use anyhow::{Context as _, ensure};
use gpui_kit::{AnyWindowHandle, AsyncApp};
use raw_window_handle::RawWindowHandle;
use std::{cell::RefCell, fs, process::Command, rc::Rc};
use windows_sys::Win32::UI::WindowsAndMessaging::IsWindow;

mod scenario;

const CHILD: &str = "extensions::worker::native_vue_recovery_tests::native_vue_recovery_child";
const ROOT_ENV: &str = "NANOBUG_NATIVE_VUE_RECOVERY_ROOT";
/// Private child-only fault selection; it never becomes a language service or tool option.
const FAILURE_ENV: &str = "NANOBUG_NATIVE_VUE_RECOVERY_FAILURE";
const RECEIPT: &str =
    "native Vue recovery: semantics, isolation, stale result and cleanup verified";

/// Cold compilation is bounded by the parent, while ordinary LSP requests retain production budgets.
#[test]
#[ignore = "requires Windows/D3D11 and real NANOBUG_TYPESCRIPT_PACKAGE, NANOBUG_VUE_PACKAGE, NANOBUG_VUE_FIXTURES"]
fn real_vue_backends_recover_without_reviving_old_results() {
    recovery_case("outer");
}

/// The real admitted adapter is a grandchild; killing only it must reach bounded native recovery.
#[test]
#[ignore = "requires Windows/D3D11 and real NANOBUG_TYPESCRIPT_PACKAGE, NANOBUG_VUE_PACKAGE, NANOBUG_VUE_FIXTURES"]
fn real_vue_adapter_children_recover_without_reviving_old_results() {
    recovery_case("inner-adapter");
}

/// Offered-contract updates have their own bounded scenario; the combined outer case retains
/// the same check after both consumer-candidate rejections for final joint coverage.
#[test]
#[ignore = "requires Windows/D3D11 and real NANOBUG_TYPESCRIPT_PACKAGE, NANOBUG_VUE_PACKAGE, NANOBUG_VUE_FIXTURES"]
fn real_incompatible_typescript_provider_preserves_working_vue_backends() {
    recovery_case("provider-update");
}

/// Give both actual failure modes the same OS scheduling, ownership and checked cleanup boundary.
fn recovery_case(failure: &str) {
    let directory = tempfile::tempdir().unwrap();
    let log_path = directory.path().join("native-vue-recovery.log");
    // The bounded parent's own path makes a running child diagnosable without searching profiles.
    eprintln!("native Vue recovery owned log: {}", log_path.display());
    let log = fs::File::create(&log_path).unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            CHILD,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(ROOT_ENV, directory.path())
        .env(FAILURE_ENV, failure)
        .env("ME_EDITOR_PROFILE_HOME", directory.path().join("profile"))
        .env("RUST_BACKTRACE", "1")
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(360);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let log = fs::read_to_string(log_path).unwrap();
    assert!(
        status.is_some_and(|status| status.success()),
        "native Vue recovery child failed or stalled: {status:?}\n{log}"
    );
    assert!(log.contains(RECEIPT), "missing checked receipt:\n{log}");
    eprintln!("{log}");
}

/// Real OS scheduling prevents synchronous pipe IO from blocking the deterministic GPUI test loop.
#[test]
#[ignore = "private child of the bounded real Vue recovery test"]
fn native_vue_recovery_child() {
    let root = PathBuf::from(std::env::var_os(ROOT_ENV).expect("parent-owned root"));
    let workspace = input("NANOBUG_VUE_FIXTURES").canonicalize().unwrap();
    let profile = root.join("profile");
    let runtime = extensions::profile_runtime_root(&profile);
    let environment = extensions::protocol::Environment {
        workspace: workspace.display().to_string(),
        os: "windows".into(),
        ..Default::default()
    };
    {
        let mut manager = Manager::open(runtime.clone(), environment.clone()).unwrap();
        for name in ["NANOBUG_TYPESCRIPT_PACKAGE", "NANOBUG_VUE_PACKAGE"] {
            let package = Package::read(&input(name)).unwrap();
            manager
                .install(&package, package.manifest.permissions.clone())
                .unwrap();
        }
    }
    let paths = ["vue26", "vue27", "vue3"].map(|name| {
        workspace
            .join(format!("tests/projects/{name}/App.vue"))
            .canonicalize()
            .unwrap()
    });
    let disks = paths
        .each_ref()
        .map(|path| fs::read_to_string(path).unwrap());
    let mut session = SessionState::load_in_profile(&workspace, &profile);
    session.workspace_trusted = true;
    let project = Workspace::open(&workspace).unwrap();
    let outcome = Rc::new(RefCell::new(None::<Result<(), String>>));
    let completed = outcome.clone();
    gpui_kit::application()
        .with_assets(AppAssets)
        .with_quit_mode(gpui_kit::QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            typography::init(cx);
            apply_theme(builtin_theme(false), cx);
            cx.set_reduce_motion(true);
            extensions::init(cx);
            editor_ui::init(cx);
            crate::app::shortcuts::bootstrap::configure(cx, profile.join("shortcuts.json"))
                .unwrap();
            let actor = Arc::new(Worker::start_background(runtime, environment, true));
            let mut options = editor_ui::window::TitleBar::window_options();
            options.show = true;
            options.focus = false;
            options.inactive_frame_interval = None;
            let (main, app) = gpui_kit::open_window(options, cx, move |window, cx| {
                window.set_window_title("Nanobug — Vue recovery regression");
                cx.new(|cx| EditorApp::new_with_session(project, None, Some(session), window, cx))
            })
            .unwrap();
            app.read(cx).extensions.clone().update(cx, |panel, cx| {
                panel.worker = actor.clone();
                panel.poll(cx);
            });
            let harness = Harness {
                main: main.into(),
                app,
                actor,
                workspace,
                paths,
                disks,
            };
            cx.spawn(async move |cx| {
                let mut scenario = Box::pin(scenario::exercise(&harness, cx));
                let observed = scenario.as_mut().await;
                drop(scenario);
                eprintln!("native Vue recovery scenario: {observed:#?}");
                let cleanup = harness.shutdown(cx).await;
                eprintln!("native Vue recovery cleanup: {cleanup:#?}");
                let failures = [observed, cleanup]
                    .into_iter()
                    .filter_map(Result::err)
                    .map(|error| format!("{error:#}"))
                    .collect::<Vec<_>>();
                *completed.borrow_mut() = Some(if failures.is_empty() {
                    eprintln!("{RECEIPT}");
                    Ok(())
                } else {
                    Err(failures.join("\n"))
                });
                drop(harness);
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
    outcome
        .borrow_mut()
        .take()
        .expect("native Vue child did not report an outcome")
        .unwrap();
}

/// The production actor publishes native document requests; no recorded mailbox or guest state is injected.
struct Harness {
    main: AnyWindowHandle,
    app: Entity<EditorApp>,
    actor: Arc<Worker>,
    workspace: PathBuf,
    paths: [PathBuf; 3],
    disks: [String; 3],
}

impl Harness {
    /// Poll actual actor publications and draw this child's original window while OS timers advance.
    fn frame(&self, cx: &mut AsyncApp) -> anyhow::Result<()> {
        self.main.update(cx, |_, window, cx| {
            self.app
                .read(cx)
                .extensions
                .clone()
                .update(cx, |panel, cx| panel.poll(cx));
            // The real render consumes provider generations and pending contribution flags.
            // Forcing synchronization on every test tick restarts still-loading grammar tasks.
            window.refresh();
            window.draw(cx).clear(cx);
        })?;
        Ok(())
    }

    /// Readiness observations never forge responses or alter a per-request timeout.
    async fn wait(
        &self,
        label: &str,
        seconds: u64,
        mut ready: impl FnMut(&EditorApp, &App) -> bool,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        let mut previous_states = String::new();
        let mut next_observation = Instant::now();
        // Each phase includes bounded retained history once, then only newer receipt identities.
        // Reading these records neither acknowledges logs nor advances any runtime request.
        let mut log_watermark = 0;
        loop {
            self.frame(cx)?;
            let states = self.main.update(cx, |_, _, cx| {
                format!("{:?}", self.app.read(cx).language_service_states)
            })?;
            if states != previous_states {
                // A bounded owned log exposes the original preparation failure while a child
                // still runs; inspecting a live user profile or changing the deadline is unnecessary.
                eprintln!(
                    "native Vue phase {label}: {}",
                    states.chars().take(4096).collect::<String>()
                );
                previous_states = states;
            }
            if Instant::now() >= next_observation {
                // Empty ingress queues do not imply idle work: native controls detach preparation.
                // Existing compilation receipts and exact plan metadata distinguish that wait
                // without a second dispatcher or a longer guest/language request deadline.
                eprintln!(
                    "native Vue request stage {label}: {}",
                    self.request_stage(cx, &mut log_watermark)?
                );
                next_observation = Instant::now() + Duration::from_secs(20);
            }
            if self
                .main
                .update(cx, |_, _, cx| ready(self.app.read(cx), cx))?
            {
                return Ok(());
            }
            if Instant::now() >= deadline {
                let states = self.main.update(cx, |_, _, cx| {
                    format!("{:?}", self.app.read(cx).language_service_states)
                })?;
                anyhow::bail!(
                    "native Vue phase timed out: {label}; {}; source states={states}; {}",
                    self.status(),
                    self.request_stage(cx, &mut log_watermark)?
                );
            }
            cx.background_executor()
                .timer(Duration::from_millis(10))
                .await;
        }
    }

    /// Keep synchronous native transport operations on the actual platform executor, with visible failures.
    async fn job<T: Send + Sync + 'static>(
        &self,
        label: &str,
        work: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<T> {
        let mut job = Box::pin(
            cx.background_executor()
                .scheduler_executor()
                .spawn_dedicated(move |_| async move { work() }),
        );
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if let std::task::Poll::Ready(result) = futures::poll!(&mut job) {
                return result;
            }
            self.frame(cx)?;
            ensure!(
                Instant::now() < deadline,
                "native Vue operation stalled: {label}; {}",
                self.status()
            );
            cx.background_executor()
                .timer(Duration::from_millis(10))
                .await;
        }
    }

    /// Report routing/setup failures without serializing source text or foreign process state.
    fn status(&self) -> String {
        let state = self.actor.state.lock().unwrap();
        format!(
            "ready={} entries={:?} bindings={} plans={} progress={:?} installation={:?} control={}",
            state.ready,
            state
                .entries
                .iter()
                .map(|entry| (&entry.manifest.id, &entry.error))
                .collect::<Vec<_>>(),
            state.language_document_bindings.len(),
            state.language_services.len(),
            state.progress,
            state.installation,
            state.install_control.is_some()
        )
    }

    /// Inspect existing actor/native metadata without retaining the publication lock across reads.
    /// Log bodies are omitted except strictly recognized host compilation timing receipts.
    fn request_stage(&self, cx: &mut AsyncApp, watermark: &mut u64) -> anyhow::Result<String> {
        use super::vue_typescript_tests::control_request_summary;
        const ROW_LIMIT: usize = 16;
        let label = |value: &str| value.chars().take(160).collect::<String>();
        let (worker, logs, owners, plans, epochs, bindings) = {
            let state = self.actor.state.lock().unwrap();
            (
                (
                    state.editor_requests.len(),
                    state
                        .editor_requests
                        .iter()
                        .take(ROW_LIMIT)
                        .map(|(plugin, request)| {
                            (label(plugin), label(&control_request_summary(request)))
                        })
                        .collect::<Vec<_>>(),
                ),
                state.logs.clone(),
                state
                    .entries
                    .iter()
                    .take(ROW_LIMIT)
                    .map(|entry| entry.manifest.id.clone())
                    .collect::<Vec<_>>(),
                (
                    state.language_services.len(),
                    state
                        .language_services
                        .iter()
                        .take(ROW_LIMIT)
                        .map(|(key, plan)| (label(key), plan.clone()))
                        .collect::<Vec<_>>(),
                ),
                (
                    state.instance_epochs.len(),
                    state
                        .instance_epochs
                        .iter()
                        .take(ROW_LIMIT)
                        .map(|(owner, epoch)| (label(owner), *epoch))
                        .collect::<Vec<_>>(),
                ),
                (
                    state.language_document_bindings.len(),
                    state
                        .language_document_bindings
                        .iter()
                        .take(ROW_LIMIT)
                        .map(|((provider, document), key)| {
                            (
                                label(provider),
                                label(&document.path),
                                document.generation,
                                label(key),
                            )
                        })
                        .collect::<Vec<_>>(),
                ),
            )
        };
        // Both log and native-plan reads have their own locks. Clone the publication first so an
        // observer cannot block actor publication while waiting on compilation/transport metadata.
        let plans = (
            plans.0,
            plans
                .1
                .into_iter()
                .map(|(key, plan)| {
                    (
                        key,
                        plan.ok().map(|plan| {
                            (
                                label(&plan.owner),
                                label(&plan.provider.id),
                                plan.is_active(),
                                plan.process_count(),
                            )
                        }),
                    )
                })
                .collect::<Vec<_>>(),
        );
        let log_stage = runtime_log_stage(&logs, &owners, watermark);
        self.main
            .update(cx, |_, _, cx| {
                let app = self.app.read(cx);
                let native = (
                    app.pending_editor_requests.len(),
                    app.pending_editor_requests
                        .iter()
                        .take(ROW_LIMIT)
                        .map(|(plugin, request)| {
                            (label(plugin), label(&control_request_summary(request)))
                        })
                        .collect::<Vec<_>>(),
                );
                format!(
                    "worker={worker:?}; native={native:?}; dispatch_scheduled={}; \
                    plans={plans:?}; instance_epochs={epochs:?}; bindings={bindings:?}; {log_stage}",
                    app.editor_request_dispatch_scheduled
                )
            })
            .map_err(Into::into)
    }

    /// Checked project shutdown retires the real actor before removing only the original owned HWND.
    async fn shutdown(&self, cx: &mut AsyncApp) -> anyhow::Result<()> {
        let task = self
            .main
            .update(cx, |_, _, cx| {
                self.app
                    .update(cx, |app, cx| app.prepare_project_shutdown(cx))
            })?
            .map_err(anyhow::Error::msg)?;
        task.await.map_err(anyhow::Error::msg)?;
        ensure!(
            self.actor
                .shutdown_complete
                .load(std::sync::atomic::Ordering::Acquire),
            "production actor did not acknowledge shutdown"
        );
        for _ in 0..2 {
            let (send, receive) = futures::channel::oneshot::channel();
            self.main.update(cx, |_, window, cx| {
                window.blur(cx);
                window.draw(cx).clear(cx);
                window.on_next_frame(move |_, _| {
                    let _ = send.send(());
                });
                window.refresh();
            })?;
            let timer = cx.background_executor().timer(Duration::from_secs(5));
            match futures::future::select(receive, Box::pin(timer)).await {
                futures::future::Either::Left((result, _)) => {
                    result.context("native blur frame dropped")?
                }
                futures::future::Either::Right(_) => anyhow::bail!("native blur frame timed out"),
            }
        }
        let hwnd = self.main.update(cx, |_, window, _| {
            let RawWindowHandle::Win32(handle) =
                raw_window_handle::HasWindowHandle::window_handle(window)
                    .unwrap()
                    .as_raw()
            else {
                unreachable!("Windows-only acceptance")
            };
            let hwnd = handle.hwnd.get();
            window.remove_window();
            hwnd
        })?;
        let deadline = Instant::now() + Duration::from_secs(5);
        // SAFETY: the opaque handle was borrowed from this child's original window, never dereferenced.
        while unsafe { IsWindow(hwnd as _) != 0 } {
            ensure!(
                Instant::now() < deadline,
                "owned Vue recovery HWND survived cleanup"
            );
            cx.background_executor()
                .timer(Duration::from_millis(10))
                .await;
        }
        for (path, disk) in self.paths.iter().zip(&self.disks) {
            ensure!(
                fs::read_to_string(path)? == *disk,
                "Vue recovery changed a project source file"
            );
        }
        Ok(())
    }
}

/// Read retained receipts only after releasing actor publication; omitted rows advance the watermark
/// explicitly, so noisy output cannot grow this owned diagnostic log without bound.
fn runtime_log_stage(
    logs: &plugin_runtime::RuntimeLogs,
    owners: &[String],
    watermark: &mut u64,
) -> String {
    let label = |value: &str| value.chars().take(160).collect::<String>();
    let mut records = owners
        .iter()
        .flat_map(|owner| logs.records(owner))
        .filter(|record| record.id > *watermark)
        .collect::<Vec<_>>();
    records.sort_by_key(|record| record.id);
    let previous_watermark = *watermark;
    if let Some(record) = records.last() {
        *watermark = record.id;
    }
    let omitted = records.len().saturating_sub(16);
    let records = records
        .into_iter()
        .skip(omitted)
        .map(|record| {
            let timing = (record.source == "wasm/compile")
                .then(|| safe_compile_timing(&record.message))
                .flatten();
            (
                record.id,
                label(&record.plugin),
                record.time,
                record.level,
                label(&record.source),
                timing,
            )
        })
        .collect::<Vec<_>>();
    format!(
        "log_ids={previous_watermark}..{watermark} omitted_logs={omitted} \
        logs(id,owner,time,level,source,compile_timing)={records:?}"
    )
}

/// Only exact host timing formats may enter the owned observer log. Compilation failures retain
/// their numeric duration while discarding arbitrary compiler diagnostics or embedded module data.
fn safe_compile_timing(message: &str) -> Option<String> {
    let numeric = |value: &str| {
        !value.is_empty() && value.len() <= 39 && value.bytes().all(|byte| byte.is_ascii_digit())
    };
    for mode in ["main", "pure-language"] {
        if message == format!("Reusing {mode} compiled component for an isolated Store") {
            return Some(message.into());
        }
        for (prefix, suffix) in [
            (format!("Begin {mode} component compilation ("), " bytes)"),
            (format!("Completed {mode} component compilation in "), " ms"),
        ] {
            if message
                .strip_prefix(&prefix)
                .and_then(|value| value.strip_suffix(suffix))
                .is_some_and(numeric)
            {
                return Some(message.into());
            }
        }
        let prefix = format!("{mode} component compilation failed after ");
        if let Some((elapsed, _)) = message
            .strip_prefix(&prefix)
            .and_then(|value| value.split_once(" ms: "))
            && numeric(elapsed)
        {
            return Some(format!("{prefix}{elapsed} ms"));
        }
    }
    None
}

/// Candidate inputs are explicit archives/fixtures supplied by the caller, never a source-folder fallback.
fn input(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).expect(name))
}
