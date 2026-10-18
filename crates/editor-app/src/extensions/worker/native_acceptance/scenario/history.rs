//! Native history pointer, wheel and keyboard routes open exact immutable source comparisons.
use super::*;
use plugin_runtime::plugin_protocol::Paint;
mod phase_probe;

/// The log's pointer, wheel and keyboard routes select immutable history; the file tree opens two readonly panes.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.settle(cx).await?;
    let key_trace = observe_keys(h, cx)?;
    let log_point = h
        .main
        .update(cx, |_, window, _| -> anyhow::Result<Point<Pixels>> {
            let branch_tree = input::tree(window, "branches")?;
            let branches = branch_tree.bounds();
            let content = gpui_base::test_support::snapshots(window)
                .into_iter()
                .filter(|node| {
                    node.visible()
                        && branch_tree.path().starts_with(node.path())
                        && matches!(node.path().last(), Some(gpui_kit::ElementId::Name(name)) if name.as_ref() == "local-dock-content-frame")
                })
                .max_by_key(|node| node.path().len())
                .context("history tree has no painted dock content ancestor")?
                .bounds();
            // The default dock must show the header and at least three complete 28px commit rows.
            // Measuring the shared native body also catches footer controls consuming its height.
            ensure!(
                branches.size.height >= px(4. * 28.),
                "history body cannot display three complete rows: {branches:?}"
            );
            // The footer can wrap or move controls to the left. Its button edges cannot identify
            // the canvas. Use the branch tree's actual dock-content ancestor for the right edge;
            // both history children occupy the same bounded horizontal body above that footer.
            ensure!(
                content.right() - branches.right() > px(64.),
                "history canvas has no usable native width: {branches:?}, {content:?}"
            );
            let canvas_region = Bounds::from_corners(
                point(branches.right(), branches.top()),
                point(content.right(), branches.bottom()),
            );
            let log_point = point(canvas_region.center().x, branches.top() + px(38.));
            ensure!(
                canvas_region.contains(&log_point)
                    && content.contains(&log_point)
                    && Bounds::new(point(px(0.), px(0.)), window.viewport_size()).contains(&log_point)
                    && !branches.contains(&log_point),
                "history pointer is outside its native canvas body: {log_point:?}"
            );
            h.record(
                "history_body_geometry",
                serde_json::json!({
                    "body": format!("{branches:?}"), "minimum_complete_commit_rows": 3,
                    "dock_content": format!("{content:?}"),
                    "canvas_region": format!("{canvas_region:?}"),
                    "history_pointer": format!("{log_point:?}")
                }),
            );
            Ok(log_point)
        })??;
    let log_focus = select_first_history_row(h, log_point, cx).await?;
    let offset_before = log_offset(h);
    h.main.update(cx, |_, window, cx| {
        input::wheel(window, log_point, -160., cx);
        Ok::<_, anyhow::Error>(())
    })??;
    h.until(
        h.main,
        "history wheel publication",
        |_, _| Ok(log_offset(h) > offset_before),
        cx,
    )
    .await?;
    let offset_after_wheel = log_offset(h);
    canvas_key(h, &log_focus, "end", &key_trace, cx).await?;
    h.until(
        h.main,
        "End selected and revealed the oldest history row",
        |_, _| {
            Ok(log_offset(h) > offset_after_wheel
                && history_row_selected(h, "Initial native review"))
        },
        cx,
    )
    .await?;
    canvas_key(h, &log_focus, "enter", &key_trace, cx).await?;
    let details = work_window(h, "commit-details", cx).await?;
    h.dpi(details, cx).await?;
    let root = h.fixture.workspace.clone();
    let oldest = cx
        .background_executor()
        .spawn(async move { fixture::git_text(&root, &["rev-list", "--max-parents=0", "HEAD"]) })
        .await?;
    h.until(
        details,
        "Enter opened the selected oldest commit identity",
        |_, _| {
            h.record("history_key_trace", serde_json::json!(*key_trace.borrow()));
            let state = h.worker.state.lock().unwrap();
            Ok(state
                .views
                .get("git/commit-details")
                .and_then(|view| view.active_node("commit-identity"))
                .is_some_and(
                    |node| matches!(&node.kind, Kind::Text { text } if text == oldest.trim()),
                ))
        },
        cx,
    )
    .await?;
    if std::env::var_os("NANOBUG_GIT_NATIVE_HISTORY_ONLY").is_some() {
        if std::env::var_os("NANOBUG_GIT_NATIVE_HISTORY_PHASES").is_some() {
            phase_probe::exercise(h, &log_focus, details, oldest.trim(), &key_trace, cx).await?;
        }
        h.record(
            "history_keyboard_probe",
            serde_json::json!({
                "exact_oldest_oid": oldest.trim(), "single_enter": true,
                "diagnostic_only": true, "original_focus_retained": true,
            }),
        );
        h.capture(details, "history-keyboard-probe.png", cx)?;
        return Ok(());
    }
    h.expand(details, "files", "src", cx).await?;
    h.select_file(details, "files", "a-review.txt", cx).await?;
    h.until(
        details,
        "readonly history sources",
        |window, _| {
            Ok(input::source(window, "snapshot-0").is_ok()
                && input::source(window, "snapshot-1").is_ok())
        },
        cx,
    )
    .await?;
    let root = h.fixture.workspace.clone();
    let commit = oldest.trim().to_owned();
    let initial_text = cx
        .background_executor()
        .spawn(async move {
            fixture::git_text(
                &root,
                &["show", &format!("{commit}:{}", fixture::REVIEW_PATH)],
            )
        })
        .await?;
    readonly_input(h, details, &initial_text, cx).await?;
    h.record("readonly_history", serde_json::json!({"commit": oldest.trim(), "source_labels": source_titles(h, "commit-details"), "wheel_before": offset_before, "wheel_after": log_offset(h), "end_revealed_oldest": true, "native_readonly_input_rejected": true}));
    h.capture(details, "history.png", cx)?;
    Ok(())
}

