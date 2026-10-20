//! A real Windows scheduler delivers native input while selected editing operations await stdio.
//! Only a parent-owned profile, package store, source and HWND participate in this acceptance test.
use super::*;
use crate::*;
use anyhow::{Context as _, ensure};
use gpui_kit::{AnyWindowHandle, AsyncApp, EntityInputHandler as _};
use raw_window_handle::RawWindowHandle;
use serde_json::{Value, json};
use std::{cell::RefCell, fs, rc::Rc};
use windows_sys::Win32::UI::WindowsAndMessaging::IsWindow;

mod completion;
mod scenario;

/// Range and selected-completion scenarios share only the OS window/actor/checked shutdown harness.
#[derive(Clone, Copy)]
enum NativeScenario {
    Range,
    Completion,
}

/// This private child is entered only by the bounded parent in `tests::language_editing`.
#[test]
#[ignore = "private child of in_flight_range_formatting_rejects_ime_preedit_even_when_bytes_return"]
fn range_formatting_ime_child() {
    run_native_child(NativeScenario::Range);
}

/// The parent admits an unfamiliar provider, then this child uses its ordinary completion menu.
#[test]
#[ignore = "private child of selected_unknown_completion_resolve_rejects_stale_sources_and_revoked_owners_whole"]
fn completion_resolve_lifetime_child() {
    run_native_child(NativeScenario::Completion);
}

/// Reuse one native scheduler and checked resource teardown without adding a host testing API.
fn run_native_child(kind: NativeScenario) {
    let (root_variable, title, wire_name) = match kind {
        NativeScenario::Range => (
            "NANOBUG_NATIVE_RANGE_FORMATTING_ROOT",
            "Nanobug — Range formatting IME regression",
            "ime-range.jsonl",
        ),
        NativeScenario::Completion => (
            "NANOBUG_NATIVE_COMPLETION_ROOT",
            "Nanobug — Completion resolve regression",
            "completion-resolve.jsonl",
        ),
    };
    let root = PathBuf::from(std::env::var_os(root_variable).expect("parent-owned root"));
    let workspace = root.join("workspace").canonicalize().unwrap();
    let path = workspace.join("sample.linked").canonicalize().unwrap();
    let disk = fs::read_to_string(&path).unwrap();
    let profile = root.join("profile");
    let runtime = extensions::profile_runtime_root(&profile);
    let environment = Environment {
        workspace: workspace.display().to_string(),
        os: "windows".into(),
        ..Default::default()
    };
    let mut session = SessionState::load_in_profile(&workspace, &profile);
    session.workspace_trusted = true;
    let project = Workspace::open(&workspace).unwrap();
    let outcome = Rc::new(RefCell::new(None::<Result<(), String>>));
    let completed = outcome.clone();
    gpui_kit::application().with_assets(AppAssets).with_quit_mode(gpui_kit::QuitMode::Explicit).run(move |cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
        extensions::init(cx);
        crate::ui::controls::init(cx);
        crate::app::shortcuts::bootstrap::configure(cx, profile.join("shortcuts.json")).unwrap();
        let actor = Arc::new(Worker::start_background(runtime, environment, true));
        let mut options = crate::ui::controls::window::TitleBar::window_options();
        options.show = true;
        options.focus = false;
        options.inactive_frame_interval = None;
        let (main, app) = gpui_kit::open_window(options, cx, move |window, cx| {
            window.set_window_title(title);
            cx.new(|cx| EditorApp::new_with_session(project, None, Some(session), window, cx))
        }).unwrap();
        app.read(cx).extensions.clone().update(cx, |panel, cx| { panel.worker = actor.clone(); panel.poll(cx); });
        let harness = Harness { main: main.into(), app, actor, path, wire: root.join(wire_name), disk };
        cx.spawn(async move |cx| {
            let observed = match kind {
                NativeScenario::Range => scenario::exercise(&harness, cx).await,
                NativeScenario::Completion => completion::exercise(&harness, cx).await,
            };
            eprintln!("native selected editing scenario: {observed:#?}");
            let cleanup = harness.shutdown(cx).await;
            eprintln!("native range formatting checked cleanup: {cleanup:#?}");
            let failures = [observed, cleanup].into_iter().filter_map(Result::err).map(|error| format!("{error:#}")).collect::<Vec<_>>();
            *completed.borrow_mut() = Some(if failures.is_empty() {
                match kind {
                    NativeScenario::Range => eprintln!("native range formatting: pending IME, fresh range, Undo and cleanup verified"),
                    NativeScenario::Completion => eprintln!("native completion: selected resolve, whole refusals, Undo and cleanup verified"),
                }
                Ok(())
            } else { Err(failures.join("\n")) });
            drop(harness);
            cx.update(|cx| cx.quit());
        }).detach();
    });
    outcome
        .borrow_mut()
        .take()
        .expect("native range child did not report an outcome")
        .unwrap();
}

