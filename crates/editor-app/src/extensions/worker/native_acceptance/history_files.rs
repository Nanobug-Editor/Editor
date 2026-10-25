//! Actual counted history trees retain native disclosure, selection and scroll while opening exact sources.
use super::*;
use gpui_kit::Role;
use plugin_runtime::plugin_protocol::ui::Kind;

/// This subcase stays inside the history-only diagnostic and cannot shorten the full workbench run.
pub(super) fn requested() -> bool {
    std::env::var_os("NANOBUG_GIT_NATIVE_HISTORY_FILES").is_some()
}

/// Read this panel's public tree resource; no test API creates nodes or changes native state.
fn snapshot(h: &Harness) -> anyhow::Result<plugin_runtime::TreeSnapshot> {
    let state = h.worker.state.lock().unwrap();
    let Kind::Tree { handle } = &state.views["git/history"]
        .active_node("files")
        .context("files scene")?
        .kind
    else {
        return Err(anyhow!("files is not a native tree"));
    };
    Ok(state
        .trees
        .iter()
        .find(|tree| tree.handle() == handle)
        .context("files resource")?
        .snapshot())
}

/// Pointer input hits the first real canvas row beside the branch tree, within its painted dock.
fn select_latest(h: &Harness, window: &mut Window, cx: &mut App) -> anyhow::Result<()> {
    let branches = input::tree(window, "branches")?.bounds();
    let files = input::tree(window, "files")?.bounds();
    ensure!(
        files.left() - branches.right() > px(80.),
        "history canvas has no room"
    );
    input::point_click(
        window,
        point(branches.right() + px(60.), branches.top() + px(14.)),
        cx,
    )
}