/// A worker scroll receipt can precede its next native paint. Require the same current, enabled
/// canvas and original focus before sending one key, then let the caller verify its real effect.
/// This waits before dispatch and never replays a key rejected by a later scene transition.
/// Sample evidence in memory, then let the original deferred input leave the native update before
/// writing files; diagnostic I/O must not extend the lifetime of an already sampled input version.
async fn canvas_key(
    h: &Harness,
    focus: &FocusHandle,
    key: &str,
    trace: &Rc<RefCell<Vec<serde_json::Value>>>,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    h.settle(cx).await?;
    let mut captured = None;
    let mut waiting = Vec::new();
    let observed = h.until(
        h.main,
        &format!("current focused history canvas for {key}"),
        |window, cx| {
            let current = h.node_current(h.main, "log", cx);
            let active = window.is_window_active();
            let focused = focus.is_focused(window);
            if !current || !active || !focused {
                // A rejected readiness sample is observation only: retain a bounded history in
                // memory and write it after the native update, without focusing or sending a key.
                let facts = serde_json::json!({
                    "node_current": current, "window_active": active,
                    "original_focus_retained": focused, "original_focus": format!("{focus:?}"),
                    "current_focus": format!("{:?}", window.focused(cx)),
                    "native_foreground": windows::OwnedWindow::for_window(window)?.foreground_snapshot(),
                    "native_canvas": native_canvas(h, window, cx),
                    "scale_factor": window.scale_factor(),
                    "scale_source": "Window::scale_factor, updated by native WM_DPICHANGED",
                    "viewport": format!("{:?}", window.viewport_size()),
                    "worker_revision": h.worker.state.lock().unwrap().views.get("git/history").map(|scene| scene.revision),
                    "host_revision": h.app.read(cx).plugin_panels["git/history"].read(cx).current_document().map(|scene| scene.revision),
                });
                if waiting.last() != Some(&facts) {
                    waiting.push(facts);
                    if waiting.len() > 12 { waiting.remove(0); }
                    h.record(&format!("history_key_{key}_readiness_wait"), serde_json::json!(waiting));
                }
                return Ok(false);
            }
            let worker_revision = h.worker.state.lock().unwrap().views["git/history"].revision;
            let host_revision = h.app.read(cx).plugin_panels["git/history"]
                .read(cx)
                .current_document()
                .map(|scene| scene.revision);
            let before = serde_json::json!({
                "node_current": current, "window_active": active, "original_focus_retained": focused,
                "current_focus": format!("{:?}", window.focused(cx)),
                "worker_revision": worker_revision, "host_revision": host_revision,
                "offset": log_offset(h), "native_canvas": native_canvas(h, window, cx),
                "trace": *trace.borrow(),
            });
            input::history_control_key(window, key, cx);
            let after = serde_json::json!({
                "current_focus": format!("{:?}", window.focused(cx)),
                "original_focus_retained": focus.is_focused(window), "offset": log_offset(h),
                "native_canvas": native_canvas(h, window, cx), "trace": *trace.borrow(),
            });
            captured = Some((before, after));
            Ok(true)
        },
        cx,
    )
    .await;
    // This FIFO observation does not poll the actor, repaint the scene or schedule another input.
    // It only waits until the original cx.defer callback preceding it has naturally left the queue.
    let (flushed, receipt) = futures::channel::oneshot::channel();
    cx.update(|cx| {
        cx.defer(move |_| {
            let _ = flushed.send(());
        })
    });
    receipt
        .await
        .context("history key deferred observation was cancelled")?;
    if !waiting.is_empty() {
        h.checkpoint(
            &format!("history_key_{key}_readiness_wait"),
            serde_json::json!(waiting),
        )?;
    }
    observed?;
    let (before, after) = captured.context("history key dispatched without evidence samples")?;
    h.checkpoint(&format!("history_key_{key}_before"), before)?;
    h.checkpoint(&format!("history_key_{key}_after_dispatch"), after)?;
    Ok(())
}

