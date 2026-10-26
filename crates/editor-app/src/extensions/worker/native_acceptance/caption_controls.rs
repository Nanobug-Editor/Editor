//! Loaded immutable commit comparisons exercise actual OS caption gestures in their own native workflow.
use super::super::native_close_tests::caption_mouse::{self, CaptionButton};
use super::*;
use gpui_kit::Role;
use plugin_runtime::plugin_protocol::ui::Kind;

/// The parent retains cursor restoration even if a blocked child must be terminated externally.
#[test]
#[ignore = "requires Windows desktop/D3D11, candidate/output and NANOBUG_GIT_NATIVE_CAPTION_CONTROLS"]
fn git_native_loaded_commit_caption_controls() {
    assert!(requested());
    for flag in [
        "NANOBUG_GIT_NATIVE_IDLE_RETENTION",
        "NANOBUG_GIT_NATIVE_HISTORY_ONLY",
        "NANOBUG_GIT_NATIVE_HISTORY_PHASES",
    ] {
        assert!(
            std::env::var_os(flag).is_none(),
            "caption workflow refuses {flag}"
        );
    }
    let _cursor = caption_mouse::CursorRestore::capture().unwrap();
    let light_scaled = Case::ALL
        .into_iter()
        .find(|case| case.name == "light-en-150")
        .expect("the caption matrix includes light English at 150 percent");
    // These two caption-only cases retain separate private children and 180-second bounds;
    // their receipts do not stand in for either complete workbench or the eight-case matrix.
    for case in [Case::SMOKE, light_scaled] {
        matrix::run(case).unwrap();
        eprintln!("loaded caption case passed: {}", case.name);
    }
}

/// This flag selects only a separately named diagnostic receipt, never complete workbench acceptance.
pub(super) fn requested() -> bool {
    std::env::var_os("NANOBUG_GIT_NATIVE_CAPTION_CONTROLS").is_some()
}

