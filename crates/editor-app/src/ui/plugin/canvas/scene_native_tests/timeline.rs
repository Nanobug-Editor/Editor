//! Actual timeline package, native PluginView and one-shot GPUI leases share a bounded update path.
use super::*;
use gpui_kit::{AnyWindowHandle, AppContext, Application, WindowBounds, WindowOptions};
use std::io::{Read, Seek};
use wasm::timeline::{Actor, Input, Snapshot};
mod lifecycle;
mod perf;
mod smoke;

/// One owned child process keeps actual platform state and retirement separate from the test runner.
#[test]
#[ignore = "opens an actual timeline Package/Windows/D3D window; run explicitly with --ignored --test-threads=1"]
fn native_canvas_timeline_real_frames_scroll_and_idle() {
    const CHILD: &str = "NANOBUG_CANVAS_TIMELINE_NATIVE_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let mut log = tempfile::tempfile().unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "native_canvas_timeline_real_frames_scroll_and_idle",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD, "1")
            .env("RUST_MIN_STACK", "8388608")
            .stdout(log.try_clone().unwrap())
            .stderr(log.try_clone().unwrap())
            .spawn()
            .unwrap();
        let seconds = std::env::var("NANOBUG_CANVAS_TIMELINE_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        let deadline = Instant::now() + Duration::from_secs(seconds + 180);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("timeline native child timed out");
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        log.rewind().unwrap();
        let mut output = String::new();
        log.read_to_string(&mut output).unwrap();
        eprintln!("{output}");
        assert!(status.success(), "timeline native child failed: {status}");
        return;
    }
    run().unwrap();
}

/// Cold component preparation is deliberately outside measured native frames.
fn run() -> anyhow::Result<()> {
    let mut actor = Actor::start()?;
    let initial = loop {
        if let Some(snapshot) = actor.take() {
            break snapshot;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let port = actor.port.clone();
    let result = Rc::new(RefCell::new(None));
    let output = result.clone();
    let platform = gpui_kit::platform::current_platform(false);
    let fonts = platform.text_system();
    Application::with_platform(platform)
        .with_assets(gpui_kit::assets::Assets)
        .with_quit_mode(gpui_kit::QuitMode::Explicit)
        .run(move |cx| {
            editor_ui::text::paragraph::install_native_raster_fonts(fonts, cx).unwrap();
            gpui_kit::init(cx);
            // Match ordinary application startup: Base Root owns the native nested-wheel router.
            editor_ui::init(cx);
            editor_ui::typography::init(cx);
            editor_ui::theme::apply_theme(editor_ui::theme::builtin_theme(false), cx);
            let saved = Rc::new(RefCell::new(None));
            let retained = saved.clone();
            let sink = port.clone();
            let document = initial.document.as_ref().clone();
            let images = initial.images.clone();
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                            point(px(80.), px(80.)),
                            size(px(640.), px(360.)),
                        ))),
                        ..Default::default()
                    },
                    move |window, cx| {
                        let view = cx.new(|cx| {
                            PluginView::new(
                                "canvas-timeline".into(),
                                document,
                                Environment::default(),
                                move |event, _| {
                                    let _ = sink.send(Input::Ui(event));
                                },
                                window,
                                cx,
                            )
                        });
                        view.update(cx, |view, cx| {
                            view.update_images("canvas-timeline/timeline", &images, window, cx)
                        });
                        *retained.borrow_mut() = Some(view.clone());
                        cx.new(|cx| gpui_base::Root::new(view, window, cx))
                    },
                )
                .unwrap();
            let window: AnyWindowHandle = window.into();
            cx.spawn(async move |cx| {
                let view = saved.borrow().as_ref().unwrap().clone();
                let mut snapshot = initial;
                let observed = async {
                    let deadline = Instant::now() + Duration::from_secs(6);
                    actor
                        .port
                        .send(Input::Command("business-timer", serde_json::Value::Null))?;
                    actor
                        .port
                        .send(Input::Command("animate", serde_json::Value::Null))?;
                    while snapshot.state["frames"].as_u64().unwrap_or(0) < 3 {
                        if let Some(next) = actor.take() {
                            apply(&view, window, &mut snapshot, next, cx)?;
                        }
                        anyhow::ensure!(
                            Instant::now() < deadline,
                            "actual visible GPUI opportunities did not reach the timeline: {}",
                            snapshot.state
                        );
                        cx.background_executor()
                            .timer(Duration::from_millis(8))
                            .await;
                    }
                    anyhow::ensure!(
                        snapshot.queue_overflow == 0,
                        "native actor cannot silently discard input"
                    );
                    eprintln!(
                        "actual timeline visible frames and business state: {}",
                        snapshot.state
                    );
                    let handle = smoke::exercise(&actor, &view, window, &mut snapshot, cx).await?;
                    perf::run(&actor, &view, window, &mut snapshot, handle, cx).await?;
                    Ok::<(), anyhow::Error>(())
                }
                .await;
                // No guest publication may enqueue more native work after retirement starts.
                actor.stop();
                let retired = lifecycle::retire(&view, window, cx).await;
                // Native input and paint buffers have retired before the final retained owners.
                saved.borrow_mut().take();
                drop(view);
                drop(snapshot);
                drop(actor);
                let no_windows = cx.update(|cx| {
                    anyhow::ensure!(
                        cx.windows().is_empty(),
                        "timeline retained a GPUI window after retirement"
                    );
                    Ok(())
                });
                let failures = [observed, retired, no_windows]
                    .into_iter()
                    .filter_map(Result::err)
                    .map(|error| format!("{error:#}"))
                    .collect::<Vec<_>>();
                *output.borrow_mut() = Some(if failures.is_empty() {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(failures.join("\n")))
                });
                let _ = cx.update(|cx| cx.quit());
            })
            .detach();
        });
    let observed = result
        .borrow_mut()
        .take()
        .expect("timeline native window completed");
    observed
}

/// UI updates use the same document/image and frame-request entry points as ordinary native panels.
fn apply(
    view: &Entity<PluginView>,
    window: AnyWindowHandle,
    current: &mut Snapshot,
    next: Snapshot,
    cx: &mut gpui_kit::AsyncApp,
) -> anyhow::Result<()> {
    if let Some(error) = &next.error {
        anyhow::bail!("{error}");
    }
    anyhow::ensure!(
        next.frames.len() <= 1,
        "one Canvas cannot own multiple pending native frame requests"
    );
    if next.document.revision != current.document.revision || next.images.changed(&current.images) {
        let document = next.document.as_ref().clone();
        let images = next.images.clone();
        window.update(cx, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.update_document(document, Environment::default(), window, cx);
                view.update_images("canvas-timeline/timeline", &images, window, cx);
            })
        })?;
    }
    view.update(cx, |view, cx| {
        view.request_canvas_frames(next.frames.clone(), cx)
    });
    *current = next;
    Ok(())
}
