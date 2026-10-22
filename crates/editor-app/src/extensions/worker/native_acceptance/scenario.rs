//! A short production workflow crosses review, index commit, immutable history and an actual Git conflict.
use super::*;
use gpui_kit::Role;
use plugin_runtime::plugin_protocol::ui::{Kind, Node};

mod branches;
mod history;
mod ime;
mod tree_projection;

/// Every step uses the installed guest and current native controls; screenshots are evidence of these exact scenes.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    eprintln!("native {}: activate", h.case.name);
    h.dpi(h.main, cx).await?;
    if commit_panel::requested() {
        return commit_panel::exercise(h, cx).await;
    }
    if workspace_presentation::requested() {
        return workspace_presentation::exercise(h, cx).await;
    }
    if exclusive::requested() {
        return exclusive::exercise(h, cx).await;
    }
    if sidebar::requested() {
        return sidebar::exercise(h, cx).await;
    }
    if caption_controls::requested() {
        return caption_controls::exercise(h, cx).await;
    }
    if std::env::var_os("NANOBUG_GIT_NATIVE_IDLE_RETENTION").is_some() {
        return idle_retention::exercise(h, cx).await;
    }
    if std::env::var_os("NANOBUG_GIT_NATIVE_HISTORY_ONLY").is_some() {
        // The diagnostic skips unrelated editor/branch flows while retaining the same real
        // pointer, wheel, End and single Enter against the installed guest and native canvas.
        let root = h.fixture.workspace.clone();
        cx.background_executor()
            .spawn(async move {
                fixture::git(
                    &root,
                    &["commit", "--allow-empty", "-m", "Native UI commit 中文"],
                )
            })
            .await?;
        h.activate(cx).await?;
        if std::env::var_os("NANOBUG_GIT_NATIVE_SCROLL_ONLY").is_some() {
            return history::exercise_scroll(h, cx).await;
        }
        return history::exercise(h, cx).await;
    }
    h.activate(cx).await?;
    branches::exercise(h, cx).await?;
    review(h, cx).await?;
    eprintln!("native {}: index commit", h.case.name);
    stage_and_commit(h, cx).await?;
    eprintln!("native {}: readonly history", h.case.name);
    history::exercise(h, cx).await?;
    eprintln!("native {}: real conflict", h.case.name);
    conflicts(h, cx).await?;
    Ok(())
}

