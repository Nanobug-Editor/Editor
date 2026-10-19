//! Real idle timer checks retain native review state without replaying a user gesture.
use super::*;
use gpui_base::test_support::{ElementSnapshot, snapshots};
use gpui_kit::Role;
use plugin_runtime::plugin_protocol::ui::Kind;
use std::collections::BTreeMap;

/// Two spans cross the slower production timer; a passing run must observe an actual background read.
const IDLE_SPAN_SECS: u64 = 35;

/// This narrower native workflow has its own receipt and cannot satisfy full workbench acceptance.
#[test]
#[ignore = "requires Windows desktop/D3D11, candidate/output and NANOBUG_GIT_NATIVE_IDLE_RETENTION"]
fn git_native_idle_checks_preserve_review_state() {
    assert!(std::env::var_os("NANOBUG_GIT_NATIVE_IDLE_RETENTION").is_some());
    for flag in [
        "NANOBUG_GIT_NATIVE_HISTORY_ONLY",
        "NANOBUG_GIT_NATIVE_HISTORY_PHASES",
    ] {
        assert!(
            std::env::var_os(flag).is_none(),
            "idle workflow refuses {flag}"
        );
    }
    matrix::run(Case::SMOKE).unwrap();
}

/// Keep only observations of original native controls; text and tree state remain in production owners.
struct Retained {
    /// The real focused textarea handle captured after the user's text-selection keys.
    focus: FocusHandle,
    /// A visible selected leaf's position relative to its tree proves nonzero scroll is retained.
    leaf_top: Pixels,
}

/// Keep the original native draft and menu through two idle spans crossing the production timer.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.activate(cx).await?;
    h.expand(h.main, "branches", h.case.label("本地", "Local"), cx)
        .await?;
    h.expand(h.main, "staging", h.case.label("未暂存", "Unstaged"), cx)
        .await?;
    h.expand(h.main, "staging", "src", cx).await?;
    h.select_file(h.main, "staging", "a-review.txt", cx).await?;
    let leaf_top = scroll_review(h, cx).await?;
    let focus = draft_with_selection(h, cx).await?;
    let retained = Retained { focus, leaf_top };
    h.capture(h.main, "idle-state-before.png", cx)?;
    let field_observed = observe_idle(h, &retained, "draft-kept中文", None, cx).await?;
    h.capture(h.main, "idle-state-after.png", cx)?;

    // Replacing the originally selected first character proves its range survived idle.
    // This is one new edit after the observation, never a retry of a rejected gesture.
    h.main
        .update(cx, |_, window, cx| input::text(window, "D", cx))?;
    h.until(
        h.main,
        "idle-retained textarea selection accepted one edit",
        |window, _| {
            Ok(
                input::field(window, "commit-message")?.value() == Some("Draft-kept中文")
                    && worker_draft(h, "Draft-kept中文"),
            )
        },
        cx,
    )
    .await?;
    open_menu(h, cx).await?;
    let (menu_focus, labels) = h.main.update(cx, |_, window, cx| {
        let rows = menu_rows(window);
        ensure!(!rows.is_empty(), "advanced menu was not painted");
        ensure!(rows.iter().all(|row| row.disabled() != Some(true)));
        Ok::<_, anyhow::Error>((
            window
                .focused(cx)
                .context("native menu did not own focus")?,
            menu_labels(&rows),
        ))
    })??;
    h.capture(h.main, "idle-menu-before.png", cx)?;
    let menu_retained = Retained {
        focus: menu_focus,
        leaf_top,
    };
    let menu_observed =
        observe_idle(h, &menu_retained, "Draft-kept中文", Some(&labels), cx).await?;
    ensure!(
        field_observed || menu_observed,
        "idle validation never crossed a real background Git observation"
    );
    h.capture(h.main, "idle-menu-after.png", cx)?;
    h.main
        .update(cx, |_, window, cx| input::key(window, "escape", cx))?;
    h.until(
        h.main,
        "idle-retained menu accepted one Escape",
        |window, _| {
            Ok(menu_rows(window).is_empty()
                && h.worker.state.lock().unwrap().views["git/history"]
                    .menu
                    .is_none())
        },
        cx,
    )
    .await?;
    h.record(
        "idle_review_state_complete",
        serde_json::json!({
            "idle_spans": 2, "minimum_seconds_per_span": IDLE_SPAN_SECS,
            "observed_background_read": field_observed || menu_observed,
            "no_refresh_or_disclosure_replay": true,
            "draft_selection_edit": "Draft-kept中文",
            "menu_dismissed_once": true,
        }),
    );
    Ok(())
}