/// This fixture commits only its own new paths; existing review/conflict worktree bytes remain intact.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    ensure!(std::env::var_os("NANOBUG_GIT_NATIVE_HISTORY_ONLY").is_some());
    let root = h.fixture.workspace.clone();
    let oid = cx
        .background_executor()
        .spawn(async move {
            let mut paths = vec![
                "crates/editor-app/src/a.rs".to_owned(),
                "crates/editor-app/src/b.rs".to_owned(),
                "crates/editor-ui/theme.rs".to_owned(),
                "docs/plugins/verification/readme.md".to_owned(),
                "history-tree-root.txt".to_owned(),
            ];
            paths.extend((0..28).map(|i| format!("crates/editor-app/src/note-{i:02}.rs")));
            for path in &paths {
                let file = root.join(path);
                fs::create_dir_all(file.parent().unwrap())?;
                fs::write(file, format!("immutable {path}\n"))?;
                fixture::git(&root, &["add", "--", path])?;
            }
            fixture::git(&root, &["commit", "-m", "Counted compact history tree"])?;
            Ok::<_, anyhow::Error>(
                fixture::git_text(&root, &["rev-parse", "HEAD"])?
                    .trim()
                    .to_owned(),
            )
        })
        .await?;
    if let Err(error) = h.activate(cx).await {
        // Preserve the first actor failure, before subsequent paused-instance callbacks obscure it.
        // This bounded diagnostic observes only our disposable fixture's existing log records.
        let state = h.worker.state.lock().unwrap();
        let first = state.logs.records("git").into_iter()
            .find(|record| record.level > plugin_runtime::logs::LogLevel::Info)
            .map(|record| serde_json::json!({"source": record.source, "message": record.message, "level": format!("{:?}", record.level)}));
        h.record(
            "history_files_activation_fault",
            serde_json::json!({"first": first}),
        );
        return Err(error);
    }
    h.main
        .update(cx, |_, window, cx| select_latest(h, window, cx))??;
    h.until(
        h.main,
        "latest immutable file set",
        |_, _| {
            let state = h.worker.state.lock().unwrap();
            Ok(state.views["git/history"]
                .active_node("history-oid")
                .is_some_and(|node| matches!(&node.kind, Kind::Text { text } if text == &oid)))
        },
        cx,
    )
    .await?;
    let total = h.case.label("33 个文件", "33 files");
    h.until(
        h.main,
        "expanded counted root",
        |window, _| {
            Ok(input::tree_item(window, "files", total)
                .is_ok_and(|row| row.expanded() == Some(true)))
        },
        cx,
    )
    .await?;
    h.expand(h.main, "files", "crates", cx).await?;
    h.expand(h.main, "files", "editor-app\\src", cx).await?;
    h.until(
        h.main,
        "real compact leaf",
        |window, _| Ok(input::tree_item(window, "files", "a.rs").is_ok()),
        cx,
    )
    .await?;
    let initial = snapshot(h)?;
    let root_id = initial.branches[&None].children[0].clone();
    let root_node = &initial.nodes[&root_id];
    ensure!(root_node.muted && root_node.label == total);
    ensure!(
        initial
            .nodes
            .values()
            .any(|node| node.label == "crates"
                && node.suffix == h.case.label("31 个文件", "31 files"))
    );
    ensure!(
        initial
            .nodes
            .values()
            .any(|node| node.label == "editor-app\\src"
                && node.suffix == h.case.label("30 个文件", "30 files"))
    );
    ensure!(
        initial
            .nodes
            .values()
            .any(|node| node.label == "docs\\plugins\\verification"
                && node.suffix == h.case.label("1 个文件", "1 file"))
    );
    h.capture(h.main, "history-files-expanded.png", cx)?;

    // One actual row press gives Base focus, then Left/Right exercise its ordinary keyboard disclosure.
    h.main.update(cx, |_, window, cx| {
        let row = input::tree_item(window, "files", total)?;
        input::click(window, &row, cx)?;
        Ok::<_, anyhow::Error>(())
    })??;
    h.until(
        h.main,
        "total owns native keyboard selection",
        |window, _| {
            Ok(input::tree_item(window, "files", total)
                .is_ok_and(|row| row.selected() == Some(true)))
        },
        cx,
    )
    .await?;
    h.main.update(cx, |_, window, cx| {
        editor_ui::testing::press_key(window, "left", cx)
    })?;
    h.until(
        h.main,
        "keyboard collapsed total",
        |window, _| {
            Ok(input::tree_item(window, "files", total)
                .is_ok_and(|row| row.expanded() == Some(false)))
        },
        cx,
    )
    .await?;
    h.command("refresh", serde_json::json!({}), cx)?;
    h.settle(cx).await?;
    h.main.update(cx, |_, window, cx| {
        ensure!(
            input::tree_item(window, "files", total)?.expanded() == Some(false),
            "refresh reopened user's collapsed root"
        );
        // Refresh may withdraw focus while loading; focus the row through ordinary pointer input.
        input::click(window, &input::tree_item(window, "files", total)?, cx)?;
        Ok::<_, anyhow::Error>(())
    })??;
    h.frame(h.main, cx).await?;
    h.main.update(cx, |_, window, cx| {
        editor_ui::testing::press_key(window, "right", cx)
    })?;
    h.until(
        h.main,
        "keyboard reopened total",
        |window, _| {
            Ok(input::tree_item(window, "files", total)
                .is_ok_and(|row| row.expanded() == Some(true)))
        },
        cx,
    )
    .await?;
    // Collapse cancels descendant disclosure under the existing shared-tree contract.
    h.expand(h.main, "files", "crates", cx).await?;
    h.expand(h.main, "files", "editor-app\\src", cx).await?;
    h.select_file(h.main, "files", "a.rs", cx).await?;
    h.until(
        h.main,
        "original compact path in native readonly comparison",
        |window, cx| {
            let painted = h
                .app
                .read(cx)
                .comparison_alignment()
                .is_some_and(|group| group.project_range(1, 0..1, cx).is_some());
            let source = gpui_base::test_support::snapshots(window)
                .into_iter()
                .any(|node| {
                    node.visible()
                        && node.role() == Some(Role::Document)
                        && node.value() == Some("immutable crates/editor-app/src/a.rs\n")
                        && node.bounds().size.width > px(100.)
                });
            Ok(painted && source)
        },
        cx,
    )
    .await?;
    h.capture(h.main, "history-files-exact-source.png", cx)?;

    let before = h.main.update(cx, |_, window, _| {
        let row = input::tree_item(window, "files", "a.rs")?;
        ensure!(row.selected() == Some(true));
        Ok::<_, anyhow::Error>(row.bounds().top() - input::tree(window, "files")?.bounds().top())
    })??;
    h.main.update(cx, |_, window, cx| {
        input::wheel(
            window,
            input::tree(window, "files")?.bounds().center(),
            -40.,
            cx,
        );
        Ok::<_, anyhow::Error>(())
    })??;
    h.until(
        h.main,
        "history files native wheel",
        |window, _| {
            Ok(input::tree_item(window, "files", "a.rs").is_ok_and(|row| {
                row.bounds().top() - input::tree(window, "files").unwrap().bounds().top()
                    < before - px(1.)
            }))
        },
        cx,
    )
    .await?;
    let mut previous = None;
    let mut stable = 0;
    h.until(
        h.main,
        "history files smooth wheel settled",
        |window, _| {
            let offset = input::tree_item(window, "files", "a.rs")?.bounds().top()
                - input::tree(window, "files")?.bounds().top();
            stable = if previous.is_some_and(|last: Pixels| (last - offset).abs() <= px(0.05)) {
                stable + 1
            } else {
                0
            };
            previous = Some(offset);
            Ok(stable >= 3)
        },
        cx,
    )
    .await?;
    let scrolled = h.main.update(cx, |_, window, _| {
        Ok::<_, anyhow::Error>(
            input::tree_item(window, "files", "a.rs")?.bounds().top()
                - input::tree(window, "files")?.bounds().top(),
        )
    })??;
    let retained = snapshot(h)?;
    h.command("refresh", serde_json::json!({}), cx)?;
    h.settle(cx).await?;
    h.main.update(cx, |_, window, _| {
        let row = input::tree_item(window, "files", "a.rs")?;
        let after = row.bounds().top() - input::tree(window, "files")?.bounds().top();
        ensure!(
            row.selected() == Some(true) && (after - scrolled).abs() <= px(1.),
            "refresh lost native selection/scroll"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    let after = snapshot(h)?;
    ensure!(
        after.epoch == retained.epoch && after.expanded == retained.expanded,
        "refresh reset tree resource/disclosure"
    );
    h.record("history_files", serde_json::json!({
        "commit": oid, "total": total, "count_includes_unloaded_files": true,
        "compact_directory": "editor-app\\src", "exact_source_path": "crates/editor-app/src/a.rs",
        "keyboard_disclosure": true, "collapsed_root_retained": true,
        "selection_retained": true, "scroll_retained": true, "resource_epoch": after.epoch,
    }));
    h.capture(h.main, "history-files-refreshed.png", cx)?;
    Ok(())
}
