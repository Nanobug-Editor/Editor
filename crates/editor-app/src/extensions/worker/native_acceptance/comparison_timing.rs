//! Compare real Windows file-navigation latency with the unchanged production guest and frame loop.
use super::*;
use gpui_base::input::RopeExt as _;
use gpui_kit::Role;

/// The dedicated receipt cannot shorten any other history or full-workbench acceptance case.
pub(super) fn requested() -> bool {
    std::env::var_os("NANOBUG_GIT_NATIVE_DIFF_TIMING").is_some()
}

/// Exact snapshots use two lines by default, with an optional bounded size for layout measurements.
fn content(name: &str, version: &str) -> String {
    // Read external immutable fixtures once, outside navigation timing. A filename header
    // distinguishes adjacent copies so an old visible comparison cannot satisfy readiness.
    if std::env::var_os("NANOBUG_DIFF_TIMING_SOURCE").is_some() {
        static SOURCES: std::sync::OnceLock<[String; 2]> = std::sync::OnceLock::new();
        let sources = SOURCES.get_or_init(|| {
            let root = PathBuf::from(std::env::var_os("NANOBUG_DIFF_TIMING_SOURCE").unwrap());
            ["before.rs", "after.rs"].map(|file| fs::read_to_string(root.join(file)).unwrap())
        });
        return format!(
            "// timing fixture: {name}\n{}",
            sources[usize::from(version != "base")]
        );
    }
    let answer = if version == "base" { 41 } else { 42 };
    let mut text = format!("# {name} {version}\nanswer = {answer}\n");
    // Use the same real grammar and navigation for bounded large-source measurements.
    let lines = std::env::var("NANOBUG_DIFF_TIMING_LINES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(2)
        .clamp(2, 8000);
    for line in 2..lines {
        text.push_str(&format!("key_{line:05} = {line}\n"));
    }
    text
}

/// Real Rust snapshots use their native extension; the default synthetic fixture stays compatible.
fn names() -> [&'static str; 3] {
    if std::env::var_os("NANOBUG_DIFF_TIMING_SOURCE").is_some() {
        ["a-manager.rs", "b-manager.rs", "c-manager.rs"]
    } else {
        ["a.novel", "b.novel", "c.novel"]
    }
}

/// Commit only the owned timing files; existing fixture review changes remain uncommitted.
fn prepare(root: &Path) -> anyhow::Result<()> {
    for version in ["base", "changed"] {
        for name in names() {
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
        || !app.comparison_alignment().is_some_and(|group| {
            let native = app.editor.read(cx);
            native.visible_row_range().is_some_and(|_| {
                static FIRST_CHANGE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
                let row = *FIRST_CHANGE.get_or_init(|| {
                    content(name, "base")
                        .lines()
                        .zip(content(name, "changed").lines())
                        .take_while(|(left, right)| left == right)
                        .count()
                });
                let byte = native.text().line_start_offset(row);
                group.status(cx) == editor_ui::aligned_editor::AlignmentStatus::Ready
                    && group.project_range(1, byte..byte, cx).is_some()
            })
        })
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
    let [first, second, third] = names();
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
        |window, _| Ok(input::tree_item(window, "files", first).is_ok()),
        cx,
    )
    .await?;
    h.select_file(h.main, "files", first, cx).await?;
    h.until(
        h.main,
        "initial timing comparison painted",
        |window, cx| Ok(displayed(h, first, window, cx)),
        cx,
    )
    .await?;

    // Measure expanded content separately when requested, with the user's choice retained across files.
    if std::env::var_os("NANOBUG_DIFF_TIMING_EXPAND").is_some() {
        h.main.update(cx, |_, window, cx| {
            let button = gpui_base::test_support::snapshots(window)
                .into_iter()
                .find(|node| node.visible() && input::named(node, "diff-expand-unchanged"))
                .ok_or_else(|| anyhow::anyhow!("expand control missing"))?;
            input::point_click(window, button.bounds().center(), cx)
        })??;
    }
    // Visible text can precede the comparison receipt; start after its owning scene is current.
    h.settle(cx).await?;
    let mut targets = vec![("next", second, false), ("next", third, false)];
    for _ in 0..8 {
        targets.extend([
            ("previous", second, true),
            ("select", first, true),
            ("next", second, true),
            ("next", third, true),
        ]);
    }
    let mut samples = Vec::new();
    for (action, name, warm) in targets {
        let before = cx.update(|_| crate::session_state::save_timings());
        let reuse_before = cx.update(|cx| h.app.read(cx).comparison_source_cache.hits());
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
        let reused = cx.update(|cx| h.app.read(cx).comparison_source_cache.hits() - reuse_before);
        let sample = serde_json::json!({
            "action": action, "file": name, "warm": warm,
            "elapsed_ms": elapsed_ms, "session_saves": saves, "git_process_observed": saw_git,
            "native_sources_reused": reused,
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
        "folded_by_default": std::env::var_os("NANOBUG_DIFF_TIMING_EXPAND").is_none(),
    });
    fs::write(
        h.fixture.output.join("diff-switch-timing.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    h.checkpoint("diff_switch_timing", result.clone())?;
    h.capture(h.main, "diff-switch-timing.png", cx)?;
    verify_repeated_selection(h, third, cx).await?;
    // An explicit local target is asserted only after retaining every sample and screenshot.
    if let Ok(limit) = std::env::var("NANOBUG_DIFF_TIMING_MAX_MS") {
        let limit: f64 = limit.parse()?;
        let maximum = result["samples"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|sample| sample["elapsed_ms"].as_f64())
            .fold(0.0, f64::max);
        ensure!(
            maximum <= limit,
            "painted diff maximum {maximum:.2}ms exceeds {limit:.2}ms"
        );
    }
    Ok(())
}

/// Real repeated tree input must keep the existing live group, source identity, folds and viewport.
async fn verify_repeated_selection(
    h: &Harness,
    name: &str,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    h.settle(cx).await?;
    let (group, source) = cx.update(|cx| {
        let owner = h.app.read(cx);
        (
            owner.comparison_alignment().unwrap().clone(),
            owner.editor.clone(),
        )
    });
    cx.update(|cx| {
        group.reveal_content_edge(true, cx);
    });
    for _ in 0..4 {
        h.frame(h.main, cx).await?;
    }
    let offset = cx.update(|cx| group.scroll_offset(cx));
    h.select_file(h.main, "files", name, cx).await?;
    h.settle(cx).await?;
    cx.update(|cx| {
        ensure!(
            group.status(cx) == editor_ui::aligned_editor::AlignmentStatus::Ready,
            "repeated selection retired its original comparison"
        );
        ensure!(
            h.app.read(cx).editor == source,
            "repeated selection replaced the native source"
        );
        ensure!(
            (group.scroll_offset(cx) - offset).abs() < px(1.),
            "repeated selection reset its scroll"
        );
        Ok(())
    })?;
    h.checkpoint(
        "repeated_selection_kept_view",
        serde_json::json!({"file":name,"offset":f32::from(offset)}),
    )?;
    Ok(())
}
