//! Real installed providers share Explorer colors and native title menus in isolated Windows profiles.
use super::*;
use gpui_base::test_support::{ElementSnapshot, snapshots};
use gpui_kit::Role;
use plugin_runtime::plugin_protocol::Paint;
mod settings;

/// This receipt covers two palettes/languages and fractional DPI without claiming full Git acceptance.
#[test]
#[ignore = "requires Windows desktop, Git/review ZIPs, output and NANOBUG_GIT_NATIVE_WORKSPACE_PRESENTATION"]
fn git_native_workspace_presentation() {
    assert!(requested());
    for case in [Case::SMOKE, Case::ALL[7]] {
        matrix::run(case).unwrap();
    }
}

/// Narrow diagnostic selection is inherited only by its private child process.
pub(super) fn requested() -> bool {
    std::env::var_os("NANOBUG_GIT_NATIVE_WORKSPACE_PRESENTATION").is_some()
}

/// Exercise actual installed packages, painted controls, typed ownership and normal resource retirement.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.activate(cx).await?;
    h.until(
        h.main,
        "Git title and filename metadata",
        |window, cx| {
            let app = h.app.read(cx);
            Ok(control(window, "workspace-entry-git/branch").is_ok()
                && app
                    .explorer_file_decoration(
                        &h.fixture
                            .workspace
                            .canonicalize()?
                            .join(fixture::REVIEW_PATH),
                        false,
                        cx,
                    )
                    .is_some())
        },
        cx,
    )
    .await?;
    check_files(h, cx)?;
    // The host Explorer must actually paint the affected file; metadata alone is insufficient.
    h.until(
        h.main,
        "Explorer src row",
        |window, cx| {
            let row = snapshots(window).into_iter().find(|node| {
                node.visible()
                    && node.role() == Some(Role::TreeItem)
                    && node.label() == Some("src")
                    && input::named(node, "explorer-root")
            });
            let Some(row) = row else {
                return Ok(false);
            };
            if row.expanded() != Some(true) {
                input::double_click(window, &row, cx)?;
            }
            Ok(true)
        },
        cx,
    )
    .await?;
    h.until(
        h.main,
        "painted Explorer review file",
        |window, _| {
            Ok(snapshots(window).iter().any(|node| {
                node.visible()
                    && node.label() == Some("a-review.txt")
                    && input::named(node, "explorer-root")
            }))
        },
        cx,
    )
    .await?;
    h.capture(h.main, "presentation-files.png", cx)?;
    head_menu(h, cx).await?;
    filtered_history(h, cx).await?;
    second_provider(h, cx).await?;
    settings::exercise(h, cx).await?;
    overflow(h, cx).await?;
    withdraw(h, cx).await?;
    h.record("workspace_presentation_complete", true.into());
    Ok(())
}

/// Source semantics, directory exclusion and the automatic choice are read from production owners.
fn check_files(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.main.update(cx, |_, _, cx| {
        let app = h.app.read(cx);
        let modified = app.explorer_file_decoration(&h.fixture.workspace.canonicalize()?.join(fixture::REVIEW_PATH), false, cx).context("modified decoration")?;
        let added = app.explorer_file_decoration(&h.fixture.workspace.canonicalize()?.join("src/z-note-00.txt"), false, cx).context("added decoration")?;
        ensure!(modified.0 != added.0, "modified and new files share a color");
        ensure!(app.explorer_file_decoration(&h.fixture.workspace.canonicalize()?.join("src"), true, cx).is_none(), "directory was colored");
        h.record("filename_colors", serde_json::json!({"modified": format!("{:?}", modified), "added": format!("{:?}", added), "directories_unchanged": true}));
        Ok::<_, anyhow::Error>(())
    })??;
    let preferences: serde_json::Value = serde_json::from_slice(&fs::read(
        h.fixture.profile.join("workspace-presentation.json"),
    )?)?;
    ensure!(
        preferences["providers"][h.fixture.workspace.canonicalize()?.to_str().unwrap()] == "git",
        "single provider was not remembered"
    );
    Ok(())
}

