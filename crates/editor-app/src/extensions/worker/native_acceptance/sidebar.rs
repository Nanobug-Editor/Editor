//! Shared-sidebar acceptance retains the actual Git viewer through client dragging and tool-window visibility.
use super::super::native_close_tests::caption_mouse::CursorRestore;
use super::*;
use gpui_base::test_support::{ElementSnapshot, snapshots};
use gpui_kit::{ElementId, Role};
use plugin_runtime::plugin_protocol::ui::Kind;

mod geometry;
pub(super) mod title_actions;

const DRAFT: &str = "Shared sidebar draft 中😀";

/// Each caption-independent sidebar case owns a native process/profile and a bounded cleanup receipt.
#[test]
#[ignore = "requires Windows desktop/D3D11, candidate/output and NANOBUG_GIT_NATIVE_SHARED_SIDEBAR"]
fn git_native_shared_sidebar_peers_drag_and_visibility() {
    assert!(requested());
    assert!(
        !title_actions::requested(),
        "full sidebar acceptance refuses title-actions-only workflow"
    );
    for flag in [
        "NANOBUG_GIT_NATIVE_CAPTION_CONTROLS",
        "NANOBUG_GIT_NATIVE_IDLE_RETENTION",
        "NANOBUG_GIT_NATIVE_HISTORY_ONLY",
        "NANOBUG_GIT_NATIVE_HISTORY_PHASES",
    ] {
        assert!(
            std::env::var_os(flag).is_none(),
            "sidebar workflow refuses {flag}"
        );
    }
    let _cursor = CursorRestore::capture().unwrap();
    let scaled = Case::ALL
        .into_iter()
        .find(|case| case.name == "light-en-150")
        .unwrap();
    for case in [Case::SMOKE, scaled] {
        matrix::run(case).unwrap();
        eprintln!("shared sidebar case passed: {}", case.name);
    }
}

/// This narrow workflow never satisfies full workbench, caption or keyboard acceptance.
pub(super) fn requested() -> bool {
    std::env::var_os("NANOBUG_GIT_NATIVE_SHARED_SIDEBAR").is_some() || title_actions::requested()
}

