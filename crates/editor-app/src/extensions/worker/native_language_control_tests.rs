//! Real Windows scheduling keeps bounded SDK controls independent from synchronous native pipe IO.
//! The parent owns every process and directory; the child uses the original editor and public SDK requests.
use super::*;
use crate::*;
use anyhow::{Context as _, ensure};
use gpui_kit::{AnyWindowHandle, AsyncApp};
use plugin_runtime::plugin_protocol::{self as protocol, api};
use raw_window_handle::RawWindowHandle;
use serde_json::{Value, json};
use std::{cell::RefCell, fs, process::Command, rc::Rc};
use windows_sys::Win32::UI::WindowsAndMessaging::IsWindow;

mod module_timeout;

/// Only this exact libtest child creates a private WindowsPlatform and native editor window.
const CHILD: &str =
    "extensions::worker::native_language_control_tests::native_language_control_child";
/// Parent-owned storage also isolates profile fallback paths used by the ordinary application.
const ROOT_ENV: &str = "NANOBUG_NATIVE_LANGUAGE_CONTROL_ROOT";
/// A success receipt follows semantic checks, checked runtime shutdown and native-window retirement.
const RECEIPT: &str =
    "native language control: timeout, reuse, source retirement and shutdown verified";

/// A separate process supplies the production OS scheduler without changing GPUI's deterministic scheduler.
#[test]
#[ignore = "requires Windows/D3D11, current SDK NANOBUG_CONTROL_FIXTURE_PACKAGE/NANOBUG_NATIVE_CONSUMERS_PACKAGE and lsp_fixture"]
fn native_controls_timeout_without_retiring_healthy_sources() {
    let directory = tempfile::tempdir().unwrap();
    let log_path = directory.path().join("native-language-control.log");
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
        .env("ME_EDITOR_PROFILE_HOME", directory.path().join("profile"))
        .env("RUST_BACKTRACE", "1")
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap();
    // This guardian includes cold component compilation. It never replaces either language budget.
    let deadline = Instant::now() + Duration::from_secs(180);
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
        "native language control child failed or stalled: {status:?}\n{log}"
    );
    assert!(log.contains(RECEIPT), "missing checked receipt:\n{log}");
    eprintln!("{log}");
}