/// The application owns all mutable text; this harness retains only immutable expectations and leases.
struct Harness {
    main: AnyWindowHandle,
    app: Entity<EditorApp>,
    actor: Arc<Worker>,
    path: PathBuf,
    wire: PathBuf,
    disk: String,
}

impl Harness {
    /// Publish the production actor and paint only the original child window.
    fn frame(&self, cx: &mut AsyncApp) -> anyhow::Result<()> {
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

    /// This wall-clock observer never changes a service or native input budget.
    async fn wait(
        &self,
        phase: &str,
        mut ready: impl FnMut(&EditorApp, &App) -> bool,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            self.frame(cx)?;
            if self
                .main
                .update(cx, |_, _, cx| ready(self.app.read(cx), cx))?
            {
                return Ok(());
            }
            ensure!(
                Instant::now() < deadline,
                "native range phase timed out: {phase}"
            );
            cx.background_executor()
                .timer(Duration::from_millis(5))
                .await;
        }
    }

    /// Ignore an in-progress final write, while rejecting malformed newline-terminated evidence.
    fn frames(&self) -> anyhow::Result<Vec<Value>> {
        let source = match fs::read_to_string(&self.wire) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        source
            .split_inclusive('\n')
            .filter(|line| line.ends_with('\n'))
            .enumerate()
            .map(|(index, line)| {
                serde_json::from_str(line)
                    .with_context(|| format!("malformed native range wire frame {}", index + 1))
            })
            .collect()
    }

    /// Await an actual stdio event without serializing the formatter's dedicated transport thread.
    async fn wire_event(
        &self,
        mut ready: impl FnMut(&Value) -> bool,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<Value> {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(event) = self.frames()?.into_iter().find(&mut ready) {
                return Ok(event);
            }
            self.frame(cx)?;
            ensure!(
                Instant::now() < deadline,
                "native range wire event timed out"
            );
            cx.background_executor()
                .timer(Duration::from_millis(5))
                .await;
        }
    }

    /// Invoke the registered native action after setting the actual EditorState selection.
    fn select_and_format(
        &self,
        range: std::ops::Range<usize>,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        self.main.update(cx, |_, window, cx| {
            window.activate_window();
            self.app.read(cx).editor.clone().update(cx, |editor, cx| {
                editor.set_selected_range(range, cx);
                editor.focus(window, cx);
                cx.notify();
            });
            window.dispatch_action(
                cx.build_action("me_editor::FormatSelection", None)
                    .expect("registered native range action"),
                cx,
            );
        })?;
        Ok(())
    }

    /// A real foreground frame releases queued callbacks; the five-second guard only diagnoses a stuck window.
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
                result.context("native range frame dropped")
            }
            futures::future::Either::Right(_) => anyhow::bail!("native range frame timed out"),
        }
    }

    /// Checked shutdown proves transport retirement before the only owned HWND is destroyed.
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
            "range actor did not acknowledge shutdown"
        );
        ensure!(
            plans.iter().all(|plan| plan.process_count() == 0),
            "native range processes survived checked shutdown"
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
                unreachable!("Windows-only native range child")
            };
            let hwnd = handle.hwnd.get();
            window.remove_window();
            hwnd
        })?;
        let deadline = Instant::now() + Duration::from_secs(5);
        // SAFETY: the opaque handle came from this child's own window and is never dereferenced.
        while unsafe { IsWindow(hwnd as _) != 0 } {
            ensure!(
                Instant::now() < deadline,
                "native range HWND survived cleanup"
            );
            cx.background_executor()
                .timer(Duration::from_millis(10))
                .await;
        }
        ensure!(
            fs::read_to_string(&self.path)? == self.disk,
            "range formatting saved a project source"
        );
        Ok(())
    }
}