/// Start from the ordinary Explorer-visible default, then use current controls and production input routes.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.activate(cx).await?;
    h.settle(cx).await?;
    if title_actions::requested() {
        return title_actions::exercise(h, cx).await;
    }
    // Keep the actual default pixels even when a too-small staging clip fails the strict checks.
    h.capture(h.main, "sidebar-default.png", cx)?;
    geometry::record_default(h, cx)?;
    h.expand(h.main, "staging", h.case.label("更改", "Changes"), cx)
        .await?;
    h.expand(h.main, "staging", "src", cx).await?;
    h.select_file(h.main, "staging", "a-review.txt", cx).await?;
    create_draft(h, cx).await?;
    let retained = cx.update(|cx| retained_state(h, cx))?;
    ensure!(
        retained["native_selection"].is_string(),
        "real file selection did not remain in Base"
    );
    h.checkpoint("sidebar_retained_before", retained.clone())?;
    toggle(h, "explorer-panel-toggle", cx).await?;
    h.until(
        h.main,
        "Explorer hidden by its normal toolbar",
        |_, cx| {
            let app = h.app.read(cx);
            Ok(!app.explorer_visible && !app.session_state.outline_visible)
        },
        cx,
    )
    .await?;
    h.settle(cx).await?;
    h.capture(h.main, "sidebar-all-hidden.png", cx)?;
    h.main.update(cx, |_, window, cx| {
        let app = h.app.read(cx);
        ensure!(
            !app.explorer_visible && !app.session_state.outline_visible,
            "host peers were not hidden by the toolbar"
        );
        let clip = geometry::staging_controls(h, window, cx)?;
        h.record(
            "sidebar_all_host_peers_hidden",
            serde_json::json!({"clip": format!("{clip:?}"), "positive_interactive_tree": true}),
        );
        Ok::<_, anyhow::Error>(())
    })??;
    assert_retained(h, &retained, false, cx).await?;
    toggle(h, "explorer-panel-toggle", cx).await?;
    h.until(
        h.main,
        "Explorer shown by its normal toolbar",
        |_, cx| Ok(h.app.read(cx).explorer_visible),
        cx,
    )
    .await?;
    toggle(h, "outline-toggle", cx).await?;
    h.until(
        h.main,
        "Outline shown by its normal toolbar",
        |_, cx| Ok(h.app.read(cx).session_state.outline_visible),
        cx,
    )
    .await?;
    h.settle(cx).await?;
    h.capture(h.main, "sidebar-before.png", cx)?;
    let before = geometry::record(h, "sidebar_before", cx)?;
    assert_retained(h, &retained, false, cx).await?;
    drag_to_first_peer(h, before.staging.outer, before.explorer.content, cx).await?;
    // Drop changes Base's real layout while ordinary focus refresh may still be in flight.
    // Await the same native readiness used after visibility changes before strict geometry checks.
    h.settle(cx).await?;
    h.until(
        h.main,
        "staging title drag moved it before Explorer",
        |window, cx| {
            let actual = geometry::sample(h, window, cx)?;
            Ok(actual.staging.outer.bottom() <= actual.explorer.outer.top() + px(2.))
        },
        cx,
    )
    .await?;
    h.capture(h.main, "sidebar-after-drag.png", cx)?;
    geometry::record(h, "sidebar_after_drag", cx)?;
    assert_retained(h, &retained, false, cx).await?;
    // This is ordinary tool-window hide/show, not a restart or a direct mutation of its Cell/layout.
    toggle(h, "plugin-window-git/staging", cx).await?;
    h.until(
        h.main,
        "staging hidden by its toolbar",
        |window, cx| {
            Ok(!h.app.read(cx).plugin_panels["git/staging"]
                .read(cx)
                .visible
                .get()
                && input::field(window, "commit-message").is_err())
        },
        cx,
    )
    .await?;
    toggle(h, "plugin-window-git/staging", cx).await?;
    h.until(
        h.main,
        "staging shown by its toolbar",
        |window, cx| {
            Ok(h.app.read(cx).plugin_panels["git/staging"]
                .read(cx)
                .visible
                .get()
                && input::field(window, "commit-message").is_ok())
        },
        cx,
    )
    .await?;
    h.settle(cx).await?;
    h.capture(h.main, "sidebar-after-reopen.png", cx)?;
    let reopened = geometry::record(h, "sidebar_after_reopen", cx)?;
    ensure!(
        reopened.staging.outer.bottom() <= reopened.explorer.outer.top() + px(2.),
        "hide/show reset the dragged order"
    );
    // hide() must retire native input/IME targets. The host panel and public resource survive;
    // remounting a fresh NativeUi after show() is the required input-lifetime contract.
    assert_retained(h, &retained, true, cx).await?;
    let remounted = cx.update(|cx| retained_state(h, cx))?;
    ensure!(
        remounted["native_ui_entity"] != retained["native_ui_entity"],
        "hidden native inputs were not retired and remounted"
    );
    ensure!(
        remounted["tree_epoch"].as_u64() > retained["tree_epoch"].as_u64(),
        "hidden native tree callbacks were not revoked"
    );
    h.record("sidebar_native_input_remount", serde_json::json!({"before": retained, "after": remounted, "retired_then_remounted": true, "old_tree_epoch_revoked": true}));
    h.record("shared_sidebar_complete", serde_json::json!({
        "three_independent_peers": true, "ordinary_default_controls_usable": true,
        "all_hidden_tree_usable": true,
        "client_title_drag": "production Window.dispatch_event MouseDown/Move/Up on real WindowsPlatform",
        "actual_os_sendinput": false, "single_drag": true,
        "normal_tool_window_hide_show": true, "draft_entity_and_tree_retained": true,
        "native_restart_verified": false, "diagnostic_only": true,
    }));
    Ok(())
}

/// Tool-window buttons are host Base controls, not guest nodes with plugin-ui-prefixed identities.
fn toolbar_button(window: &Window, id: &str) -> anyhow::Result<ElementSnapshot> {
    snapshots(window)
        .into_iter()
        .find(|node| {
            node.visible()
                && node.role() == Some(Role::Button)
                && node
                    .path()
                    .iter()
                    .any(|part| matches!(part, ElementId::Name(name) if name.as_ref() == id))
        })
        .with_context(|| format!("native tool-window button {id} not painted"))
}

/// Wait for a painted host button and emit exactly one client click, never replaying a dispatched toggle.
pub(super) async fn toggle(h: &Harness, id: &str, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.until(
        h.main,
        id,
        |window, cx| {
            let Ok(button) = toolbar_button(window, id) else {
                return Ok(false);
            };
            if button.disabled() == Some(true) {
                return Ok(false);
            }
            input::click(window, &button, cx)?;
            Ok(true)
        },
        cx,
    )
    .await
}

/// The native textarea and the guest publication must both acknowledge the same unsent commit draft.
async fn create_draft(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.main.update(cx, |_, window, cx| {
        input::click(window, &input::field(window, "commit-message")?, cx)?;
        Ok::<_, anyhow::Error>(())
    })??;
    h.until(
        h.main,
        "commit textarea received native focus",
        |window, _| Ok(input::field(window, "commit-message")?.focused() == Some(true)),
        cx,
    )
    .await?;
    h.main.update(cx, |_, window, cx| {
        editor_ui::testing::input_text(window, DRAFT, cx)
    })?;
    h.until(
        h.main,
        "shared-sidebar draft acknowledged",
        |window, _| {
            Ok(
                input::field(window, "commit-message")?.value() == Some(DRAFT)
                    && draft_acknowledged(h),
            )
        },
        cx,
    )
    .await
}

