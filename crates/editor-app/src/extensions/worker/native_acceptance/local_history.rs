//! Local history runs in genuine isolated Windows windows, independently of a Git/plugin fixture.

use super::*;
use gpui_base::test_support::snapshots;
use gpui_kit::{EntityInputHandler as _, Role};
use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};
mod entrypoints;
mod scene;
mod timings;

const CHILD: &str = "extensions::worker::native_acceptance::local_history::history_native_child";

/// Both native palettes/locales and a fractional DPI use fresh processes and parent-owned profiles.
#[test]
#[ignore = "requires a real Windows desktop/D3D11 and NANOBUG_HISTORY_NATIVE_OUTPUT"]
fn history_native_matrix() {
    let output = PathBuf::from(
        std::env::var_os("NANOBUG_HISTORY_NATIVE_OUTPUT").expect("native output required"),
    );
    for case in [Case::SMOKE, Case::ALL[7]] {
        let directory = tempfile::tempdir().unwrap();
        let output = output.join(case.name);
        fs::create_dir_all(&output).unwrap();
        let log = fs::File::create(output.join("native.log")).unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                CHILD,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("NANOBUG_GIT_NATIVE_CASE", case.name)
            .env("NANOBUG_HISTORY_NATIVE_ROOT", directory.path())
            .env("NANOBUG_HISTORY_NATIVE_OUTPUT", &output)
            .env("ME_EDITOR_PROFILE_HOME", directory.path().join("profile"))
            .env("RUST_MIN_STACK", "8388608")
            .stdin(Stdio::null())
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(120);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "native {} timed out: {}",
                    case.name,
                    fs::read_to_string(output.join("native.log")).unwrap()
                );
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(
            status.success(),
            "native {}: {status}: {}",
            case.name,
            fs::read_to_string(output.join("native.log")).unwrap()
        );
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(output.join("evidence.json")).unwrap()).unwrap();
        assert_eq!(receipt["shutdown_complete"], true);
    }
}

/// Only the bounded parent launches this real WindowsPlatform; no TestAppContext is involved.
#[test]
#[ignore = "spawn through history_native_matrix with a parent-owned temporary profile"]
fn history_native_child() {
    let case = Case::from_environment().unwrap();
    let root = PathBuf::from(std::env::var_os("NANOBUG_HISTORY_NATIVE_ROOT").unwrap());
    let output = PathBuf::from(std::env::var_os("NANOBUG_HISTORY_NATIVE_OUTPUT").unwrap());
    let project = root.join("project");
    let profile = root.join("profile");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&profile).unwrap();
    let path = project.join("记录.txt");
    fs::write(&path, "原始内容\r\nsecond line\nthird line\n").unwrap();
    let result = Rc::new(RefCell::new(None));
    let completed = result.clone();
    rust_i18n::set_locale(case.locale);
    gpui_kit::application()
        .with_assets(AppAssets)
        .with_quit_mode(gpui_kit::QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            editor_ui::init(cx);
            typography::init(cx);
            apply_theme(builtin_theme(case.dark), cx);
            cx.set_reduce_motion(true);
            extensions::init(cx);
            app::shortcuts::bootstrap::configure(cx, profile.join("shortcuts.json")).unwrap();
            let mut session = SessionState::load_in_profile(&project, &profile);
            session.messages_visible = false;
            let workspace = Workspace::open(&project).unwrap();
            let mut options = editor_ui::window::TitleBar::window_options();
            options.window_bounds = Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1150.), px(820.)),
                cx,
            )));
            options.show = true;
            options.focus = true;
            options.inactive_frame_interval = None;
            let (window, app) = gpui_kit::open_window(options, cx, move |window, cx| {
                cx.new(|cx| {
                    EditorApp::new_with_session(workspace, Some(path), Some(session), window, cx)
                })
            })
            .unwrap();
            let native = Native {
                window,
                app,
                output,
                case,
                evidence: RefCell::new(serde_json::json!({
                    "case": case.name, "platform": "Windows/D3D11", "shutdown_complete": false,
                    "os_ime_candidates_verified": false, "observations": {}, "screenshots": {}
                })),
            };
            cx.activate(true);
            cx.spawn(async move |cx| native.run(completed, cx).await)
                .detach();
        });
    result
        .borrow_mut()
        .take()
        .expect("native history did not report a verdict")
        .unwrap();
}

/// Own only production window/editor handles and read-only evidence, without an alternate document.
struct Native {
    window: AnyWindowHandle,
    app: Entity<EditorApp>,
    output: PathBuf,
    case: Case,
    evidence: RefCell<serde_json::Value>,
}

