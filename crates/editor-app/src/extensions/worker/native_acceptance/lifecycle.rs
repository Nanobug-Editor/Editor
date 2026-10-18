//! Preserve the observation error before native resource retirement can report a second failure.
use super::*;

/// A completed async scenario can still own editor handles in its generator. Drop that generator,
/// await production shutdown and actual native-window destruction before recording success or quitting.
pub(super) async fn run(
    harness: Harness,
    completed: Rc<RefCell<Option<Result<(), String>>>>,
    cx: &mut AsyncApp,
) {
    let mut scenario = Box::pin(scenario::exercise(&harness, cx));
    let observed = scenario.as_mut().await;
    drop(scenario);
    let observation_receipt = report(&harness, "scenario", &observed);
    if observed.is_err() {
        // Failure pixels belong to this run and do not replace its original error or count as success.
        let image = harness.capture(harness.main, "failure-workbench.png", cx);
        let _ = report(&harness, "failure_capture", &image);
    }

    let diagnostic_output = harness.fixture.output.clone();
    let diagnostic_thread = harness
        .main
        .update(cx, |_, window, _| {
            windows::OwnedUiThread::for_window(window)
        })
        .and_then(|thread| thread)
        .map_err(|error| format!("{error:#}"));
    trace_window_identities(
        "before_project_shutdown",
        &diagnostic_thread,
        &diagnostic_output,
        cx,
    );
    // Production cleanup drains the work-window map, so capture every original native owner first.
    let native = capture_native_windows(&harness, cx);
    let capture_receipt = report(&harness, "native_window_capture", &native.capture);
    let cleanup = match harness.main.update(cx, |_, _, cx| {
        harness
            .app
            .update(cx, |app, cx| app.prepare_project_shutdown(cx))
    }) {
        Ok(Ok(task)) => task.await.map_err(anyhow::Error::msg),
        Ok(Err(error)) => Err(anyhow!(error)),
        Err(error) => Err(error),
    };
    let cleanup_receipt = report(&harness, "project_shutdown", &cleanup);
    let retired = retire_native_input(&harness, cx).await;
    let retirement_receipt = report(&harness, "native_input_retirement", &retired);
    let removed = harness
        .main
        .update(cx, |_, window, _| window.remove_window());
    eprintln!("native child: main window retirement {removed:?}");
    let removal_receipt = report(&harness, "main_window_removal", &removed);
    let native_retired = windows::wait_for_retirement(&native.windows, cx).await;
    let retirement_snapshot = trace_window_identities(
        "after_original_retirement_wait",
        &diagnostic_thread,
        &diagnostic_output,
        cx,
    );
    let no_native_owners = windows::verify_retirement(&retirement_snapshot);
    let owner_receipt = report(&harness, "no_native_window_owners", &no_native_owners);
    let native_receipt = report_native_retirement(&harness, &native, &native_retired);
    let mut errors = [
        observed,
        observation_receipt,
        native.capture,
        capture_receipt,
        cleanup,
        cleanup_receipt,
        retired,
        retirement_receipt,
        removed,
        removal_receipt,
        native_retired,
        native_receipt,
        no_native_owners,
        owner_receipt,
    ]
    .into_iter()
    .filter_map(Result::err)
    .map(|error| format!("{error:#}"))
    .collect::<Vec<_>>();
    if !harness
        .worker
        .shutdown_complete
        .load(std::sync::atomic::Ordering::Acquire)
    {
        errors.push("actor shutdown not acknowledged".into());
    }
    let verdict = if errors.is_empty() {
        harness.finish_evidence()
    } else {
        Err(anyhow!(errors.join("\n")))
    };
    // Print before Application's leak detector drops, so a second panic cannot hide the real failure.
    eprintln!("native {}: verdict {verdict:#?}", harness.case.name);
    *completed.borrow_mut() = Some(verdict.map_err(|error| format!("{error:#}")));
    drop(harness);
    trace_window_identities(
        "after_harness_drop_before_quit",
        &diagnostic_thread,
        &diagnostic_output,
        cx,
    );
    cx.update(|cx| cx.quit());
}

/// Persist and return a read-only identity observation without changing focus or close ordering.
/// The caller may verify the returned snapshot; a file-write failure does not discard the observation.
fn trace_window_identities(
    phase: &str,
    owner: &Result<windows::OwnedUiThread, String>,
    output: &std::path::Path,
    cx: &mut AsyncApp,
) -> serde_json::Value {
    let mut snapshot = cx.update(|cx| match owner {
        Ok(owner) => owner.snapshot(cx),
        Err(error) => serde_json::json!({"capture_error": error}),
    });
    snapshot["phase"] = phase.into();
    eprintln!("native shutdown windows {phase}: {snapshot}");
    let saved = (|| -> anyhow::Result<()> {
        std::fs::write(
            output.join(format!("shutdown-windows-{phase}.json")),
            serde_json::to_vec_pretty(&snapshot)?,
        )?;
        Ok(())
    })();
    if let Err(error) = saved {
        eprintln!("native shutdown window probe could not save {phase}: {error:#}");
    }
    snapshot
}

