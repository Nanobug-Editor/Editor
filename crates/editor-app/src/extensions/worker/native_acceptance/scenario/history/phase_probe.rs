//! Independent native history operations sample actual timer phases without retrying failed input.
use super::*;

/// Every pair navigates and opens one exact immutable commit through the unchanged native gates.
/// The next pair starts only after the prior identity and ordinary idle receipt, never after failure.
pub(super) async fn exercise(
    h: &Harness,
    focus: &FocusHandle,
    details: AnyWindowHandle,
    oldest: &str,
    trace: &Rc<RefCell<Vec<serde_json::Value>>>,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    const ROUNDS: usize = 32;
    let root = h.fixture.workspace.clone();
    let latest = cx
        .background_executor()
        .spawn(async move { fixture::git_text(&root, &["rev-parse", "HEAD"]) })
        .await?;
    let mut completed = 0;
    for round in 0..ROUNDS {
        for (key, title, expected) in [
            ("home", "Native UI commit 中文", latest.trim()),
            ("end", "Initial native review", oldest),
        ] {
            // The diagnostic must not repeatedly synthesize the platform's activation/Alt sequence.
            // Observe the original main-window activation and focus without changing either one.
            let mut last_wait = None;
            h.until(
                h.main,
                "phase probe retained original history focus",
                |window, cx| {
                    let active = window.is_window_active();
                    let focused = focus.is_focused(window);
                    if !active || !focused {
                        let facts = serde_json::json!({
                            "round": round, "navigate": key, "window_active": active,
                            "original_focus_retained": focused, "original_focus": format!("{focus:?}"),
                            "current_focus": format!("{:?}", window.focused(cx)),
                            "native_foreground": windows::OwnedWindow::for_window(window)?.foreground_snapshot(),
                            "native_canvas": native_canvas(h, window, cx),
                            "scale_factor": window.scale_factor(), "viewport": format!("{:?}", window.viewport_size()),
                        });
                        if last_wait.as_ref() != Some(&facts) {
                            h.record("history_phase_readiness_wait", facts.clone());
                            last_wait = Some(facts);
                        }
                    }
                    Ok(active && focused)
                },
                cx,
            )
            .await?;
            let facts = serde_json::json!({
                "stage": "phase_begin", "round": round, "navigate": key,
                "expected_oid": expected, "completed_operations": completed,
                "at_millis": millis(),
            });
            eprintln!("[git120-key-probe] {facts}");
            h.checkpoint("history_keyboard_phase", facts)?;
            canvas_key(h, focus, key, trace, cx).await?;
            h.until(
                h.main,
                &format!("phase {round} {key} selected its exact history row"),
                |_, _| Ok(history_row_selected(h, title)),
                cx,
            )
            .await?;
            canvas_key(h, focus, "enter", trace, cx).await?;
            h.until(
                details,
                &format!("phase {round} {key} Enter opened its exact commit"),
                |_, _| {
                    h.record("history_key_trace", serde_json::json!(*trace.borrow()));
                    let state = h.worker.state.lock().unwrap();
                    Ok(state
                        .views
                        .get("git/commit-details")
                        .and_then(|view| view.active_node("commit-identity"))
                        .is_some_and(
                            |node| matches!(&node.kind, Kind::Text { text } if text == expected),
                        ))
                },
                cx,
            )
            .await?;
            h.settle(cx).await?;
            completed += 1;
            let facts = serde_json::json!({
                "stage": "phase_completed", "round": round, "navigate": key,
                "exact_oid": expected, "completed_operations": completed,
                "single_navigation": true, "single_enter": true, "at_millis": millis(),
            });
            eprintln!("[git120-key-probe] {facts}");
            h.checkpoint("history_keyboard_phase_completed", facts)?;
        }
    }
    h.record(
        "history_keyboard_phases",
        serde_json::json!({
            "rounds": ROUNDS, "completed_operations": completed,
            "navigation_keys": completed, "enter_keys": completed,
            "latest_oid": latest.trim(), "oldest_oid": oldest,
            "diagnostic_only": true, "failed_input_retried": false,
        }),
    );
    Ok(())
}

/// Wall-clock stamps correlate the native callback, deferred sink and unchanged actor admission.
fn millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
}
