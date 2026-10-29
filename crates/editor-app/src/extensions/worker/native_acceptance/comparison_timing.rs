//! Compare real Windows file-navigation latency with the unchanged production guest and frame loop.
use super::*;
use gpui_kit::Role;

/// The dedicated receipt cannot shorten any other history or full-workbench acceptance case.
pub(super) fn requested() -> bool {
    std::env::var_os("NANOBUG_GIT_NATIVE_DIFF_TIMING").is_some()
}

/// Exact two-line snapshots keep algorithm and large-file layout work outside this first experiment.
fn content(name: &str, version: &str) -> String {
    let answer = if version == "base" { 41 } else { 42 };
    format!("# {name} {version}\nanswer = {answer}\n")
}

/// Commit only the owned timing files; existing fixture review changes remain uncommitted.
fn prepare(root: &Path) -> anyhow::Result<()> {
    for version in ["base", "changed"] {
        for name in ["a.novel", "b.novel", "c.novel"] {
            fs::write(root.join(name), content(name, version))?;
            fixture::git(root, &["add", "--", name])?;
        }
        fixture::git(root, &["commit", "-m", version])?;
    }
    Ok(())
}

/// Both actual sources and admitted native geometry must belong to the requested comparison.
fn displayed(h: &Harness, name: &str, window: &Window, cx: &App) -> bool {
    let app = h.app.read(cx);
    if app.editor.read(cx).value().to_string() != content(name, "changed")
        || !app
            .comparison_alignment()
            .is_some_and(|group| group.project_range(1, 0..1, cx).is_some())
    {
        return false;
    }
    let nodes = gpui_base::test_support::snapshots(window);
    ["base", "changed"].into_iter().all(|version| {
        let expected = content(name, version);
        nodes.iter().any(|node| {
            node.visible()
                && node.role() == Some(Role::Document)
                && node.value() == Some(expected.as_str())
                && node.bounds().size.width > px(80.)
        })
    })
}

/// Dispatch the requested physical control, never a direct editor or guest comparison command.
fn click(action: &str, name: &str, window: &mut Window, cx: &mut App) -> anyhow::Result<()> {
    let target = if action == "select" {
        input::tree_item(window, "files", name)?
    } else {
        let id = if action == "next" {
            "diff-file-next"
        } else {
            "diff-file-previous"
        };
        gpui_base::test_support::snapshots(window)
            .into_iter()
            .find(|node| {
                node.role() == Some(Role::Button) && node.visible() && input::named(node, id)
            })
            .with_context(|| format!("native comparison control {id} not painted"))?
    };
    // Base button clicks use the native hover target as well as the press/release positions.
    window.activate_window();
    window.dispatch_event(
        PlatformInput::MouseMove(gpui_kit::MouseMoveEvent {
            position: target.bounds().center(),
            pressed_button: None,
            modifiers: Default::default(),
        }),
        cx,
    );
    input::click(window, &target, cx)
}

/// Observe two cache misses followed by 32 cached previous/next/tree selections in each palette.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let root = h.fixture.workspace.clone();
    cx.background_executor()
        .spawn(async move { prepare(&root) })
        .await?;
    h.activate(cx).await?;
    h.main.update(cx, |_, window, cx| {
        let branches = input::tree(window, "branches")?.bounds();
        input::point_click(
            window,
            point(branches.right() + px(60.), branches.top() + px(14.)),
            cx,
        )
    })??;
    h.until(
        h.main,
        "timing files in native history tree",
        |window, _| Ok(input::tree_item(window, "files", "a.novel").is_ok()),
        cx,
    )
    .await?;
    h.select_file(h.main, "files", "a.novel", cx).await?;
    h.until(
        h.main,
        "initial timing comparison painted",
        |window, cx| Ok(displayed(h, "a.novel", window, cx)),
        cx,
    )
    .await?;

    // Visible text can precede the comparison receipt; start after its owning scene is current.
    h.settle(cx).await?;
    let mut targets = vec![("next", "b.novel", false), ("next", "c.novel", false)];
    for _ in 0..8 {
        targets.extend([
            ("previous", "b.novel", true),
            ("select", "a.novel", true),
            ("next", "b.novel", true),
            ("next", "c.novel", true),
        ]);
    }
    let mut samples = Vec::new();
    for (action, name, warm) in targets {
        let before = cx.update(|_| crate::session_state::save_timings());
        let start = Instant::now();
        h.main
            .update(cx, |_, window, cx| click(action, name, window, cx))??;
        let mut saw_git = false;
        h.until(
            h.main,
            &format!("timed {action} {name} comparison painted"),
            |window, cx| {
                saw_git |= h
                    .worker
                    .state
                    .lock()
                    .unwrap()
                    .processes
                    .get("git")
                    .copied()
                    .unwrap_or(0)
                    > 0;
                Ok(displayed(h, name, window, cx))
            },
            cx,
        )
        .await?;
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.;
        let saves = cx.update(|_| crate::session_state::save_timings().since(before));
        let sample = serde_json::json!({
            "action": action, "file": name, "warm": warm,
            "elapsed_ms": elapsed_ms, "session_saves": saves, "git_process_observed": saw_git,
        });
        eprintln!("native diff switch: {sample}");
        samples.push(sample);
        // Model independent browsing, not burst input: reading time is outside click latency.
        // Readiness above still requires both exact texts and actual admitted native geometry.
        cx.background_executor()
            .timer(Duration::from_millis(200))
            .await;
    }
    // Evidence writes stay outside timed navigation; samples contain only fixed fixture content.
    let result = serde_json::json!({
        "case": h.case.name, "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "measurement": "physical click to both exact source texts painted with admitted geometry",
        "includes_guest_startup": false, "samples": samples,
        "inter_action_reading_ms": 200,
    });
    fs::write(
        h.fixture.output.join("diff-switch-timing.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    h.checkpoint("diff_switch_timing", result)?;
    h.capture(h.main, "diff-switch-timing.png", cx)?;
    Ok(())
}