/// Send one real wheel and retain the selected leaf in view, independently of the fixed commit form.
async fn scroll_review(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<Pixels> {
    let before = h.main.update(cx, |_, window, cx| {
        let tree = input::tree(window, "staging")?;
        let leaf = input::tree_item(window, "staging", "a-review.txt")?;
        let before = leaf.bounds().top() - tree.bounds().top();
        input::wheel(window, tree.bounds().center(), -40., cx);
        Ok::<_, anyhow::Error>(before)
    })??;
    h.until(
        h.main,
        "one wheel moved the selected review leaf",
        |window, _| {
            let tree = input::tree(window, "staging")?;
            let leaf = input::tree_item(window, "staging", "a-review.txt")?;
            Ok(leaf.selected() == Some(true)
                && before - (leaf.bounds().top() - tree.bounds().top()) > px(1.))
        },
        cx,
    )
    .await?;
    h.main.update(cx, |_, window, _| {
        Ok(input::tree_item(window, "staging", "a-review.txt")?
            .bounds()
            .top()
            - input::tree(window, "staging")?.bounds().top())
    })?
}

/// A pointer and native typing create the real draft; Home/ShiftRight select its first character once.
async fn draft_with_selection(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<FocusHandle> {
    h.until(
        h.main,
        "current native commit textarea",
        |window, cx| {
            let field = input::field(window, "commit-message")?;
            if field.disabled() == Some(true) || !h.node_current(h.main, "commit-message", cx) {
                return Ok(false);
            }
            input::click(window, &field, cx)?;
            Ok(true)
        },
        cx,
    )
    .await?;
    h.until(
        h.main,
        "native commit textarea pointer focus",
        |window, _| Ok(input::field(window, "commit-message")?.focused() == Some(true)),
        cx,
    )
    .await?;
    h.main.update(cx, |_, window, cx| {
        input::text(window, "draft-kept中文", cx)
    })?;
    h.until(
        h.main,
        "native commit draft acknowledged by its real guest",
        |window, _| {
            Ok(
                input::field(window, "commit-message")?.value() == Some("draft-kept中文")
                    && worker_draft(h, "draft-kept中文"),
            )
        },
        cx,
    )
    .await?;
    h.main.update(cx, |_, window, cx| {
        input::key(window, "home", cx);
        input::key(window, "shift-right", cx);
        window
            .focused(cx)
            .context("commit textarea lost original focus")
    })?
}

/// Only the public guest textarea value acknowledges delivery; a local draft alone cannot pass.
fn worker_draft(h: &Harness, expected: &str) -> bool {
    h.worker.state.lock().unwrap().views["git/staging"]
        .active_node("commit-message")
        .is_some_and(|node| matches!(&node.kind, Kind::Textarea(input) if input.value == expected))
}

/// Click the current real trigger once. A menu legitimately owns input, so do not use settle's refresh gate.
async fn open_menu(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.until(
        h.main,
        "current native advanced-menu trigger",
        |window, cx| {
            let button = input::button(window, "advanced-open")?;
            if button.disabled() == Some(true) || !h.node_current(h.main, "advanced-open", cx) {
                return Ok(false);
            }
            input::click(window, &button, cx)?;
            Ok(true)
        },
        cx,
    )
    .await?;
    h.until(
        h.main,
        "native advanced menu opened once",
        |window, _| Ok(!menu_rows(window).is_empty()),
        cx,
    )
    .await
}

/// Read native menu items rather than assuming a guest declaration proves a usable popup.
fn menu_rows(window: &Window) -> Vec<ElementSnapshot> {
    snapshots(window)
        .into_iter()
        .filter(|node| node.visible() && node.role() == Some(Role::MenuItem))
        .collect()
}

/// Labels identify this menu's actual published items without relying on private PopupMenu state.
fn menu_labels(rows: &[ElementSnapshot]) -> Vec<String> {
    let mut labels = rows
        .iter()
        .filter_map(|row| row.label().map(str::to_owned))
        .collect::<Vec<_>>();
    labels.sort();
    labels
}

/// Run native frames without input; return whether the real Worker ran a background Git process.
async fn observe_idle(
    h: &Harness,
    retained: &Retained,
    draft: &str,
    menu: Option<&[String]>,
    cx: &mut AsyncApp,
) -> anyhow::Result<bool> {
    let expected_revisions = h
        .worker
        .state
        .lock()
        .unwrap()
        .views
        .iter()
        .map(|(panel, scene)| (panel.clone(), scene.revision))
        .collect::<BTreeMap<_, _>>();
    h.record(
        if menu.is_some() {
            "idle_menu_start_revisions"
        } else {
            "idle_field_start_revisions"
        },
        serde_json::json!(expected_revisions),
    );
    let started = Instant::now();
    let mut frames = 0;
    let mut observed_read = false;
    while started.elapsed() < Duration::from_secs(IDLE_SPAN_SECS) {
        h.frame(h.main, cx).await?;
        observed_read |= h
            .worker
            .state
            .lock()
            .unwrap()
            .processes
            .get("git")
            .is_some_and(|count| *count > 0);
        h.main.update(cx, |_, window, cx| {
            assert_native_state(h, retained, draft, menu, &expected_revisions, window, cx)
        })??;
        frames += 1;
    }
    h.checkpoint(
        if menu.is_some() {
            "idle_menu_span"
        } else {
            "idle_field_span"
        },
        serde_json::json!({
            "elapsed_millis": started.elapsed().as_millis(), "native_frames": frames,
            "minimum_seconds": IDLE_SPAN_SECS, "input_or_refresh_during_span": false,
            "observed_background_read": observed_read,
        }),
    )?;
    Ok(observed_read)
}

/// User-visible continuity is checked on every paint, including temporary disabling during an unchanged check.
fn assert_native_state(
    h: &Harness,
    retained: &Retained,
    draft: &str,
    menu: Option<&[String]>,
    expected_revisions: &BTreeMap<String, u64>,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<()> {
    let field = input::field(window, "commit-message")?;
    let leaf = input::tree_item(window, "staging", "a-review.txt").ok();
    let directory = input::tree_item(window, "staging", "src").ok();
    let local = input::tree_item(window, "branches", h.case.label("本地", "Local")).ok();
    let tree = input::tree(window, "staging")?;
    let commit = input::button(window, "commit")?;
    let leaf_top = leaf
        .as_ref()
        .map(|leaf| leaf.bounds().top() - tree.bounds().top());
    let rows = menu_rows(window);
    let (processes, revisions, commit_admitted, staging_admitted, history_admitted) = {
        let state = h.worker.state.lock().unwrap();
        (
            state.processes.get("git").copied(),
            state
                .views
                .iter()
                .map(|(panel, scene)| (panel.clone(), scene.revision))
                .collect::<BTreeMap<_, _>>(),
            state.views["git/staging"].active_node("commit").is_some(),
            state.views["git/staging"].active_node("staging").is_some(),
            if menu.is_some() {
                state.views["git/history"]
                    .menu
                    .as_ref()
                    .is_some_and(|menu| menu.items.iter().all(|item| !item.disabled))
            } else {
                state.views["git/history"].active_node("branches").is_some()
            },
        )
    };
    h.record(
        "idle_last_native_state",
        serde_json::json!({
            "field_value": field.value(), "field_disabled": field.disabled(),
            "field_focused": field.focused(), "window_active": window.is_window_active(),
            "original_focus": format!("{:?}", retained.focus),
            "actual_focus": format!("{:?}", window.focused(cx)),
            "selected_review": leaf.as_ref().and_then(|leaf| leaf.selected()),
            "review_leaf_top": leaf_top.map(f32::from),
            "expected_leaf_top": f32::from(retained.leaf_top),
            "src_expanded": directory.as_ref().and_then(|row| row.expanded()),
            "local_expanded": local.as_ref().and_then(|row| row.expanded()),
            "worker_processes": processes, "scene_revisions": revisions,
            "expected_scene_revisions": expected_revisions,
            "native_commit_disabled": commit.disabled(), "native_tree_disabled": tree.disabled(),
            "worker_commit_admitted": commit_admitted, "worker_staging_admitted": staging_admitted,
            "worker_history_or_menu_admitted": history_admitted,
            "menu_labels": menu_labels(&rows),
            "menu_disabled": rows.iter().map(|row| row.disabled()).collect::<Vec<_>>(),
        }),
    );
    ensure!(
        field.value() == Some(draft),
        "idle check replaced the native commit draft"
    );
    ensure!(
        field.disabled() != Some(true),
        "idle check disabled the native commit textarea"
    );
    // Native disabled metadata may be absent. The actual public Worker scene is the same
    // authority used to accept these visible controls, so absence cannot hide busy gating.
    ensure!(
        commit_admitted && staging_admitted && history_admitted,
        "idle check disabled current commit/tree/menu admission"
    );
    ensure!(
        &revisions == expected_revisions,
        "idle check replaced unchanged native scene authority"
    );
    ensure!(
        window.is_window_active(),
        "idle observation lost its own active window"
    );
    ensure!(
        window.focused(cx).as_ref() == Some(&retained.focus),
        "idle check replaced original native focus"
    );
    ensure!(
        leaf.as_ref()
            .is_some_and(|leaf| leaf.selected() == Some(true)),
        "idle check lost the selected native review file"
    );
    ensure!(
        directory
            .as_ref()
            .is_some_and(|row| row.expanded() == Some(true))
            && local
                .as_ref()
                .is_some_and(|row| row.expanded() == Some(true)),
        "idle check collapsed an expanded native tree"
    );
    ensure!(
        leaf_top.is_some_and(|top| (top - retained.leaf_top).abs() <= px(1.)),
        "idle check reset native tree scroll"
    );
    ensure!(
        input::tree_item(window, "branches", "main").is_ok(),
        "idle check replaced loaded branches with a placeholder"
    );
    if let Some(labels) = menu {
        ensure!(
            !rows.is_empty() && menu_labels(&rows) == labels,
            "idle check dismissed or replaced the open native menu"
        );
        ensure!(
            rows.iter().all(|row| row.disabled() != Some(true)),
            "idle check disabled the open native menu"
        );
    } else {
        ensure!(
            field.focused() == Some(true),
            "idle check blurred the native commit textarea"
        );
    }
    Ok(())
}