/// Read only stable native-view and public tree ownership; no duplicate text/tree model is maintained.
fn retained_state(h: &Harness, cx: &App) -> anyhow::Result<serde_json::Value> {
    let app = h.app.read(cx);
    let panel = app
        .plugin_panels
        .get("git/staging")
        .context("staging viewer missing")?;
    let native = panel
        .read(cx)
        .native_ui
        .clone()
        .context("native staging UI missing")?;
    let viewer = format!("{:?}", native.entity_id());
    let native_selection = native
        .read(cx)
        .native_tree_selected_id("staging", cx)
        .context("native staging tree missing")?;
    let state = h.worker.state.lock().unwrap();
    let Kind::Tree { handle } = &state.views["git/staging"]
        .active_node("staging")
        .context("staging tree missing")?
        .kind
    else {
        anyhow::bail!("staging source is not a tree")
    };
    let tree = state
        .trees
        .iter()
        .find(|tree| tree.handle() == handle)
        .context("owned tree resource missing")?;
    let snapshot = tree.snapshot();
    Ok(
        serde_json::json!({"panel_entity": format!("{:?}", panel.entity_id()), "native_ui_entity": viewer,
        "tree_handle": handle, "tree_epoch": snapshot.epoch, "expanded": snapshot.expanded,
        "native_selection": native_selection}),
    )
}

/// Geometry may change during dragging, while original viewer ownership, expansion and the draft stay intact.
async fn assert_retained(
    h: &Harness,
    expected: &serde_json::Value,
    allow_native_remount: bool,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    h.until(
        h.main,
        "sidebar draft and tree retained",
        |window, cx| {
            let mut current = retained_state(h, cx)?;
            let mut previous = expected.clone();
            if allow_native_remount {
                // Hiding revokes the old native callbacks and releases Base's local selection.
                // The new mount keeps the public tree, expansions, draft and business selection.
                for field in ["native_ui_entity", "native_selection", "tree_epoch"] {
                    current.as_object_mut().unwrap().remove(field);
                    previous.as_object_mut().unwrap().remove(field);
                }
            }
            ensure!(
                current == previous,
                "sidebar operation replaced retained panel/tree ownership or expansion"
            );
            if !allow_native_remount {
                if let Ok(item) = input::tree_item(window, "staging", "a-review.txt") {
                    ensure!(
                        item.selected() == Some(true),
                        "visible selected file lost native selection"
                    );
                }
            }
            Ok(draft_acknowledged(h)
                && input::field(window, "commit-message")?.value() == Some(DRAFT))
        },
        cx,
    )
    .await
}

/// Read the authoritative guest publication rather than accepting local textarea state alone.
fn draft_acknowledged(h: &Harness) -> bool {
    h.worker.state.lock().unwrap().views["git/staging"]
        .active_node("commit-message")
        .is_some_and(|node| matches!(&node.kind, Kind::Textarea(value) if value.value == DRAFT))
}

/// Client dragging uses production GPUI dispatch, not Win32 caption messages or direct DockArea edits.
/// Each phase gets an actual frame before its next single input; the final peer order is mandatory.
async fn drag_to_first_peer(
    h: &Harness,
    source: Bounds<Pixels>,
    target: Bounds<Pixels>,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let start = point(
        source.center().x,
        source.top() + px(PANEL_HEADER_HEIGHT / 2.),
    );
    let destination = point(target.center().x, target.top() + px(8.));
    h.checkpoint("sidebar_drag_attempt", serde_json::json!({"from": format!("{start:?}"), "to": format!("{destination:?}"), "input_method": "production-window-client-dispatch"}))?;
    h.main.update(cx, |_, window, cx| {
        let viewport = Bounds::new(point(px(0.), px(0.)), window.viewport_size());
        ensure!(
            viewport.contains(&start) && viewport.contains(&destination),
            "dock drag is outside the native viewport"
        );
        window.activate_window();
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                position: start,
                button: MouseButton::Left,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseMove(gpui_kit::MouseMoveEvent {
                position: start + point(px(12.), px(0.)),
                pressed_button: Some(MouseButton::Left),
                modifiers: Default::default(),
            }),
            cx,
        );
        Ok::<_, anyhow::Error>(())
    })??;
    h.until(
        h.main,
        "native Base drag accepted the current panel title",
        |_, cx| Ok(cx.has_active_drag()),
        cx,
    )
    .await?;
    h.main.update(cx, |_, window, cx| {
        window.dispatch_event(
            PlatformInput::MouseMove(gpui_kit::MouseMoveEvent {
                position: destination,
                pressed_button: Some(MouseButton::Left),
                modifiers: Default::default(),
            }),
            cx,
        );
    })?;
    h.frame(h.main, cx).await?;
    h.capture(h.main, "sidebar-drag-preview.png", cx)?;
    h.main.update(cx, |_, window, cx| {
        ensure!(
            cx.has_active_drag(),
            "native drag ended before the only drop"
        );
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position: destination,
                button: MouseButton::Left,
                modifiers: Default::default(),
                click_count: 1,
            }),
            cx,
        );
        Ok::<_, anyhow::Error>(())
    })??;
    Ok(())
}
