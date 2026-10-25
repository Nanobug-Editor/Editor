//! Real Windows acceptance for the commit form, controlled tree checkboxes and actual index writes.
use super::*;
use gpui_base::test_support::{ElementSnapshot, snapshots};
use gpui_kit::Role;

mod appearance;
mod stability;

/// Isolate both palettes/locales and fractional scaling using the ordinary native child runner.
#[test]
#[ignore = "requires Windows desktop, Git ZIP and NANOBUG_GIT_NATIVE_COMMIT_PANEL"]
fn git_native_commit_panel() {
    assert!(requested());
    for case in [Case::SMOKE, Case::ALL[7]] {
        matrix::run(case).unwrap();
    }
}

/// Select this narrow workflow without changing application production behavior.
pub(super) fn requested() -> bool {
    std::env::var_os("NANOBUG_GIT_NATIVE_COMMIT_PANEL").is_some()
}

/// Locate the actual checkbox by its accessible file name within the commit tree.
fn checkbox(window: &Window, label: &str) -> anyhow::Result<ElementSnapshot> {
    snapshots(window)
        .into_iter()
        .find(|node| {
            node.role() == Some(Role::CheckBox)
                && node.label() == Some(label)
                && node.visible()
                && input::named(node, "plugin-tree-staging")
        })
        .context("commit checkbox not painted")
}