impl Native {
    /// Always drain production cleanup and retire native input/window, including after a failed scene.
    async fn run(self, result: Rc<RefCell<Option<Result<(), String>>>>, cx: &mut AsyncApp) {
        let observed = scene::exercise(&self, cx).await;
        eprintln!("history native scene: {observed:#?}");
        if observed.is_err() {
            let _ = self.capture("failure.png", cx);
        }
        let cleanup = self.window.update(cx, |_, _, cx| {
            self.app
                .update(cx, |app, cx| app.prepare_project_shutdown(cx))
        });
        let cleanup = match cleanup {
            Ok(Ok(task)) => task.await.map_err(anyhow::Error::msg),
            Ok(Err(error)) => Err(anyhow!(error)),
            Err(error) => Err(error),
        };
        let _ = self.window.update(cx, |_, window, cx| window.blur(cx));
        let input_retired = self.frame(cx).await;
        let input_retired = input_retired.and(self.frame(cx).await);
        let native = self
            .window
            .update(cx, |_, window, _| windows::OwnedWindow::for_window(window));
        let _ = self.window.update(cx, |_, window, _| {
            window.remove_window();
        });
        let retired = match native {
            Ok(Ok(native)) => windows::wait_for_retirement(&[native], cx).await,
            Ok(Err(error)) | Err(error) => Err(error),
        };
        let verdict = observed.and(cleanup).and(input_retired).and(retired);
        self.evidence.borrow_mut()["shutdown_complete"] = verdict.is_ok().into();
        self.evidence.borrow_mut()["verdict"] = format!("{verdict:#?}").into();
        let saved = fs::write(
            self.output.join("evidence.json"),
            serde_json::to_vec_pretty(&*self.evidence.borrow()).unwrap(),
        );
        *result.borrow_mut() = Some(
            verdict
                .and(saved.map_err(anyhow::Error::from))
                .map_err(|error| format!("{error:#}")),
        );
        drop(self);
        cx.update(|cx| cx.quit());
    }

    /// A real platform next-frame callback confirms dispatch and painting have run outside entity borrows.
    async fn frame(&self, cx: &mut AsyncApp) -> anyhow::Result<()> {
        self.frame_for(self.window, cx).await
    }

    /// Native settings retain a separate real root while sharing this editor's profile worker.
    async fn frame_for(&self, target: AnyWindowHandle, cx: &mut AsyncApp) -> anyhow::Result<()> {
        let (send, receive) = futures::channel::oneshot::channel();
        target.update(cx, |_, window, cx| {
            window.draw(cx).clear(cx);
            window.on_next_frame(move |_, _| {
                let _ = send.send(());
            });
            window.refresh();
        })?;
        match futures::future::select(
            receive,
            Box::pin(cx.background_executor().timer(Duration::from_secs(5))),
        )
        .await
        {
            futures::future::Either::Left((result, _)) => {
                result.context("native history frame dropped")
            }
            futures::future::Either::Right(_) => Err(anyhow!("native history frame timed out")),
        }
    }

    /// Observe actual current UI state while native frames continue to deliver accepted storage replies.
    async fn until(
        &self,
        label: &str,
        mut ready: impl FnMut(&EditorApp, &App) -> bool,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            self.frame(cx).await?;
            if cx.update(|cx| ready(self.app.read(cx), cx)) {
                return Ok(());
            }
            ensure!(
                Instant::now() < deadline,
                "native history timed out: {label}"
            );
        }
    }

    /// Dispatch a pointer at the current visible enabled native control's actual painted geometry.
    async fn click(&self, id: &str, cx: &mut AsyncApp) -> anyhow::Result<()> {
        self.click_for(self.window, id, cx).await
    }

    /// The same native control route applies to the owned settings window.
    async fn click_for(
        &self,
        target: AnyWindowHandle,
        id: &str,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        self.frame_for(target, cx).await?;
        target.update(cx, |_, window, cx| {
            let node = snapshots(window)
                .into_iter()
                .find(|node| {
                    node.role() == Some(Role::Button) && node.visible() && input::named(node, id)
                })
                .with_context(|| format!("history button {id} not painted"))?;
            input::click(window, &node, cx)
        })??;
        self.frame_for(target, cx).await
    }

    /// Persist only this owned native window's real D3D pixels and geometry.
    fn capture(&self, name: &str, cx: &mut AsyncApp) -> anyhow::Result<()> {
        let image = self.window.update(cx, |_, window, cx| {
            window.draw(cx).clear(cx);
            windows::capture(window, &self.output.join(name))
        })??;
        self.evidence.borrow_mut()["screenshots"][name] = image;
        Ok(())
    }
}
