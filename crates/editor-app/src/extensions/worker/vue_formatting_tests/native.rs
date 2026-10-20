//! One bounded Windows child per scenario keeps real actor wakeups on the production OS scheduler.
//! Profiles, manager receipts and HWNDs remain private; no deterministic scheduler check is disabled.
use super::*;
use anyhow::{Context as _, ensure};
use gpui_kit::{AnyWindowHandle, AsyncApp, EntityInputHandler as _};
use raw_window_handle::RawWindowHandle;
use std::{cell::RefCell, fs, process::Command, rc::Rc};
use windows_sys::Win32::UI::WindowsAndMessaging::IsWindow;

mod actions;

const CHILD: &str = "extensions::worker::vue_formatting_tests::native::formatting_child";
const ROOT_ENV: &str = "NANOBUG_NATIVE_VUE_FORMATTING_ROOT";
const MODE_ENV: &str = "NANOBUG_NATIVE_VUE_FORMATTING_CASE";

/// Only the owning libtest child opens a window; the guardian covers cold setup, never an LSP budget.
pub(super) fn run(mode: &str) {
    let directory = tempfile::tempdir().unwrap();
    let log_path = directory.path().join("native-vue-formatting.log");
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
        .env(MODE_ENV, mode)
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
        "native Vue formatting {mode} failed/stalled: {status:?}\n{log}"
    );
    assert!(
        log.contains(&format!(
            "native Vue formatting {mode}: assertions and checked cleanup verified"
        )),
        "missing checked {mode} receipt:\n{log}"
    );
    eprintln!("{log}");
}