/// Capture completeness stays separate from retirement so one failed handle cannot hide other owners.
struct NativeWindows {
    /// Main plus every plugin work window present before production shutdown drains their map.
    original_count: usize,
    /// Only identities borrowed from this child's live GPUI windows may be queried later.
    windows: Vec<windows::OwnedWindow>,
    /// Missing captures fail the verdict even if every successfully captured HWND is destroyed.
    capture: anyhow::Result<()>,
}

/// Borrow `harness`'s main and plugin windows through `cx` before cleanup loses their native handles.
fn capture_native_windows(harness: &Harness, cx: &mut AsyncApp) -> NativeWindows {
    let handles = cx.update(|cx| {
        let mut handles = vec![harness.main];
        handles.extend(
            harness
                .app
                .read(cx)
                .plugin_windows
                .values()
                .copied()
                .map(AnyWindowHandle::from),
        );
        handles
    });
    let mut windows = Vec::with_capacity(handles.len());
    let mut errors = Vec::new();
    let original_count = handles.len();
    for (index, handle) in handles.into_iter().enumerate() {
        match handle
            .update(cx, |_, window, _| windows::OwnedWindow::for_window(window))
            .and_then(|native| native)
        {
            Ok(window) => windows.push(window),
            Err(error) => errors.push(format!("original window {index}: {error:#}")),
        }
    }
    NativeWindows {
        original_count,
        windows,
        capture: if errors.is_empty() {
            Ok(())
        } else {
            Err(anyhow!(errors.join("\n")))
        },
    }
}

/// Persist actual remaining HWNDs and capture failures after the bounded wait, before final evidence.
fn report_native_retirement(
    harness: &Harness,
    original: &NativeWindows,
    retired: &anyhow::Result<()>,
) -> anyhow::Result<()> {
    let remaining = original
        .windows
        .iter()
        .copied()
        .filter(|window| window.is_alive())
        .collect::<Vec<_>>();
    eprintln!(
        "native {}: native_window_retirement {retired:#?}; original_count={} captured_count={} remaining={remaining:?}",
        harness.case.name,
        original.original_count,
        original.windows.len(),
    );
    harness.checkpoint(
        "lifecycle_native_window_retirement",
        serde_json::json!({
            "ok": original.capture.is_ok() && retired.is_ok(),
            "original_count": original.original_count,
            "captured_count": original.windows.len(),
            "remaining": remaining.len(),
            "remaining_windows": format!("{remaining:?}"),
            "capture_error": original.capture.as_ref().err().map(|error| format!("{error:#}")),
            "error": retired.as_ref().err().map(|error| format!("{error:#}")),
        }),
    )
}

/// Diagnostics are persisted independently from the verdict; a cleanup error never overwrites the scenario.
fn report(harness: &Harness, phase: &str, result: &anyhow::Result<()>) -> anyhow::Result<()> {
    eprintln!("native {}: {phase} {result:#?}", harness.case.name);
    harness.checkpoint(
        &format!("lifecycle_{phase}"),
        serde_json::json!({"ok": result.is_ok(), "error": result.as_ref().err().map(|error| format!("{error:#}"))}),
    )
}

/// Draw two native frames after blur. Both GPUI paint buffers and the Windows input handler may
/// retain the original EditorState; no synthetic frame or fixed sleep stands in for their release.
async fn retire_native_input(harness: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    for turn in 0..2 {
        eprintln!("native {}: retirement frame {turn}", harness.case.name);
        let (send, receive) = futures::channel::oneshot::channel();
        harness.main.update(cx, |_, window, cx| {
            window.blur(cx);
            window.refresh();
            window.draw(cx).clear(cx);
            window.on_next_frame(move |_, _| {
                let _ = send.send(());
            });
        })?;
        let timeout = cx.background_executor().timer(Duration::from_secs(5));
        match futures::future::select(receive, Box::pin(timeout)).await {
            futures::future::Either::Left((result, _)) => {
                result.context("retirement frame callback dropped")?;
            }
            futures::future::Either::Right(_) => {
                anyhow::bail!("native retirement frame timed out");
            }
        }
    }
    Ok(())
}