/// A new real commit changes an existing file, so both actual parent/result panes contain distinct text.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let root = h.fixture.workspace.clone();
    let (oid, left, right) = cx
        .background_executor()
        .spawn(async move {
            let left =
                fixture::git_text(&root, &["show", &format!("HEAD:{}", fixture::REVIEW_PATH)])?;
            fixture::git(&root, &["add", "--", fixture::REVIEW_PATH])?;
            fixture::git(&root, &["commit", "-m", "Caption loaded native review"])?;
            let oid = fixture::git_text(&root, &["rev-parse", "HEAD"])?;
            let right =
                fixture::git_text(&root, &["show", &format!("HEAD:{}", fixture::REVIEW_PATH)])?;
            ensure!(
                !left.is_empty() && !right.is_empty() && left != right,
                "fixture requires two nonempty changed sources"
            );
            Ok::<_, anyhow::Error>((oid.trim().to_owned(), left, right))
        })
        .await?;
    h.activate(cx).await?;
    // The branch tree and canvas share this actual dock content frame. Hit the first loaded
    // commit row through ordinary native pointer dispatch rather than injecting a guest selection.
    h.main.update(cx, |_, window, cx| {
        let tree = input::tree(window, "branches")?;
        let branch_bounds = tree.bounds();
        let body = gpui_base::test_support::snapshots(window).into_iter().filter(|node| {
            node.visible() && tree.path().starts_with(node.path())
                && matches!(node.path().last(), Some(gpui_kit::ElementId::Name(name)) if name.as_ref() == "local-dock-content-frame")
        }).max_by_key(|node| node.path().len()).context("history dock body not painted")?.bounds();
        ensure!(branch_bounds.size.height >= px(56.) && body.right() - branch_bounds.right() > px(64.));
        input::point_click(window, point((branch_bounds.right() + body.right()) / 2., branch_bounds.top() + px(38.)), cx)
    })??;
    h.until(
        h.main,
        "exact loaded caption commit opened",
        |_, cx| {
            let state = h.worker.state.lock().unwrap();
            Ok(h.app
                .read(cx)
                .plugin_windows
                .contains_key("git/commit-details")
                && state
                    .views
                    .get("git/commit-details")
                    .and_then(|view| view.active_node("commit-identity"))
                    .is_some_and(|node| matches!(&node.kind, Kind::Text { text } if text == &oid)))
        },
        cx,
    )
    .await?;
    let details =
        cx.update(|cx| AnyWindowHandle::from(h.app.read(cx).plugin_windows["git/commit-details"]));
    h.dpi(details, cx).await?;
    h.expand(details, "files", "src", cx).await?;
    h.select_file(details, "files", "a-review.txt", cx).await?;
    h.until(
        details,
        "both immutable commit panes fully loaded",
        |window, _| {
            Ok([
                ("snapshot-0", left.as_str()),
                ("snapshot-1", right.as_str()),
            ]
            .into_iter()
            .all(|(id, text)| {
                input::source(window, id).is_ok_and(|node| {
                    node.role() == Some(Role::Document) && node.value() == Some(text)
                })
            }))
        },
        cx,
    )
    .await?;
    let before = source_state(h, &oid)?;
    h.capture(details, "caption-loaded-before.png", cx)?;
    h.checkpoint("caption_loaded_sources", before.clone())?;
    h.checkpoint("caption_attempt", "minimize".into())?;
    let minimized = caption_mouse::click(details, CaptionButton::Minimize, cx).await?;
    ensure!(
        minimized.alive && minimized.iconic,
        "loaded details did not minimize"
    );
    h.checkpoint("caption_minimize", state(minimized))?;
    let restored = caption_mouse::restore_minimized(details, cx).await?;
    ensure!(restored.alive && !restored.iconic && !restored.zoomed);
    h.checkpoint("caption_restore_minimized", state(restored))?;
    verify_sources(h, details, &oid, &left, &right, &before, cx).await?;
    h.checkpoint("caption_attempt", "maximize".into())?;
    let maximized = caption_mouse::click(details, CaptionButton::Maximize, cx).await?;
    ensure!(
        maximized.alive && maximized.zoomed,
        "loaded details did not maximize"
    );
    h.checkpoint("caption_maximize", state(maximized))?;
    verify_sources(h, details, &oid, &left, &right, &before, cx).await?;
    let restored = caption_mouse::click(details, CaptionButton::Restore, cx).await?;
    ensure!(restored.alive && !restored.iconic && !restored.zoomed);
    h.checkpoint("caption_restore_maximized", state(restored))?;
    verify_sources(h, details, &oid, &left, &right, &before, cx).await?;
    h.capture(details, "caption-loaded-after-restore.png", cx)?;
    let original =
        details.update(cx, |_, window, _| windows::OwnedWindow::for_window(window))??;
    h.checkpoint("caption_attempt", "close".into())?;
    let closed = caption_mouse::click(details, CaptionButton::Close, cx).await?;
    ensure!(
        !closed.alive && !original.is_alive(),
        "loaded caption close retained its original HWND"
    );
    h.until(
        h.main,
        "only loaded details retired",
        |_, cx| {
            Ok(!h
                .app
                .read(cx)
                .plugin_windows
                .contains_key("git/commit-details")
                && !original.is_alive())
        },
        cx,
    )
    .await?;
    h.checkpoint("caption_close", state(closed))?;
    refresh_after_close(h, cx).await?;
    h.record(
        "caption_controls_complete",
        serde_json::json!({
            "commit": oid, "path": fixture::REVIEW_PATH,
            "actual_os_caption_mouse": true, "caption_clicks": 4,
            "minimized_restore_method": "own HWND ShowWindowAsync(SW_RESTORE)",
            "same_sources_after_each_restore": true, "main_refresh_after_close": true,
            "original_details_hwnd_retired": true, "diagnostic_only": true,
        }),
    );
    Ok(())
}