/// Private child: ordinary libtest enters WindowsPlatform instead of TestAppContext.
#[test]
#[ignore = "private child of the four bounded Vue formatting parents; do not invoke directly"]
fn formatting_child() {
    let root = PathBuf::from(std::env::var_os(ROOT_ENV).expect("parent-owned root"));
    let mode = std::env::var(MODE_ENV).expect("parent-owned scenario");
    assert!(matches!(
        mode.as_str(),
        "matrix" | "provider" | "boundaries" | "aba"
    ));
    let workspace = input("NANOBUG_VUE_FIXTURES").canonicalize().unwrap();
    let disks = ["vue26", "vue27", "vue3"]
        .into_iter()
        .map(|version| {
            let path = workspace
                .join(format!("tests/projects/{version}/App.vue"))
                .canonicalize()
                .unwrap();
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect();
    let profile = root.join("profile");
    let runtime = extensions::profile_runtime_root(&profile);
    let environment = Environment {
        workspace: workspace.display().to_string(),
        os: "windows".into(),
        ..Default::default()
    };
    {
        let mut manager = Manager::open(runtime.clone(), environment.clone()).unwrap();
        for name in ["NANOBUG_TYPESCRIPT_PACKAGE", "NANOBUG_VUE_PACKAGE"] {
            if mode == "provider" && name == "NANOBUG_VUE_PACKAGE" {
                continue;
            }
            let package = Package::read(&input(name)).unwrap();
            manager
                .install(&package, package.manifest.permissions.clone())
                .unwrap();
        }
        manager.shutdown_checked().unwrap();
    }
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
            crate::ui::controls::init(cx);
            crate::app::shortcuts::bootstrap::configure(cx, profile.join("shortcuts.json"))
                .unwrap();
            let actor = Arc::new(Worker::start_background(runtime, environment, true));
            let mut options = crate::ui::controls::window::TitleBar::window_options();
            options.show = true;
            options.focus = false;
            options.inactive_frame_interval = None;
            let (main, app) = gpui_kit::open_window(options, cx, move |window, cx| {
                window.set_window_title("Nanobug — Vue formatting acceptance");
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
                disks,
                external: RefCell::new(Vec::new()),
            };
            cx.spawn(async move |cx| {
                let observed = match mode.as_str() {
                    "matrix" => matrix::exercise(&harness, cx).await,
                    "provider" => matrix::provider(&harness, cx).await,
                    "boundaries" => boundaries::exercise(&harness, cx).await,
                    "aba" => selection::exercise(&harness, cx).await,
                    _ => unreachable!("validated parent scenario"),
                };
                eprintln!("native Vue formatting {mode} scenario: {observed:#?}");
                let cleanup = harness.shutdown(cx).await;
                eprintln!("native Vue formatting {mode} checked cleanup: {cleanup:#?}");
                let failures = [observed, cleanup]
                    .into_iter()
                    .filter_map(Result::err)
                    .map(|error| format!("{error:#}"))
                    .collect::<Vec<_>>();
                *completed.borrow_mut() = Some(if failures.is_empty() {
                    eprintln!(
                        "native Vue formatting {mode}: assertions and checked cleanup verified"
                    );
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
        .expect("native formatting child did not report an outcome")
        .unwrap();
}

/// Observations are immutable preimages only; native EditorState/DocumentSession owns text and Undo.
pub(super) struct Harness {
    pub(super) main: AnyWindowHandle,
    pub(super) app: Entity<EditorApp>,
    pub(super) actor: Arc<Worker>,
    pub(super) workspace: PathBuf,
    disks: Vec<(PathBuf, Vec<u8>)>,
    /// Additional disk fixtures survive through checked shutdown and are then removed by their owner.
    external: RefCell<Vec<tempfile::NamedTempFile>>,
}

impl Harness {
    /// Publish the actual actor, reconcile contributions and draw only this child's original window.
    pub(super) fn frame(&self, cx: &mut AsyncApp) -> anyhow::Result<()> {
        self.main.update(cx, |_, window, cx| {
            self.app
                .read(cx)
                .extensions
                .clone()
                .update(cx, |panel, cx| panel.poll(cx));
            self.app
                .update(cx, |app, cx| app.sync_runtime_contributions(window, cx));
            window.draw(cx).clear(cx);
        })?;
        Ok(())
    }

    /// Retain each original observer budget while production timers advance on real OS threads.
    pub(super) async fn wait(
        &self,
        phase: &str,
        seconds: u64,
        mut ready: impl FnMut(&EditorApp, &App) -> anyhow::Result<bool>,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        loop {
            self.frame(cx)?;
            if self
                .main
                .update(cx, |_, _, cx| ready(self.app.read(cx), cx))??
            {
                return Ok(());
            }
            ensure!(
                Instant::now() < deadline,
                "native Vue formatting timed out: {phase}; {}",
                self.status(cx)?
            );
            cx.background_executor()
                .timer(Duration::from_millis(10))
                .await;
        }
    }

    /// Context, package and UI phase errors remain visible without printing user source.
    fn status(&self, cx: &mut AsyncApp) -> anyhow::Result<String> {
        Ok(self.main.update(cx, |_, _, cx| {
            let state = self.actor.state.lock().unwrap();
            format!(
                "status={} bindings={} services={} entries={:?}",
                self.app.read(cx).status,
                state.language_document_bindings.len(),
                state.language_services.len(),
                state
                    .entries
                    .iter()
                    .map(|entry| (&entry.manifest.id, &entry.error))
                    .collect::<Vec<_>>()
            )
        })?)
    }

    /// Shared input files are checked after every phase and again after worker/window teardown.
    pub(super) fn disks_unchanged(&self) -> anyhow::Result<()> {
        for (path, bytes) in &self.disks {
            ensure!(
                fs::read(path)? == *bytes,
                "native formatting changed {} on disk",
                path.display()
            );
        }
        Ok(())
    }

    /// Keep the ordinary source identity and IME-aware revision available to a same-turn assertion.
    pub(super) fn version(&self, cx: &mut AsyncApp) -> anyhow::Result<api::DocumentVersion> {
        self.main.update(cx, |_, _, cx| {
            let app = self.app.read(cx);
            app.plugin_document_info(
                app.active_text_tab_index()
                    .context("active native source")?,
                cx,
            )
            .map(|info| info.document)
            .map_err(anyhow::Error::msg)
        })?
    }

    /// Real frames release native input before the sole owned window is physically removed.
    async fn next_frame(&self, cx: &mut AsyncApp) -> anyhow::Result<()> {
        let (send, receive) = futures::channel::oneshot::channel();
        self.main.update(cx, |_, window, _| {
            window.on_next_frame(move |_, _| {
                let _ = send.send(());
            });
            window.refresh();
        })?;
        let timer = cx.background_executor().timer(Duration::from_secs(5));
        match futures::future::select(receive, Box::pin(timer)).await {
            futures::future::Either::Left((result, _)) => {
                result.context("native formatting frame dropped")
            }
            futures::future::Either::Right(_) => anyhow::bail!("native formatting frame timed out"),
        }
    }

    /// Successful cleanup means the production actor acknowledged retirement, processes are zero and HWND is gone.
    async fn shutdown(&self, cx: &mut AsyncApp) -> anyhow::Result<()> {
        let plans = self
            .actor
            .state
            .lock()
            .unwrap()
            .language_services
            .values()
            .filter_map(|plan| plan.as_ref().ok().cloned())
            .collect::<Vec<_>>();
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
            "formatting actor did not acknowledge shutdown"
        );
        ensure!(
            plans
                .iter()
                .all(|plan| !plan.is_active() && plan.process_count() == 0),
            "formatting service survived checked shutdown"
        );
        for _ in 0..2 {
            self.main.update(cx, |_, window, cx| {
                window.blur(cx);
                window.draw(cx).clear(cx);
            })?;
            self.next_frame(cx).await?;
        }
        let hwnd = self.main.update(cx, |_, window, _| {
            let RawWindowHandle::Win32(handle) =
                raw_window_handle::HasWindowHandle::window_handle(window)
                    .unwrap()
                    .as_raw()
            else {
                unreachable!("Windows-only formatter acceptance")
            };
            let hwnd = handle.hwnd.get();
            window.remove_window();
            hwnd
        })?;
        let deadline = Instant::now() + Duration::from_secs(5);
        // SAFETY: the opaque handle came from this child's original window and is not dereferenced.
        while unsafe { IsWindow(hwnd as _) != 0 } {
            ensure!(
                Instant::now() < deadline,
                "owned formatting HWND survived cleanup"
            );
            cx.background_executor()
                .timer(Duration::from_millis(10))
                .await;
        }
        self.disks_unchanged()
    }
}

/// Package errors are fatal observations, never replaced by a longer result wait.
pub(super) fn preparation(app: &EditorApp, actor: &Worker) -> anyhow::Result<()> {
    for (key, state) in &app.language_service_states {
        if let crate::app::language_servers::ServiceLoadState::Failed(error) = state {
            anyhow::bail!("Vue formatting preparation {key}: {error}");
        }
    }
    for entry in &actor.state.lock().unwrap().entries {
        ensure!(
            entry.error.is_none(),
            "format Package {}: {:?}",
            entry.manifest.id,
            entry.error
        );
    }
    Ok(())
}