/// Tree disclosure/selection and the main split comparison retain the file's sole original native session.
async fn review(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.expand(h.main, "staging", h.case.label("更改", "Changes"), cx)
        .await?;
    h.expand(h.main, "staging", "src", cx).await?;
    h.select_file(h.main, "staging", "a-review.txt", cx).await?;
    h.until(
        h.main,
        "editable comparison geometry",
        |_, cx| {
            Ok(h.app
                .read(cx)
                .comparison_alignment()
                .is_some_and(|group| group.project_range(1, 0..0, cx).is_some()))
        },
        cx,
    )
    .await?;
    let original = cx.update(|cx| h.app.read(cx).editor.clone());
    h.main.update(cx, |_, window, cx| {
        let app = h.app.read(cx);
        let group = app
            .comparison_alignment()
            .context("comparison did not mount")?;
        let source = group
            .project_range(1, 0..0, cx)
            .context("original source range not measured")?;
        input::point_click(
            window,
            point(source.left() + px(0.4), source.center().y),
            cx,
        )?;
        Ok::<_, anyhow::Error>(())
    })??;
    // The root forwards this physical pointer after the native listener table is released.
    // Observe its result on actual frames before starting composition in the original session.
    h.until(
        h.main,
        "comparison pointer focused the original editor",
        |window, cx| Ok(original.focus_handle(cx).is_focused(window)),
        cx,
    )
    .await?;
    ime::exercise(h, h.main, &original, true, cx).await?;
    let fixed_before = h.main.update(cx, |_, window, _| {
        input::field(window, "commit-message").map(|node| node.bounds())
    })??;
    let before = cx.update(|cx| original.read(cx).scroll_offset());
    h.main.update(cx, |_, window, cx| {
        let viewport = h
            .app
            .read(cx)
            .comparison_alignment()
            .and_then(|group| group.viewport(1, cx))
            .context("comparison viewport missing")?;
        input::wheel(window, viewport.center(), -180., cx);
        // The staging list scrolls independently; the commit field stays fixed beneath it.
        input::wheel(
            window,
            input::tree(window, "staging")?.bounds().center(),
            -140.,
            cx,
        );
        Ok::<_, anyhow::Error>(())
    })??;
    h.frame(h.main, cx).await?;
    h.main.update(cx, |_, window, cx| {
        let fixed_after = input::field(window, "commit-message")?.bounds();
        ensure!((fixed_after.top() - fixed_before.top()).abs() <= px(1.), "tree scrolling moved the fixed commit form");
        let group = h.app.read(cx).comparison_alignment().context("comparison unmounted during wheel")?;
        ensure!(group.scroll_offset(cx) > px(0.) || original.read(cx).scroll_offset() != before, "real wheel did not scroll the comparison");
        h.record("independent_scroll_and_fixed_form", serde_json::json!({"form_before": format!("{fixed_before:?}"), "form_after": format!("{fixed_after:?}"), "source_scroll": format!("{:?}", group.scroll_offset(cx))}));
        // Return the comparison to its first visible changes using the native keyboard route.
        editor_ui::testing::press_key(window, "ctrl-home", cx);
        Ok::<_, anyhow::Error>(())
    })??;
    // IME/Undo exercises advance the original document's capability revision. Follow the same
    // explicit save-and-refresh workflow as a user before approving a newly sampled index patch.
    h.main.update(cx, |_, window, cx| {
        editor_ui::testing::press_key(window, "ctrl-s", cx)
    })?;
    h.until(
        h.main,
        "reviewed original document saved",
        |_, cx| {
            Ok(h.app
                .read(cx)
                .tabs
                .iter()
                .any(|tab| tab.owns_editor(&original) && !tab.is_dirty()))
        },
        cx,
    )
    .await?;
    h.command("refresh-hunks", serde_json::Value::Null, cx)?;
    h.settle(cx).await?;
    tree_projection::staging(h, cx).await?;
    h.record("saved_and_refreshed_before_staging", true.into());
    h.capture(h.main, "workbench.png", cx)?;
    Ok(())
}

/// A visible hunk action creates a partial index, then the actual textarea and Commit button submit index only.
async fn stage_and_commit(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.click_button(h.main, "commit-hunks", cx).await?;
    h.click_button(h.main, "hunk-0", cx).await?;
    let reviewed = h.fixture.review.clone();
    h.until_repository(
        "partial hunk written to the real index",
        move |root| {
            let staged = fixture::git_text(root, &["show", ":src/a-review.txt"])?;
            let baseline = fixture::git_text(root, &["show", "HEAD:src/a-review.txt"])?;
            Ok(staged != baseline && staged != reviewed)
        },
        cx,
    )
    .await?;
    h.until(
        h.main,
        "partial index remains in the changes tree",
        |window, _| {
            Ok(input::tree_item(window, "staging", h.case.label("更改", "Changes")).is_ok())
        },
        cx,
    )
    .await?;
    h.record("partial_index_groups", true.into());
    h.click_button(h.main, "stage", cx).await?;
    let reviewed = h.fixture.review.clone();
    h.until_repository(
        "whole reviewed file written to the real index",
        move |root| Ok(fixture::git_text(root, &["show", ":src/a-review.txt"])? == reviewed),
        cx,
    )
    .await?;
    h.settle(cx).await?;
    h.main.update(cx, |_, window, cx| {
        input::click(window, &input::field(window, "commit-message")?, cx)?;
        editor_ui::testing::input_text(window, "Native UI commit 中文", cx);
        Ok::<_, anyhow::Error>(())
    })??;
    h.until(h.main, "commit message publication", |_, _| {
        let state = h.worker.state.lock().unwrap();
        Ok(state.views.get("git/staging").and_then(|view| view.active_node("commit-message")).is_some_and(|node| {
            matches!(&node.kind, Kind::Textarea(input) if input.value == "Native UI commit 中文")
        }))
    }, cx).await?;
    commit_index(h, cx).await
}