/// Current HEAD offers no self-checkout/delete; a lazy submenu refresh preserves the native popup.
async fn head_menu(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    click_control(h, h.main, "workspace-entry-git/branch", cx).await?;
    click_menu(h, "main", cx).await?;
    h.until(
        h.main,
        "HEAD contextual actions",
        |window, _| {
            Ok(menu_labels(window, "action-menu-column-1")
                .iter()
                .any(|label| label == h.case.label("重命名…", "Rename…")))
        },
        cx,
    )
    .await?;
    h.main.update(cx, |_, window, _| {
        let labels = menu_labels(window, "action-menu-column-1");
        ensure!(
            !labels
                .iter()
                .any(|label| label == h.case.label("签出", "Checkout")
                    || label == h.case.label("删除…", "Delete…")),
            "HEAD offered checkout/delete: {labels:?}"
        );
        ensure!(
            labels.iter().any(|label| label
                == h.case
                    .label("显示与工作树的差异", "Show diff with working tree")),
            "HEAD worktree comparison missing"
        );
        h.record("head_actions", serde_json::json!(labels));
        Ok::<_, anyhow::Error>(())
    })??;
    h.capture(h.main, "presentation-head-menu.png", cx)?;
    close_menu(h, cx).await
}

/// A double click selects historical refs while the actual checked-out title stays main.
async fn filtered_history(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.expand(h.main, "branches", h.case.label("本地", "Local"), cx)
        .await?;
    h.until(
        h.main,
        "incoming double click",
        |window, cx| {
            let Ok(row) = input::tree_item(window, "branches", "incoming") else {
                return Ok(false);
            };
            input::double_click(window, &row, cx)?;
            Ok(true)
        },
        cx,
    )
    .await?;
    h.settle(cx).await?;
    h.until(h.main, "incoming filtered history with main title", |window, _| {
        let state = h.worker.state.lock().unwrap();
        let has = state.views.get("git/history").and_then(|view| view.active_node("log")).is_some_and(|node| {
            matches!(&node.kind, plugin_runtime::plugin_protocol::ui::Kind::Canvas(canvas) if canvas.paint.iter().any(|paint| matches!(paint, Paint::Text { text, .. } if text == "Incoming native changes")))
        });
        Ok(has && control(window, "workspace-entry-git/branch").is_ok_and(|row| row.label() == Some("main")))
    }, cx).await?;
    h.record("filter_independent_of_head", true.into());
    Ok(())
}

/// Installing a non-Git provider must not replace the remembered filename source by arrival order.
async fn second_provider(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let path = std::env::var_os("NANOBUG_REVIEW_CANDIDATE").context("review ZIP required")?;
    let package = Package::read(Path::new(&path))?;
    ensure!(
        h.worker
            .queue_lifecycle(super::super::Work::Install(package, true)),
        "review install not queued"
    );
    h.until(
        h.main,
        "second public provider title",
        |window, _| Ok(control(window, "workspace-entry-review-queue/queue").is_ok()),
        cx,
    )
    .await?;
    check_files(h, cx)?;
    h.record("second_provider_did_not_override_git", true.into());
    Ok(())
}

/// Real client resizing forces overflow, whose search must reach Git's nested branch rows.
async fn overflow(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.main
        .update(cx, |_, window, _| resize_owned(window, 700., 900.))??;
    h.until(
        h.main,
        "narrow title overflow",
        |window, _| Ok(control(window, "workspace-entry-overflow").is_ok()),
        cx,
    )
    .await?;
    click_control(h, h.main, "workspace-entry-overflow", cx).await?;
    h.frame(h.main, cx).await?;
    h.main.update(cx, |_, window, cx| {
        editor_ui::testing::input_text(window, "incoming", cx)
    })?;
    h.until(
        h.main,
        "search reaches Git overflow descendants",
        |window, _| {
            Ok(menu_labels(window, "action-menu-column-0")
                .iter()
                .any(|label| label == "incoming"))
        },
        cx,
    )
    .await?;
    h.capture(h.main, "presentation-overflow.png", cx)?;
    close_menu(h, cx).await?;
    h.main
        .update(cx, |_, window, _| resize_owned(window, 1280., 900.))??;
    h.frame(h.main, cx).await
}