/// Observe the actual keyed entity and original sink without replacing the key or its envelope.
/// The bounded trace separates a native callback gate from deferred admission and actor handling.
fn observe_keys(
    h: &Harness,
    cx: &mut AsyncApp,
) -> anyhow::Result<Rc<RefCell<Vec<serde_json::Value>>>> {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let recorded = trace.clone();
    let worker = h.worker.clone();
    let observer = Rc::new(move |mut facts: serde_json::Value| {
        let state = worker.state.lock().unwrap();
        facts["worker_revision"] =
            serde_json::json!(state.views.get("git/history").map(|view| view.revision));
        facts["worker_processes"] = serde_json::json!(state.processes.get("git"));
        facts["at_millis"] = serde_json::json!(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );
        eprintln!("[git120-key-probe] {facts}");
        let mut trace = recorded.borrow_mut();
        trace.push(facts);
        if trace.len() > 12 {
            trace.remove(0);
        }
    });
    let installed = h.main.update(cx, |_, _, cx| {
        let view = h.app.read(cx).plugin_panels["git/history"]
            .read(cx)
            .native_ui
            .clone();
        view.is_some_and(|view| {
            view.update(cx, |view, cx| {
                view.observe_native_input("log", observer, cx)
            })
        })
    })?;
    ensure!(
        installed,
        "history has no mounted native canvas for its input observation"
    );
    Ok(trace)
}

/// Panel publications and a real Canvas entity have independent observation points.
fn native_canvas(h: &Harness, window: &Window, cx: &App) -> Option<serde_json::Value> {
    h.app.read(cx).plugin_panels["git/history"]
        .read(cx)
        .native_ui
        .as_ref()?
        .read(cx)
        .native_canvas_input_snapshot("log", window, cx)
}