/// Count the native Commit only after a new commit has the exact staged tree and user message.
/// The pre-existing HEAD and untracked files must not masquerade as an asynchronous completion.
async fn commit_index(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let root = h.fixture.workspace.clone();
    let previous = cx
        .background_executor()
        .spawn(async move { fixture::git_text(&root, &["rev-parse", "HEAD"]) })
        .await?;
    h.click_button(h.main, "commit", cx).await?;
    let old_head = previous.trim().to_owned();
    let reviewed = h.fixture.review.clone();
    h.until_repository(
        "native index-only commit completed",
        move |root| {
            let actual = fixture::git_text(root, &["rev-parse", "HEAD"])?;
            if actual.trim() == old_head {
                return Ok(false);
            }
            ensure!(
                fixture::git_text(root, &["show", "-s", "--format=%s", "HEAD"])?.trim()
                    == "Native UI commit 中文",
                "native Commit message differs from the native textarea"
            );
            ensure!(
                fixture::git_text(root, &["rev-parse", "HEAD^"])?.trim() == old_head,
                "native Commit has an unexpected parent"
            );
            ensure!(
                fixture::git_text(root, &["show", "HEAD:src/a-review.txt"])? == reviewed,
                "native Commit did not submit the selected index content"
            );
            ensure!(
                fixture::git_text(
                    root,
                    &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]
                )?
                .trim()
                    == fixture::REVIEW_PATH,
                "native Commit included a path outside the reviewed index"
            );
            ensure!(
                fixture::git_text(root, &["diff", "--cached", "--name-only"])?.is_empty(),
                "native Commit left reviewed changes in the index"
            );
            let untracked =
                fixture::git_text(root, &["ls-files", "--others", "--exclude-standard"])?;
            ensure!(
                untracked.lines().count() == 28
                    && untracked
                        .lines()
                        .all(|path| path.starts_with("src/z-note-") && root.join(path).is_file()),
                "native Commit consumed an untracked worktree file"
            );
            Ok(true)
        },
        cx,
    )
    .await?;
    h.record(
        "native_index_commit",
        serde_json::json!({"index_only": true, "message": "Native UI commit 中文", "previous_head": previous.trim(), "untracked_files_retained": 28}),
    );
    Ok(())
}