/// Native pointer/keyboard input operates only on the isolated fixture and installed candidate.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.activate(cx).await?;
    h.settle(cx).await?;
    check_unversioned_selection(h, cx).await?;
    check_locked_directory(h, cx).await?;
    rapid_tracked_reversal(h, cx).await?;
    appearance::exercise(h, cx).await?;
    h.capture(h.main, "commit-default.png", cx)?;
    h.click_button(h.main, "commit-expand", cx).await?;
    h.select_file(h.main, "staging", "a-review.txt", cx).await?;
    h.settle(cx).await?;
    let history_revision = h.worker.state.lock().unwrap().views["git/history"].revision;
    h.main.update(cx, |_, window, cx| {
        input::click(window, &input::field(window, "commit-message")?, cx)?;
        Ok::<_, anyhow::Error>(())
    })??;
    h.until(
        h.main,
        "commit field focused",
        |window, _| Ok(input::field(window, "commit-message")?.focused() == Some(true)),
        cx,
    )
    .await?;
    h.main.update(cx, |_, window, cx| {
        editor_ui::testing::input_text(window, "Commit layout 中文\nSecond line", cx)
    })?;
    h.settle(cx).await?;
    ensure!(
        h.worker.state.lock().unwrap().views["git/history"].revision == history_revision,
        "typing in the composer replaced the history scene"
    );
    let staging_revision = h.worker.state.lock().unwrap().views["git/staging"].revision;
    let previous_commit = h.worker.state.lock().unwrap().views["git/history"]
        .active_node("history-oid")
        .cloned();
    h.main.update(cx, |_, window, cx| {
        let branches = input::tree(window, "branches")?.bounds();
        let at = point(branches.right() + px(32.), branches.top() + px(42.));
        input::point_click(window, at, cx)?;
        Ok::<_, anyhow::Error>(())
    })??;
    h.settle(cx).await?;
    ensure!(
        h.worker.state.lock().unwrap().views["git/history"].active_node("history-oid")
            != previous_commit.as_ref(),
        "native history click did not select another commit"
    );
    ensure!(
        h.worker.state.lock().unwrap().views["git/staging"].revision == staging_revision,
        "selecting a history row replaced the composer scene"
    );
    h.record("panel_selection_and_draft_isolation", true.into());
    h.click_button(h.main, "commit-hunks", cx).await?;
    h.click_button(h.main, "hunk-0", cx).await?;
    let reviewed = h.fixture.review.clone();
    h.until_repository(
        "partial index retained",
        move |root| {
            let index = fixture::git_text(root, &["show", ":src/a-review.txt"])?;
            Ok(
                index != fixture::git_text(root, &["show", "HEAD:src/a-review.txt"])?
                    && index != reviewed,
            )
        },
        cx,
    )
    .await?;
    h.settle(cx).await?;
    stability::click(h, "a-review.txt", cx).await?;
    let reviewed = h.fixture.review.clone();
    h.until_repository(
        "checking stages remaining worktree changes",
        move |root| Ok(fixture::git_text(root, &["show", ":src/a-review.txt"])? == reviewed),
        cx,
    )
    .await?;
    h.settle(cx).await?;
    h.capture(h.main, "commit-checked.png", cx)?;
    let form = h.main.update(cx, |_, window, cx| {
        let field = input::field(window, "commit-message")?.bounds();
        input::wheel(
            window,
            input::tree(window, "staging")?.bounds().center(),
            -160.,
            cx,
        );
        Ok::<_, anyhow::Error>(field)
    })??;
    h.frame(h.main, cx).await?;
    h.main.update(cx, |_, window, _| {
        ensure!(
            (input::field(window, "commit-message")?.bounds().top() - form.top()).abs() < px(1.),
            "file scrolling moved the composer"
        );
        ensure!(
            input::button(window, "commit")?.focused().is_some(),
            "valid staged commit disabled"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    // Restore the tree position via explicit collapse/expand rather than changing its private scroll state.
    h.click_button(h.main, "commit-collapse", cx).await?;
    h.click_button(h.main, "commit-expand", cx).await?;
    h.settle(cx).await?;
    stability::click(h, "a-review.txt", cx).await?;
    h.until_repository(
        "unchecking restores only the index",
        |root| {
            Ok(
                fixture::git_text(root, &["diff", "--cached", "--", "src/a-review.txt"])?
                    .is_empty(),
            )
        },
        cx,
    )
    .await?;
    h.settle(cx).await?;
    h.main.update(cx, |_, window, _| {
        ensure!(
            // Base's disabled buttons withdraw their focus binding; disabled() is not exposed.
            input::button(window, "commit")?.focused().is_none(),
            "empty index still permits commit"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    let state = h.worker.state.lock().unwrap();
    ensure!(
        state.views["git/staging"].active_node("commit").is_none(),
        "guest permits an empty-index commit"
    );
    if let Some(status) = state.views["git/staging"].active_node("status") {
        ensure!(
            !matches!(&status.kind, plugin_runtime::plugin_protocol::ui::Kind::Text {text} if text.contains("Tree update cannot")),
            "branch metadata update failed during staging"
        );
    }
    let field = state
        .views
        .get("git/staging")
        .and_then(|view| view.active_node("commit-message"))
        .context("message missing")?;
    ensure!(
        matches!(&field.kind, plugin_runtime::plugin_protocol::ui::Kind::Textarea(input) if input.value == "Commit layout 中文\nSecond line"),
        "draft changed during staging/scrolling: {:?}",
        field.kind
    );
    drop(state);
    h.record("checkbox_partial_stage_unstage_and_draft", true.into());
    h.capture(h.main, "commit-final.png", cx)?;
    Ok(())
}

/// Unversioned selection is local even under a competing lock; the index and panel documents stay intact.
async fn check_unversioned_selection(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let index = fs::read(h.fixture.workspace.join(".git/index"))?;
    let revisions = {
        let state = h.worker.state.lock().unwrap();
        (
            state.views["git/staging"].revision,
            state.views["git/history"].revision,
        )
    };
    let label = h.case.label("未进行版本管理的文件", "Unversioned Files");
    let lock = h.fixture.workspace.join(".git/index.lock");
    fs::write(&lock, b"native local selection owns this lock")?;
    stability::click(h, label, cx).await?;
    h.settle(cx).await?;
    ensure!(fs::read(h.fixture.workspace.join(".git/index"))? == index);
    ensure!(
        fixture::git_text(
            &h.fixture.workspace,
            &["ls-files", "--others", "--exclude-standard"]
        )?
        .lines()
        .count()
            == 28
    );
    {
        let state = h.worker.state.lock().unwrap();
        ensure!(
            (
                state.views["git/staging"].revision,
                state.views["git/history"].revision
            ) == revisions,
            "local selection replaced a panel scene"
        );
        ensure!(state.processes.get("git").copied().unwrap_or(0) == 0);
        for id in ["commit-summary", "commit-feedback", "status"] {
            ensure!(
                state.views["git/staging"].active_node(id).is_none(),
                "removed feedback returned"
            );
        }
    }
    h.capture(h.main, "commit-unversioned-selected.png", cx)?;
    stability::click(h, label, cx).await?;
    ensure!(fs::read(&lock)? == b"native local selection owns this lock");
    fs::remove_file(lock)?;
    h.record(
        "unversioned_selection_preserves_index_and_scenes",
        true.into(),
    );
    Ok(())
}

/// Exercise real tracked group gestures under a fixture-owned lock, then verify rollback and recovery.
async fn check_locked_directory(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let before = h.main.update(cx, |_, window, _| {
        Ok::<_, anyhow::Error>(input::tree(window, "staging")?.bounds().size.height)
    })??;
    let lock = h.fixture.workspace.join(".git/index.lock");
    fs::write(&lock, b"native fixture owns this lock")?;
    stability::click(h, h.case.label("更改", "Changes"), cx).await?;
    h.settle(cx).await?;
    ensure!(fs::read(&lock)? == b"native fixture owns this lock");
    h.until(
        h.main,
        "failed check rolls back its native marker",
        |window, _| Ok(checkbox(window, h.case.label("更改", "Changes"))?.checked() == Some(false)),
        cx,
    )
    .await?;
    {
        let state = h.worker.state.lock().unwrap();
        ensure!(
            state.views["git/staging"].active_node("status").is_none(),
            "removed status returned"
        );
    }
    ensure!(
        fixture::git_text(&h.fixture.workspace, &["diff", "--cached", "--name-only"])?.is_empty()
    );
    h.main.update(cx, |_, window, _| {
        ensure!(
            (input::tree(window, "staging")?.bounds().size.height - before).abs() < px(1.),
            "lock error expanded the composer"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    h.capture(h.main, "commit-lock.png", cx)?;
    // The fixture, never the plugin, releases the competing operation's lock.
    fs::remove_file(&lock)?;
    stability::click(h, h.case.label("更改", "Changes"), cx).await?;
    h.until_repository(
        "tracked group staged after lock release",
        |root| {
            Ok(
                fixture::git_text(root, &["diff", "--cached", "--name-only"])?
                    .lines()
                    .count()
                    == 1,
            )
        },
        cx,
    )
    .await?;
    h.settle(cx).await?;
    stability::click(h, h.case.label("更改", "Changes"), cx).await?;
    h.until_repository(
        "group uncheck clears every descendant",
        |root| Ok(fixture::git_text(root, &["diff", "--cached", "--name-only"])?.is_empty()),
        cx,
    )
    .await?;
    h.settle(cx).await?;
    h.record(
        "tracked_directory_lock_rollback_and_recovery",
        serde_json::json!(true),
    );
    Ok(())
}

/// Reverse a real tracked group before its first Git write settles; the second click must survive.
async fn rapid_tracked_reversal(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let label = h.case.label("更改", "Changes");
    for selected in [true, false] {
        h.main.update(cx, |_, window, cx| {
            input::click(window, &checkbox(window, label)?, cx)
        })??;
        h.until(
            h.main,
            "rapid tracked marker responds",
            |window, _| Ok(checkbox(window, label)?.checked() == Some(selected)),
            cx,
        )
        .await?;
    }
    h.until_repository(
        "rapid reversal restores the index",
        |root| Ok(fixture::git_text(root, &["diff", "--cached", "--name-only"])?.is_empty()),
        cx,
    )
    .await?;
    h.settle(cx).await?;
    // Wait through both serialized operations before concluding from an initially empty index.
    h.main.update(cx, |_, window, _| {
        ensure!(checkbox(window, label)?.checked() == Some(false));
        Ok::<_, anyhow::Error>(())
    })??;
    h.record("rapid_native_tracked_reversal", true.into());
    Ok(())
}