/// The root projects aligned pointer events after dispatch releases its listener table. Await
/// the ordinary native frame and observed focus before typing, then inspect the next real frame.
/// Neither this readiness wait nor its diagnostics can assign focus or replace source text.
async fn readonly_input(
    h: &Harness,
    target: AnyWindowHandle,
    expected: &str,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    target.update(cx, |_, window, cx| {
        let before = input::source(window, "snapshot-1")?;
        ensure!(
            before.role() == Some(Role::Document),
            "history result advertises writable input"
        );
        ensure!(
            before.value() == Some(expected)
                && input::source(window, "snapshot-0")?.value() == Some(""),
            "oldest history did not compare the empty tree to its actual source"
        );
        input::click(window, &before, cx)?;
        let immediate = input::source(window, "snapshot-1")?;
        h.checkpoint(
            "readonly_pointer_immediate",
            serde_json::json!({
                "window_active": window.is_window_active(), "focused": immediate.focused(),
                "current_focus": format!("{:?}", window.focused(cx)),
                "value_preserved": immediate.value() == Some(expected),
                "source_bounds": format!("{:?}", before.bounds())
            }),
        )?;
        Ok::<_, anyhow::Error>(())
    })??;
    h.frame(target, cx).await?;
    let mut last_focus = None;
    h.until(target, "readonly pane received native pointer focus", |window, cx| {
        let source = input::source(window, "snapshot-1")?;
        let active = window.is_window_active();
        let focused = source.focused() == Some(true);
        let preserved = source.value() == Some(expected);
        let facts = serde_json::json!({"window_active": active, "focused": source.focused(), "current_focus": format!("{:?}", window.focused(cx)), "value_preserved": preserved});
        if last_focus.as_ref() != Some(&facts) {
            h.checkpoint("readonly_pointer_after_frame", facts.clone())?;
            last_focus = Some(facts);
        }
        ensure!(preserved, "readonly pointer changed the immutable history source");
        Ok(active && focused)
    }, cx).await?;
    target.update(cx, |_, window, cx| {
        input::text(window, "must remain readonly", cx)
    })?;
    h.frame(target, cx).await?;
    target.update(cx, |_, window, cx| {
        let after = input::source(window, "snapshot-1")?;
        let focused = after.focused() == Some(true);
        let preserved = after.value() == Some(expected);
        h.checkpoint(
            "readonly_native_input_result",
            serde_json::json!({
                "window_active": window.is_window_active(), "focused": after.focused(),
                "current_focus": format!("{:?}", window.focused(cx)), "value_preserved": preserved,
                "role": format!("{:?}", after.role())
            }),
        )?;
        ensure!(
            focused && preserved,
            "readonly pane did not retain focus/value after native input"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    Ok(())
}

/// Pointer selection may open a separate window and asynchronously replace the canvas scene.
/// Observe that exact selection before wheeling; batching both native gestures would send the
/// wheel through the pointer's old revision and could never prove independent wheel behavior.
async fn select_first_history_row(
    h: &Harness,
    position: Point<Pixels>,
    cx: &mut AsyncApp,
) -> anyhow::Result<FocusHandle> {
    let root = h.fixture.workspace.clone();
    let expected = cx
        .background_executor()
        .spawn(async move { fixture::git_text(&root, &["rev-parse", "HEAD"]) })
        .await?;
    let previous_revision = h.worker.state.lock().unwrap().views["git/history"].revision;
    let focus = h.main.update(cx, |_, window, cx| {
        input::point_click(window, position, cx)?;
        window
            .focused(cx)
            .context("history pointer did not focus a native target")
    })??;
    let mut last_selection = None;
    h.until(
        h.main,
        "history pointer selection and current scene",
        |window, cx| {
            let (revision, selected_oid) = {
                let state = h.worker.state.lock().unwrap();
                (
                    state.views.get("git/history").map(|view| view.revision),
                    state
                        .views
                        .get("git/commit-details")
                        .and_then(|view| view.active_node("commit-identity"))
                        .and_then(|node| match &node.kind {
                            Kind::Text { text } => Some(text.clone()),
                            _ => None,
                        }),
                )
            };
            let advanced = revision.is_some_and(|revision| revision > previous_revision);
            let selected = history_row_selected(h, "Native UI commit 中文");
            let exact_oid = selected_oid.as_deref() == Some(expected.trim());
            let facts = serde_json::json!({
                "revision_advanced": advanced, "selected_title": selected,
                "expected_oid": expected.trim(), "actual_oid": selected_oid,
                "exact_oid": exact_oid, "window_active": window.is_window_active(),
                "captured_focus_retained": focus.is_focused(window),
                "captured_focus": format!("{focus:?}"),
                "current_focus": format!("{:?}", window.focused(cx))
            });
            // Keep the latest counter for a timeout receipt without writing/logging every frame.
            // Detailed diagnostics persist only when an actual gate/focus observation changes.
            h.record(
                "history_pointer_latest_revision",
                serde_json::json!(revision),
            );
            if last_selection.as_ref() != Some(&facts) {
                h.checkpoint("history_pointer_selection_wait", facts.clone())?;
                last_selection = Some(facts);
            }
            // The new detail window may currently own OS activation. Verify original main-window
            // focus after the ordinary activation below, independently from the selection receipt.
            Ok(advanced && selected && exact_oid)
        },
        cx,
    )
    .await?;
    let details = work_window(h, "commit-details", cx).await?;
    h.frame(details, cx).await?;
    h.settle(cx).await?;
    // Opening commit details can activate that window. Return through ordinary native activation;
    // the original canvas must retain its keyboard focus, without forcing a different FocusHandle.
    h.main.update(cx, |_, window, _| window.activate_window())?;
    let mut last_focus = None;
    h.until(
        h.main,
        "native history canvas active before wheel",
        |window, cx| {
            let active = window.is_window_active();
            let retained = focus.is_focused(window);
            let facts = serde_json::json!({
                "window_active": active, "captured_focus_retained": retained,
                "captured_focus": format!("{focus:?}"),
                "current_focus": format!("{:?}", window.focused(cx))
            });
            if last_focus.as_ref() != Some(&facts) {
                h.checkpoint("history_pointer_focus_wait", facts.clone())?;
                last_focus = Some(facts);
            }
            Ok(active && retained)
        },
        cx,
    )
    .await?;
    h.settle(cx).await?;
    h.record(
        "native_history_pointer_selection",
        serde_json::json!({
            "commit": expected.trim(), "current_scene_selected": true,
            "native_focus_retained": true, "separate_from_wheel": true
        }),
    );
    Ok(focus)
}

/// The public draw list exposes a selected row. Offset changes alone could come from another
/// queued scroll callback and must not authorize a subsequent pointer or keyboard gesture.
fn history_row_selected(h: &Harness, title: &str) -> bool {
    let state = h.worker.state.lock().unwrap();
    let Some(Kind::Canvas(canvas)) = state
        .views
        .get("git/history")
        .and_then(|view| view.active_node("log"))
        .map(|node| &node.kind)
    else {
        return false;
    };
    let row = canvas.paint.iter().find_map(|paint| match paint {
        Paint::Text { y, text, .. } if text == title => Some(*y - 5.),
        _ => None,
    });
    row.is_some_and(|y| canvas.paint.iter().any(|paint| {
        matches!(paint, Paint::Fill { rect, .. } if (rect.y - y).abs() < 0.01 && (rect.h - 28.).abs() < 0.01)
    }))
}

fn log_offset(h: &Harness) -> f32 {
    h.worker
        .state
        .lock()
        .unwrap()
        .views
        .get("git/history")
        .and_then(|view| view.active_node("log"))
        .and_then(|node| match &node.kind {
            Kind::Canvas(canvas) => canvas.scroll.as_ref().map(|range| range.offset),
            _ => None,
        })
        .unwrap_or_default()
}