/// An unchanged automatic sample is silent in 0.1.8. Only this explicit refresh can publish a
/// new busy scene here: closure/visibility receipts do not queue reads, and activation settles first.
async fn refresh_after_close(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.main.update(cx, |_, window, _| window.activate_window())?;
    h.until(
        h.main,
        "main window active before its new refresh",
        |window, _| Ok(window.is_window_active()),
        cx,
    )
    .await?;
    h.settle(cx).await?;
    let mut before = None;
    h.until(
        h.main,
        "one current post-close refresh click",
        |window, cx| {
            let button = input::button(window, "refresh")?;
            if !window.is_window_active()
                || !h.node_current(h.main, "refresh", cx)
                || button.disabled() == Some(true)
                || button.focused().is_none()
            {
                return Ok(false);
            }
            before = Some(h.worker.state.lock().unwrap().views["git/history"].revision);
            // The predicate succeeds after its only gesture. Later waits only observe publications.
            input::click(window, &button, cx)?;
            Ok(true)
        },
        cx,
    )
    .await?;
    let before = before.context("post-close refresh was not dispatched")?;
    h.record(
        "caption_post_close_refresh_before",
        serde_json::json!({"revision": before, "main_active": true, "single_click": true}),
    );
    let mut started = None;
    h.until(
        h.main,
        "explicit post-close refresh published busy controls",
        |_, _| {
            let state = h.worker.state.lock().unwrap();
            let view = state
                .views
                .get("git/history")
                .context("history scene missing")?;
            let mut disabled = false;
            view.root.visit(&mut |node| {
                if node.id == "refresh" {
                    disabled = node.disabled;
                }
            });
            if view.revision > before
                && disabled
                && state.processes.get("git").copied().unwrap_or(0) > 0
            {
                started = Some(view.revision);
                Ok(true)
            } else {
                Ok(false)
            }
        },
        cx,
    )
    .await?;
    let started = started.context("explicit refresh never published its busy scene")?;
    h.settle(cx).await?;
    let mut finished = None;
    h.until(
        h.main,
        "post-close refresh completed on a newer enabled scene",
        |window, cx| {
            let (revision, idle) = {
                let state = h.worker.state.lock().unwrap();
                (
                    state.views["git/history"].revision,
                    state.processes.get("git").copied().unwrap_or(0) == 0,
                )
            };
            if revision > started
                && idle
                && h.node_current(h.main, "refresh", cx)
                && input::button(window, "refresh")?.disabled() != Some(true)
            {
                finished = Some(revision);
                Ok(true)
            } else {
                Ok(false)
            }
        },
        cx,
    )
    .await?;
    h.checkpoint(
        "caption_post_close_refresh",
        serde_json::json!({
            "before_revision": before, "busy_revision": started,
            "completed_revision": finished.context("post-close refresh receipt missing")?,
            "single_click": true, "busy_process_observed": true,
            "completed_idle_and_enabled": true, "unchanged_timer_is_not_a_receipt": true,
        }),
    )
}

/// Capture immutable domain source declarations, excluding unrelated scene revision changes.
fn source_state(h: &Harness, oid: &str) -> anyhow::Result<serde_json::Value> {
    let state = h.worker.state.lock().unwrap();
    let view = state
        .views
        .get("git/commit-details")
        .context("commit details source missing")?;
    ensure!(
        view.source.is_none() && view.active_native_editor().is_none(),
        "history exposed a mutable result"
    );
    ensure!(
        view.active_node("commit-identity")
            .is_some_and(|node| matches!(&node.kind, Kind::Text { text } if text == oid)),
        "commit source identity changed"
    );
    let panes = ["snapshot-0", "snapshot-1"].map(|id| {
        let node = view
            .active_node(id)
            .context("readonly source declaration missing")?;
        ensure!(
            matches!(node.kind, Kind::Editor(_)),
            "history source is no longer readonly"
        );
        Ok::<_, anyhow::Error>(serde_json::json!({"id": id, "source": node.kind}))
    });
    let [left, right] = panes;
    Ok(
        serde_json::json!({"commit": oid, "panes": [left?, right?], "mutable_document": view.source}),
    )
}

/// A native frame must still paint the same readonly bytes after each real window state transition.
async fn verify_sources(
    h: &Harness,
    target: AnyWindowHandle,
    oid: &str,
    left: &str,
    right: &str,
    before: &serde_json::Value,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    h.until(
        target,
        "loaded immutable sources preserved",
        |window, cx| {
            ensure!(
                h.app
                    .read(cx)
                    .plugin_windows
                    .get("git/commit-details")
                    .copied()
                    .map(AnyWindowHandle::from)
                    == Some(target),
                "caption action replaced the work window"
            );
            ensure!(
                &source_state(h, oid)? == before,
                "caption action replaced immutable source authority"
            );
            Ok([("snapshot-0", left), ("snapshot-1", right)]
                .into_iter()
                .all(|(id, text)| {
                    input::source(window, id).is_ok_and(|node| {
                        node.role() == Some(Role::Document) && node.value() == Some(text)
                    })
                }))
        },
        cx,
    )
    .await
}

/// Record only native state of this private child window, never another application's HWND.
fn state(observed: caption_mouse::CaptionState) -> serde_json::Value {
    serde_json::json!({"alive": observed.alive, "iconic": observed.iconic, "zoomed": observed.zoomed})
}