/// Disable withdraws live colors/title/menu and prevents retained resources from executing again.
async fn withdraw(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.until(
        h.main,
        "restored Git title",
        |window, _| Ok(control(window, "workspace-entry-git/branch").is_ok()),
        cx,
    )
    .await?;
    click_control(h, h.main, "workspace-entry-git/branch", cx).await?;
    let resource = h
        .worker
        .state
        .lock()
        .unwrap()
        .presentations
        .iter()
        .find(|resource| resource.plugin() == "git")
        .cloned()
        .context("Git presentation")?;
    ensure!(
        h.worker
            .queue_lifecycle(super::super::Work::Disable("git".into())),
        "disable not queued"
    );
    h.until(
        h.main,
        "disabled provider withdrawal",
        |window, cx| {
            Ok(!resource.is_live()
                && control(window, "workspace-entry-git/branch").is_err()
                && h.app
                    .read(cx)
                    .explorer_file_decoration(
                        &h.fixture
                            .workspace
                            .canonicalize()?
                            .join(fixture::REVIEW_PATH),
                        false,
                        cx,
                    )
                    .is_none())
        },
        cx,
    )
    .await?;
    let snapshot = resource.snapshot();
    ensure!(
        resource.command(snapshot.revision, "branch", None).is_err(),
        "retired resource command accepted"
    );
    h.record("retired_provider_no_fallback", true.into());
    h.capture(h.main, "presentation-withdrawn.png", cx)
}

/// Snapshot lookup never bypasses actual visibility, enabled state or the original native hit target.
fn control(window: &Window, id: &str) -> anyhow::Result<ElementSnapshot> {
    snapshots(window)
        .into_iter()
        .find(|node| {
            node.visible()
                && Bounds::new(point(px(0.), px(0.)), window.viewport_size())
                    .contains(&node.bounds().center())
                && input::named(node, id)
                && matches!(node.role(), Some(Role::Button | Role::CheckBox))
        })
        .with_context(|| format!("native control {id} unavailable"))
}
/// A single dispatch occurs only after the current frame exposes the intended stable control.
async fn click_control(
    h: &Harness,
    target: AnyWindowHandle,
    id: &str,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    h.until(
        target,
        id,
        |window, cx| {
            let Ok(row) = control(window, id) else {
                return Ok(false);
            };
            input::click(window, &row, cx)?;
            Ok(true)
        },
        cx,
    )
    .await?;
    h.frame(target, cx).await
}
/// Match a menu label within the current root column instead of invoking a guest command directly.
async fn click_menu(h: &Harness, label: &str, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.until(
        h.main,
        label,
        |window, cx| {
            let row = snapshots(window).into_iter().find(|node| {
                node.visible()
                    && node.role() == Some(Role::MenuItem)
                    && node.label() == Some(label)
                    && input::named(node, "action-menu-column-0")
            });
            let Some(row) = row else {
                return Ok(false);
            };
            input::click(window, &row, cx)?;
            Ok(true)
        },
        cx,
    )
    .await?;
    h.frame(h.main, cx).await
}
/// Native accessibility labels are sufficient to compare contextual behavior across locale variants.
fn menu_labels(window: &Window, column: &str) -> Vec<String> {
    snapshots(window)
        .into_iter()
        .filter(|node| {
            node.visible() && node.role() == Some(Role::MenuItem) && input::named(node, column)
        })
        .filter_map(|node| node.label().map(str::to_owned))
        .collect()
}
/// Escape closes at most one cascading level per key, retaining the original focus lease.
async fn close_menu(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    for _ in 0..4 {
        h.main.update(cx, |_, window, cx| {
            editor_ui::testing::press_key(window, "escape", cx)
        })?;
        h.frame(h.main, cx).await?;
    }
    Ok(())
}
/// Change only the child-owned HWND's size, leaving system display settings and other windows intact.
fn resize_owned(window: &mut Window, width: f32, height: f32) -> anyhow::Result<()> {
    window.resize(size(px(width), px(height)));
    Ok(())
}