/// A real merge is discovered after the UI has committed. Result actions address the existing original
/// EditorState; history stays open while review, Chinese IME and Undo operate in the three-pane window.
async fn conflicts(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let root = h.fixture.workspace.clone();
    let (base, previous, incoming) = cx
        .background_executor()
        .spawn(async move {
            Fixture::enter_conflict(&root)?;
            Ok::<_, anyhow::Error>((
                fixture::git_text(&root, &["show", ":1:conflicts/merge.txt"])?,
                fixture::git_text(&root, &["rev-parse", "HEAD"])?,
                fixture::git_text(&root, &["rev-parse", "MERGE_HEAD"])?,
            ))
        })
        .await?;
    h.click_button(h.main, "refresh", cx).await?;
    h.until(
        h.main,
        "refreshed real conflict tree",
        |window, _| {
            Ok(input::tree_item(window, "staging", h.case.label("冲突", "Conflicts")).is_ok())
        },
        cx,
    )
    .await?;
    h.command(
        "resolve-file",
        serde_json::json!({"path": fixture::CONFLICT_PATH}),
        cx,
    )?;
    let separate = work_window(h, "conflicts", cx).await?;
    h.dpi(separate, cx).await?;
    h.settle(cx).await?;
    // A command may reveal the window with a collapsed tree. Exercise its actual directory
    // and file navigation too, so the screenshot and result cannot bypass the left sidebar.
    h.expand(separate, "conflicts", h.case.label("冲突", "Conflicts"), cx)
        .await?;
    h.select_file(separate, "conflicts", "merge.txt", cx)
        .await?;
    h.until(
        separate,
        "conflict sidebar selected the real file",
        |window, _| {
            Ok(input::tree_item(window, "conflicts", "merge.txt")
                .is_ok_and(|item| item.selected() == Some(true)))
        },
        cx,
    )
    .await?;
    h.record(
        "conflict_tree_navigation",
        serde_json::json!({"relative_path": fixture::CONFLICT_PATH, "native_selected": true}),
    );
    h.until(
        separate,
        "three conflict sources",
        |window, _| {
            Ok(input::source(window, "snapshot-0").is_ok()
                && input::source(window, "snapshot-2").is_ok())
        },
        cx,
    )
    .await?;
    let original = cx
        .update(|cx| {
            h.app
                .read(cx)
                .tabs
                .iter()
                .find(|tab| tab.path().ends_with(fixture::CONFLICT_PATH))
                .and_then(|tab| tab.text.as_ref())
                .map(|text| text.editor.clone())
        })
        .context("original conflict document not opened")?;
    apply_conflict_proposals(h, separate, &original, base, cx).await?;
    // Edge actions own button focus. Re-enter the actually painted editable result with a
    // native pointer gesture, so composition cannot bypass a broken result-pane focus route.
    h.until(
        separate,
        "editable conflict result",
        |window, _| {
            Ok(input::source(window, "merge-result")
                .is_ok_and(|source| source.role() == Some(Role::TextInput)))
        },
        cx,
    )
    .await?;
    separate.update(cx, |_, window, cx| {
        input::click(window, &input::source(window, "merge-result")?, cx)?;
        Ok::<_, anyhow::Error>(())
    })??;
    // Edge buttons own the prior focus. Wait for this one projected click to reach the sole
    // original editor; never repair the assertion by assigning focus or sending another click.
    h.until(
        separate,
        "conflict pointer focused the original editor",
        |window, cx| {
            Ok(original.focus_handle(cx).is_focused(window)
                && input::source(window, "merge-result")?.focused() == Some(true))
        },
        cx,
    )
    .await?;
    h.frame(separate, cx).await?;
    ime::exercise(h, separate, &original, false, cx).await?;
    // Undo restores the same bytes and proposal ownership is sampled again before Save and resolve.
    h.settle(cx).await?;
    let titles = source_titles(h, "conflicts");
    ensure!(
        titles
            .iter()
            .any(|text| text.contains(h.case.label("来源 2", "Stage 2")))
            && titles
                .iter()
                .any(|text| text.contains(h.case.label("来源 3", "Stage 3"))),
        "conflict source titles do not identify stages: {titles:?}"
    );
    h.record("conflict_sources", serde_json::json!({"titles": titles, "same_original_entity": format!("{:?}", original.entity_id()), "history_window_retained": cx.update(|cx| h.app.read(cx).plugin_windows.contains_key("git/commit-details"))}));
    tree_projection::conflicts(h, separate, cx).await?;
    h.capture(separate, "conflicts.png", cx)?;
    let manual_review = {
        let state = h.worker.state.lock().unwrap();
        state
            .views
            .get("git/conflicts")
            .and_then(|view| view.active_node("merge-block-0-confirm"))
            .is_some_and(|node| !node.disabled)
    };
    if manual_review {
        h.click_button(separate, "merge-block-0-confirm", cx)
            .await?;
    }
    finish_conflict(h, separate, previous, incoming, cx).await
}

/// Each proposal must update the sole original EditorState before the next native action. Its
/// source bytes come from the actual stage-1 object and the fixture's existing merge expectation.
async fn apply_conflict_proposals(
    h: &Harness,
    target: AnyWindowHandle,
    original: &Entity<EditorState>,
    base: String,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let nonconflicting = Fixture::merged_text().replace("local block", "base conflict");
    for (action, expected) in [
        ("merge-from-base", base),
        ("merge-nonconflicting", nonconflicting),
        ("merge-block-0-left", Fixture::merged_text()),
    ] {
        h.click_button(target, action, cx).await?;
        let mut last_result = None;
        h.until(
            target,
            action,
            |_, cx| {
                let value = original.read(cx).value();
                let matches = value.as_ref() == expected;
                let facts = serde_json::json!({
                    "matches_expected": matches, "actual_bytes": value.len(),
                    "expected_bytes": expected.len(), "markers_present": value.contains("<<<<<<<"),
                    "source_current": h.node_current(target, action, cx)
                });
                if last_result.as_ref() != Some(&facts) {
                    h.checkpoint(&format!("native_proposal_{action}_result"), facts.clone())?;
                    last_result = Some(facts);
                }
                Ok(matches)
            },
            cx,
        )
        .await?;
    }
    Ok(())
}