/// Ordinary libtest deliberately creates the real Windows renderer and platform scheduler here.
#[test]
#[ignore = "private child of the bounded native language-control test"]
fn native_language_control_child() {
    let root = PathBuf::from(std::env::var_os(ROOT_ENV).expect("bounded parent root"));
    let workspace = root.join("workspace");
    let profile = root.join("profile");
    fs::create_dir_all(&workspace).unwrap();
    let path = workspace.join("source.txt");
    fs::write(&path, "source = 1\n").unwrap();
    let path = path.canonicalize().unwrap();
    let workspace = workspace.canonicalize().unwrap();
    let wire = root.join("wire.jsonl");
    let packages = module_timeout::prepare(control_package(&wire));
    let mut manager = Manager::open(
        extensions::profile_runtime_root(&profile),
        protocol::Environment {
            workspace: workspace.display().to_string(),
            os: "windows".into(),
            ..Default::default()
        },
    )
    .unwrap();
    for package in packages {
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
    }
    // Cold guest compilation precedes the measured controls; the native source still opens normally.
    extensions::demand_plugin_fixture(
        &mut manager,
        "capability-example",
        plugin_runtime::DemandReason::Command("scope-probe".into()),
    )
    .unwrap();
    let mut session = SessionState::load_in_profile(&workspace, &profile);
    session.workspace_trusted = true;
    let workspace = Workspace::open(&workspace).unwrap();
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
            let mut options = crate::ui::controls::window::TitleBar::window_options();
            options.show = true;
            options.focus = false;
            options.inactive_frame_interval = None;
            let initial = path.clone();
            let (main, app) = gpui_kit::open_window(options, cx, move |window, cx| {
                window.set_window_title("Nanobug — Native language control regression");
                cx.new(|cx| {
                    EditorApp::new_with_session(workspace, Some(initial), Some(session), window, cx)
                })
            })
            .unwrap();
            let harness = Harness {
                main: main.into(),
                app,
                manager: RefCell::new(manager),
                path,
                wire,
            };
            cx.spawn(async move |cx| {
                let mut scenario = Box::pin(harness.exercise(cx));
                let scenario_result = scenario.as_mut().await;
                drop(scenario);
                eprintln!("native language control: scenario {scenario_result:#?}");
                let cleanup = harness.shutdown(cx).await;
                eprintln!("native language control: checked cleanup {cleanup:#?}");
                let failures = [scenario_result, cleanup]
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
        .expect("native language control child did not report an outcome")
        .unwrap();
}

/// Observations retain public Manager receipts; editor text stays exclusively in EditorState/DocumentSession.
struct Harness {
    /// The sole native window created by this child; cleanup never searches other applications.
    main: AnyWindowHandle,
    /// The original application dispatcher and source-incarnation owner.
    app: Entity<EditorApp>,
    /// The granted SDK package executes in the real runtime, with existing publication adapters.
    manager: RefCell<Manager>,
    /// Parent-owned source whose close/reopen lifecycle must not reuse the old document identity.
    path: PathBuf,
    /// Actual framed server ingress, stored outside the watched project.
    wire: PathBuf,
}

impl Harness {
    /// Observe the public timeout outcome, matching wire cancellation and a still-usable native context.
    async fn exercise(&self, cx: &mut AsyncApp) -> anyhow::Result<()> {
        eprintln!("native language control phase: warm plain barrier");
        self.publish(cx)?;
        let warm = self.begin("fixture/barrier", 15_000, cx)?;
        self.finish(&warm, cx).await?;
        completed(&warm)?;
        let document = protocol::language::DocumentContext {
            path: "source.txt".into(),
            generation: self
                .manager
                .borrow_mut()
                .availability("capability-example")?
                .configuration_generation(),
        };
        let original = self.plan(&document)?;
        let started = Instant::now();
        let pending = self.begin("fixture/pending", 1_000, cx)?;
        self.finish(&pending, cx).await?;
        ensure!(
            matches!(
                pending.status(),
                api::RequestUpdate::Cancelled {
                    reason: api::ErrorCode::TimedOut,
                    ..
                }
            ),
            "withheld control must retain its original 1000ms timeout: {:?}",
            pending.status()
        );
        eprintln!(
            "native language control: 1000ms request observed after {:?}",
            started.elapsed()
        );
        self.wait_for_cancel(1, cx).await?;
        eprintln!(
            "native language control phase: plain timeout and matching cancellation verified"
        );
        let reused = self.begin("fixture/barrier", 15_000, cx)?;
        self.finish(&reused, cx).await?;
        completed(&reused)?;
        ensure!(
            Arc::ptr_eq(&original, &self.plan(&document)?),
            "timeout replaced a healthy native context"
        );
        ensure!(
            self.count("initialize")? == 1,
            "timeout restarted the native pipe"
        );
        eprintln!("native language control phase: same native context reused after timeout");
        self.closed_source_reopens(&document, &original, cx).await?;
        eprintln!(
            "native language control phase: source retirement/reopen verified; begin modules"
        );
        module_timeout::exercise(self, cx).await?;
        ensure!(
            fs::read_to_string(&self.path)? == "source = 1\n",
            "analysis changed the source file"
        );
        Ok(())
    }

    /// Close a genuinely pending original incarnation, then require a new identity and native plan on reopen.
    async fn closed_source_reopens(
        &self,
        document: &protocol::language::DocumentContext,
        original: &Arc<plugin_runtime::LanguageService>,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        eprintln!("native language control phase: source-close pending request");
        let pending = self.begin("fixture/pending", 15_000, cx)?;
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.count("fixture/pending")? != 2 {
            self.publish(cx)?;
            ensure!(
                !pending.status().is_terminal(),
                "source control ended before close: {:?}",
                pending.status()
            );
            ensure!(
                Instant::now() < deadline,
                "second pending control never entered the native wire"
            );
            cx.background_executor()
                .timer(Duration::from_millis(10))
                .await;
        }
        let previous = self.main.update(cx, |_, window, cx| {
            self.app.update(cx, |app, cx| {
                let previous = app
                    .plugin_document_version(app.active_text_tab_index().unwrap())
                    .unwrap();
                app.discard_tab(self.path.clone(), window, cx);
                ensure!(
                    app.tabs.iter().all(|tab| tab.path() != self.path),
                    "closed source stayed open"
                );
                Ok::<_, anyhow::Error>(previous)
            })
        })??;
        self.finish(&pending, cx).await?;
        ensure!(
            matches!(pending.status(), api::RequestUpdate::Completed { result: Err(ref error) } if error.code == api::ErrorCode::StaleRevision),
            "old source control must fail with StaleRevision: {:?}",
            pending.status()
        );
        self.publish(cx)?;
        ensure!(
            self.manager
                .borrow_mut()
                .language_document_bindings()
                .is_empty(),
            "closed source retained its public binding"
        );
        let current = self.main.update(cx, |_, window, cx| {
            self.app.update(cx, |app, cx| {
                app.open_file(self.path.clone(), window, cx);
                app.plugin_document_version(app.active_text_tab_index().unwrap())
            })
        })??;
        ensure!(
            previous.id != current.id,
            "reopened source reused its old incarnation"
        );
        eprintln!("native language control phase: source reopened with a new document identity");
        let fresh = self.begin("fixture/barrier", 15_000, cx)?;
        self.finish(&fresh, cx).await?;
        completed(&fresh)?;
        ensure!(
            !Arc::ptr_eq(original, &self.plan(document)?),
            "reopened source reused the retired plan"
        );
        ensure!(!original.is_active(), "closed source plan remained active");
        ensure!(
            self.count("initialize")? == 2,
            "reopen must create exactly one replacement pipe"
        );
        ensure!(
            matches!(pending.status(), api::RequestUpdate::Completed { result: Err(ref error) } if error.code == api::ErrorCode::StaleRevision),
            "new source changed the old control's terminal result"
        );
        Ok(())
    }

    /// Issue one SDK operation from its installed guest and dispatch only the runtime-sealed request.
    fn begin(
        &self,
        method: &str,
        timeout_ms: u32,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<plugin_runtime::EditorRequest> {
        self.begin_request(
            protocol::language_control::Request {
                provider: "analysis".into(),
                document: protocol::language::DocumentContext {
                    path: "source.txt".into(),
                    generation: 0,
                },
                method: method.into(),
                arguments: Value::Null,
                context_snapshot: false,
                service_sets: Vec::new(),
                imports: Vec::new(),
                publish_binding: None,
            },
            timeout_ms,
            cx,
        )
    }

    /// Preserve every declared import and original timeout while the SDK seals one native control operation.
    fn begin_request(
        &self,
        request: protocol::language_control::Request,
        timeout_ms: u32,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<plugin_runtime::EditorRequest> {
        let method = request.method.clone();
        let imports = request
            .imports
            .iter()
            .map(|import| import.export.clone())
            .collect::<Vec<_>>();
        eprintln!(
            "native control SDK request: method={method}, timeout_ms={timeout_ms}, imports={imports:?}"
        );
        let mut manager = self.manager.borrow_mut();
        manager.invoke_command(
            "capability-example",
            "scope-probe",
            serde_json::to_value(api::Operation::Editor {
                operation: api::EditorOperation::LanguageRequest { request },
                timeout_ms,
            })?,
        )?;
        let requests = manager.take_editor_requests();
        // A guest may return a structured admission error without issuing any editor request.
        // Preserve that actual SDK result and the observed queue instead of losing the failure phase.
        let diagnostic = manager
            .live
            .get("capability-example")
            .and_then(|instance| instance.views.get("welcome"))
            .and_then(|view| match &view.root.kind {
                protocol::ui::Kind::Text { text } => {
                    Some(text.chars().take(4096).collect::<String>())
                }
                _ => None,
            });
        ensure!(
            requests.len() == 1,
            "SDK must issue exactly one native request: method={method}, timeout_ms={timeout_ms}, imports={imports:?}, count={}, sdk_result={diagnostic:?}, queued={:?}",
            requests.len(),
            requests
                .iter()
                .map(|(owner, request)| (owner, request.operation(), request.status()))
                .collect::<Vec<_>>()
        );
        let (plugin, request) = requests.into_iter().next().unwrap();
        drop(manager);
        self.main.update(cx, |_, window, cx| {
            self.app.update(cx, |app, cx| {
                app.perform_editor_request(&plugin, request.clone(), window, cx)
            })
        })?;
        Ok(request)
    }

    /// Observe the original request within the unchanged 15s guard while production timers and IO advance.
    async fn finish(
        &self,
        request: &plugin_runtime::EditorRequest,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            self.publish(cx)?;
            let status = request.status();
            if status.is_terminal() {
                return Ok(());
            }
            ensure!(
                Instant::now() < deadline,
                "native control did not finish: {status:?}"
            );
            cx.background_executor()
                .timer(Duration::from_millis(10))
                .await;
        }
    }

    /// Reuse the existing actual-Manager publication adapter with real native frames, without synthetic results.
    fn publish(&self, cx: &mut AsyncApp) -> anyhow::Result<()> {
        self.main.update(cx, |_, window, cx| {
            extensions::package_ui_test_support::pump(
                &self.app,
                &mut self.manager.borrow_mut(),
                cx,
            );
            self.app
                .update(cx, |app, cx| app.sync_runtime_contributions(window, cx));
            window.draw(cx).clear(cx);
        })?;
        Ok(())
    }

    /// Fetch the active public context receipt for the original source, preserving Arc identity observations.
    fn plan(
        &self,
        document: &protocol::language::DocumentContext,
    ) -> anyhow::Result<Arc<plugin_runtime::LanguageService>> {
        self.manager
            .borrow_mut()
            .language_document_service("capability-example/analysis", document)
            .context("native context is not prepared")?
            .map_err(anyhow::Error::msg)
    }

    /// Read only complete ingress frames; matching IDs cannot be borrowed from another control.
    fn frames(&self) -> anyhow::Result<Vec<Value>> {
        match fs::read_to_string(&self.wire) {
            Ok(source) => source
                .split_inclusive('\n')
                .filter(|line| line.ends_with('\n'))
                .map(|line| serde_json::from_str(line).map_err(Into::into))
                .collect(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(error.into()),
        }
    }

    /// Count the actual server's observations, including independent process startups.
    fn count(&self, method: &str) -> anyhow::Result<usize> {
        Ok(self
            .frames()?
            .iter()
            .filter(|frame| frame["method"] == method)
            .count())
    }

    /// Await the exact pending request's standard cancellation before testing another request on its pipe.
    async fn wait_for_cancel(&self, occurrence: usize, cx: &mut AsyncApp) -> anyhow::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            self.publish(cx)?;
            let frames = self.frames()?;
            if let Some(pending) = frames
                .iter()
                .filter(|frame| frame["method"] == "fixture/pending")
                .nth(occurrence - 1)
                && frames.iter().any(|frame| {
                    frame["method"] == "$/cancelRequest" && frame["params"]["id"] == pending["id"]
                })
            {
                return Ok(());
            }
            ensure!(
                Instant::now() < deadline,
                "pending control {occurrence} has no matching cancellation frame"
            );
            cx.background_executor()
                .timer(Duration::from_millis(10))
                .await;
        }
    }

    /// Check runtime shutdown and remove only this child's original HWND after two actual blur frames.
    async fn shutdown(&self, cx: &mut AsyncApp) -> anyhow::Result<()> {
        let task = self
            .main
            .update(cx, |_, _, cx| {
                self.app
                    .update(cx, |app, cx| app.prepare_project_shutdown(cx))
            })?
            .map_err(anyhow::Error::msg)?;
        task.await.map_err(anyhow::Error::msg)?;
        self.manager.borrow_mut().shutdown_checked()?;
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
            let timeout = cx.background_executor().timer(Duration::from_secs(5));
            match futures::future::select(receive, Box::pin(timeout)).await {
                futures::future::Either::Left((result, _)) => {
                    result.context("native blur frame callback dropped")?
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
                unreachable!("Windows-only native acceptance");
            };
            let hwnd = handle.hwnd.get();
            window.remove_window();
            hwnd
        })?;
        let deadline = Instant::now() + Duration::from_secs(5);
        // SAFETY: the opaque identity was borrowed from this child's own window and is never dereferenced.
        while unsafe { IsWindow(hwnd as _) != 0 } {
            ensure!(
                Instant::now() < deadline,
                "owned language-control HWND survived cleanup"
            );
            cx.background_executor()
                .timer(Duration::from_millis(10))
                .await;
        }
        Ok(())
    }
}

/// A barrier must return its actual null result; a terminal failure or preserved resource count is insufficient.
fn completed(request: &plugin_runtime::EditorRequest) -> anyhow::Result<()> {
    ensure!(
        matches!(
            request.status(),
            api::RequestUpdate::Completed {
                result: Ok(api::EditorValue::Language(Value::Null))
            }
        ),
        "native barrier failed: {:?}",
        request.status()
    );
    Ok(())
}

/// Preserve the current SDK guest and normal package validation, changing only declared native fixture configuration.
fn control_package(wire: &Path) -> Package {
    let archive = PathBuf::from(
        std::env::var_os("NANOBUG_CONTROL_FIXTURE_PACKAGE").expect("current SDK package"),
    );
    let mut files = Package::read(&archive).unwrap().files;
    let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["api"]["required"]["language.control"] = json!("^1");
    manifest["api"]["required"]["language.lsp"] = json!("^1.5");
    manifest["api"]["required"]["process"] = json!("^1");
    manifest["permissions"]
        .as_array_mut()
        .unwrap()
        .push(json!("process.service.analysis"));
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/lsp_fixture.exe")
        .canonicalize()
        .expect("matching native lsp_fixture");
    manifest["services"] = json!({"analysis":{"program":executable,"args":[wire]}});
    manifest["language_servers"] = json!([{"id":"analysis","language":"unfamiliar-sidecar","service":"analysis",
    "control_methods":{
        "fixture/barrier":{"parameters":{"type":"null"},"result":{"type":"null"}},
        "fixture/pending":{"parameters":{"type":"null"},"result":{"type":"null"}}
    }}]);
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    Package::from_files(files).unwrap()
}
