//! Real native branch trees expose local/remote groups and filter history without checking out refs.
use super::*;
use plugin_runtime::plugin_protocol::Paint;

/// No synthetic tree event or placeholder bypass participates in this acceptance. The initial
/// lazy root must load through the same production view that a user sees after installation.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let root = h.fixture.workspace.clone();
    let original_head = cx
        .background_executor()
        .spawn(async move { fixture::git_text(&root, &["rev-parse", "HEAD"]) })
        .await?;
    let local = h.case.label("本地", "Local");
    let remote = h.case.label("远程", "Remote");
    h.until(
        h.main,
        "native local and remote branch roots loaded",
        |window, _| {
            Ok(input::tree_item(window, "branches", local).is_ok()
                && input::tree_item(window, "branches", remote).is_ok())
        },
        cx,
    )
    .await?;

    h.expand(h.main, "branches", local, cx).await?;
    h.until(
        h.main,
        "native local main and incoming branch leaves",
        |window, _| {
            Ok(input::tree_item(window, "branches", "main").is_ok()
                && input::tree_item(window, "branches", "incoming").is_ok())
        },
        cx,
    )
    .await?;
    h.main.update(cx, |_, window, _| {
        h.record("native_local_branch_group", serde_json::json!({
            "label": local,
            "main": format!("{:?}", input::tree_item(window, "branches", "main")?.bounds()),
            "incoming": format!("{:?}", input::tree_item(window, "branches", "incoming")?.bounds())
        }));
        Ok::<_, anyhow::Error>(())
    })??;
    h.capture(h.main, "branches-local.png", cx)?;

    // Four complete rows fit the default branch viewport. Collapse through native keyboard input
    // so the remote's nested leaf remains reachable without mutating Base's scroll/selection state.
    collapse(h, local, cx).await?;
    h.expand(h.main, "branches", remote, cx).await?;
    h.expand(h.main, "branches", "fixture", cx).await?;
    h.until(
        h.main,
        "native fixture remote main leaf",
        |window, _| Ok(input::tree_item(window, "branches", "main").is_ok()),
        cx,
    )
    .await?;
    h.main.update(cx, |_, window, _| {
        h.record(
            "native_remote_branch_group",
            serde_json::json!({
                "label": remote,
                "remote": "fixture",
                "main": format!("{:?}", input::tree_item(window, "branches", "main")?.bounds())
            }),
        );
        Ok::<_, anyhow::Error>(())
    })??;
    h.capture(h.main, "branches-remote.png", cx)?;

    collapse(h, remote, cx).await?;
    h.expand(h.main, "branches", local, cx).await?;
    h.until(
        h.main,
        "incoming branch double click",
        |window, cx| {
            let Ok(item) = input::tree_item(window, "branches", "incoming") else {
                return Ok(false);
            };
            input::double_click(window, &item, cx)?;
            Ok(true)
        },
        cx,
    )
    .await?;
    h.settle(cx).await?;
    h.until(h.main, "incoming selection filtered native history", |_, _| {
        Ok(history_has(h, "Incoming native changes") && !history_has(h, "Review history 35"))
    }, cx).await?;
    unchanged_checkout(h, &original_head, "branch filtering retained checkout", cx).await?;
    h.record(
        "native_branch_filter_without_checkout",
        serde_json::json!({
            "selected_ref": "refs/heads/incoming", "checkout": "main",
            "head": original_head.trim(), "incoming_history_present": true,
            "main_history_absent": true
        }),
    );
    h.click_button(h.main, "all", cx).await?;
    h.until(
        h.main,
        "All refs restored the main history",
        |_, _| Ok(history_has(h, "Review history 35")),
        cx,
    )
    .await?;
    unchanged_checkout(h, &original_head, "All refs retained checkout", cx).await?;
    h.record("native_all_refs_restored", true.into());
    Ok(())
}

/// Collapsing a real parent keeps the short production viewport usable for the next group.
async fn collapse(h: &Harness, label: &str, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.settle(cx).await?;
    // A completed Git refresh can temporarily replace lazy tree pages with their loading row.
    // Wait for paint and dispatch once in that same update; idle buttons are not a tree receipt.
    h.until(
        h.main,
        label,
        |window, cx| {
            let Ok(row) = input::tree_item(window, "branches", label) else {
                return Ok(false);
            };
            if row.disabled() == Some(true) {
                return Ok(false);
            }
            if row.expanded() == Some(true) {
                input::double_click(window, &row, cx)?;
            }
            Ok(true)
        },
        cx,
    )
    .await?;
    h.settle(cx).await?;
    h.until(
        h.main,
        label,
        |window, cx| {
            let Ok(row) = input::tree_item(window, "branches", label) else {
                return Ok(false);
            };
            if row.disabled() == Some(true) {
                return Ok(false);
            }
            if row.expanded() == Some(true) {
                editor_ui::testing::press_key(window, "left", cx);
            }
            Ok(true)
        },
        cx,
    )
    .await?;
    h.until(
        h.main,
        label,
        |window, _| {
            Ok(input::tree_item(window, "branches", label)
                .is_ok_and(|row| row.expanded() != Some(true)))
        },
        cx,
    )
    .await
}

/// The public drawing payload is the actual history publication, not a guessed process-idle receipt.
fn history_has(h: &Harness, title: &str) -> bool {
    let state = h.worker.state.lock().unwrap();
    state
        .views
        .get("git/history")
        .and_then(|view| view.active_node("log"))
        .is_some_and(|node| {
            matches!(&node.kind, Kind::Canvas(canvas) if canvas.paint.iter().any(|paint| {
                matches!(paint, Paint::Text { text, .. } if text == title)
            }))
        })
}

/// A branch selection's completed view must leave the real symbolic ref and OID unchanged.
async fn unchanged_checkout(
    h: &Harness,
    original_head: &str,
    label: &str,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let expected = original_head.trim().to_owned();
    h.until_repository(
        label,
        move |root| {
            ensure!(
                fixture::git_text(root, &["rev-parse", "HEAD"])?.trim() == expected,
                "native branch filter changed HEAD"
            );
            ensure!(
                fixture::git_text(root, &["symbolic-ref", "--short", "HEAD"])?.trim() == "main",
                "native branch filter checked out a different branch"
            );
            Ok(true)
        },
        cx,
    )
    .await
}