/// Saving resolves the actual index before Continue may create a merge commit. Verify both real
/// parents, not merely the absence of a marker or an old HEAD read before the actor handled input.
async fn finish_conflict(
    h: &Harness,
    separate: AnyWindowHandle,
    previous: String,
    incoming: String,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    h.click_button(separate, "resolved", cx).await?;
    h.until_repository(
        "saved conflict result staged without unmerged entries",
        |root| {
            if !fixture::git_text(root, &["ls-files", "-u", "--", fixture::CONFLICT_PATH])?
                .is_empty()
            {
                return Ok(false);
            }
            ensure!(
                fs::read_to_string(root.join(fixture::CONFLICT_PATH))? == Fixture::merged_text(),
                "native resolve saved an unexpected result"
            );
            ensure!(
                fixture::git_text(root, &["show", ":conflicts/merge.txt"])?
                    == Fixture::merged_text(),
                "native resolve staged a different result from the document"
            );
            Ok(true)
        },
        cx,
    )
    .await?;
    h.click_button(separate, "continue", cx).await?;
    let local_parent = previous.trim().to_owned();
    let remote_parent = incoming.trim().to_owned();
    h.until_repository(
        "native Continue produced the actual two-parent merge",
        move |root| {
            if root.join(".git/MERGE_HEAD").exists()
                || fixture::git_text(root, &["rev-parse", "HEAD"])?.trim() == local_parent
            {
                return Ok(false);
            }
            let parents = fixture::git_text(root, &["show", "-s", "--format=%P", "HEAD"])?;
            ensure!(
                parents.split_whitespace().collect::<Vec<_>>()
                    == [local_parent.as_str(), remote_parent.as_str()],
                "native Continue produced an unexpected merge parent list"
            );
            ensure!(
                fixture::git_text(root, &["ls-files", "-u"])?.is_empty(),
                "unmerged entries remain"
            );
            ensure!(
                fixture::git_text(root, &["show", "HEAD:conflicts/merge.txt"])?
                    == Fixture::merged_text(),
                "merge commit does not retain the reviewed result"
            );
            Ok(true)
        },
        cx,
    )
    .await?;
    h.settle(cx).await?;
    h.record("native_conflict_continue", serde_json::json!({"completed": true, "parents": [previous.trim(), incoming.trim()], "result_matches_review": true}));
    Ok(())
}

async fn work_window(
    h: &Harness,
    panel: &str,
    cx: &mut AsyncApp,
) -> anyhow::Result<AnyWindowHandle> {
    let key = format!("git/{panel}");
    h.until(
        h.main,
        panel,
        |_, cx| Ok(h.app.read(cx).plugin_windows.contains_key(&key)),
        cx,
    )
    .await?;
    cx.update(|cx| {
        h.app
            .read(cx)
            .plugin_windows
            .get(&key)
            .copied()
            .map(AnyWindowHandle::from)
    })
    .context("work window vanished")
}

/// The visible source titles are guest-provided domain facts, independent from native geometry.
fn source_titles(h: &Harness, panel: &str) -> Vec<String> {
    fn collect(node: &Node, values: &mut Vec<String>) {
        if node.id.starts_with("pane-title-") || node.id == "result-title" {
            if let Kind::Text { text } = &node.kind {
                values.push(text.clone());
            }
        }
        if let Kind::Column { children } | Kind::Row { children } = &node.kind {
            for child in children {
                collect(child, values);
            }
        }
    }
    let mut titles = Vec::new();
    if let Some(view) = h
        .worker
        .state
        .lock()
        .unwrap()
        .views
        .get(&format!("git/{panel}"))
    {
        collect(&view.root, &mut titles);
    }
    titles
}
