//! Real native wheel input must reveal and select the actual Git root after several lazy pages.
use super::*;
use plugin_runtime::plugin_protocol::Paint;

/// Read the guest's accepted range and root text position, independently of the native input entity.
fn root_visible(h: &Harness, subject: &str, height: f32, total: usize) -> bool {
    let state = h.worker.state.lock().unwrap();
    let Some(Kind::Canvas(canvas)) = state
        .views
        .get("git/history")
        .and_then(|view| view.active_node("log"))
        .map(|node| &node.kind)
    else {
        return false;
    };
    canvas.scroll.as_ref().is_some_and(|range| {
        (range.content - total as f32 * 28.).abs() < 1.
            && (range.offset + height - range.content).abs() < 1.
    }) && canvas.paint.iter().any(|paint| {
        // Titles remain ordinary left-aligned text; fitting a narrow column can add an ellipsis.
        matches!(paint,
            Paint::Text { text, y, .. }
                if !text.is_empty() && subject.starts_with(text.trim_end_matches('…'))
                    && *y >= 0. && *y + 18. <= height
        )
    })
}

/// Read the fixture's real count, root OID and subject off the UI thread through `cx`.
async fn repository_history(
    h: &Harness,
    cx: &mut AsyncApp,
) -> anyhow::Result<(usize, String, String)> {
    let root = h.fixture.workspace.clone();
    cx.background_executor()
        .spawn(async move {
            let total = fixture::git_text(&root, &["rev-list", "--count", "--all"])?;
            let oldest = fixture::git_text(&root, &["rev-list", "--max-parents=0", "--all"])?;
            let subject = fixture::git_text(&root, &["show", "-s", "--format=%s", oldest.trim()])?;
            Ok::<_, anyhow::Error>((
                total.trim().parse::<usize>()?,
                oldest.trim().to_owned(),
                subject.trim().to_owned(),
            ))
        })
        .await
}

/// Exercise current painted wheel targets; verify the final row and its clicked OID against Git.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let (total, oldest, subject) = repository_history(h, cx).await?;
    ensure!(
        total > 384,
        "native pagination fixture is not longer than three pages"
    );
    let bounds = h
        .main
        .update(cx, |_, window, cx| scrollbar::viewport(h, window, cx))??;
    let height = bounds.size.height / px(1.);
    let at = point(bounds.left() + px(120.), bounds.center().y);
    let selected_before = selected(h);
    let mut gestures = 0;
    let mut stalled = 0;
    for _ in 0..64 {
        if root_visible(h, &subject, height, total) {
            break;
        }
        h.until(
            h.main,
            "current history wheel target",
            |_, cx| Ok(h.node_current(h.main, "log", cx)),
            cx,
        )
        .await?;
        let before = log_offset(h);
        h.main
            .update(cx, |_, window, cx| input::wheel(window, at, -10_000., cx))?;
        gestures += 1;
        h.settle(cx).await?;
        // Several settled boundary gestures are a sharper failure signal than repeating 64 no-ops.
        stalled = if log_offset(h) == before {
            stalled + 1
        } else {
            0
        };
        if stalled == 3 {
            break;
        }
    }
    ensure!(
        root_visible(h, &subject, height, total),
        "native scrolling stopped before the repository root: commits={total}, offset={}",
        log_offset(h)
    );
    ensure!(
        selected(h) == selected_before,
        "pagination changed the selected commit"
    );
    let at = point(bounds.left() + px(24.), bounds.bottom() - px(14.));
    h.main
        .update(cx, |_, window, cx| input::point_click(window, at, cx))??;
    h.until(
        h.main,
        "clicked root's immutable preview",
        |_, _| Ok(selected(h).as_deref() == Some(oldest.as_str())),
        cx,
    )
    .await?;
    h.record(
        "history_pagination",
        serde_json::json!({
            "actual_git_count": total, "oldest_oid": oldest, "oldest_subject": subject,
            "wheel_gestures": gestures, "complete_root_row_visible": true,
            "scroll_selection_retained": true, "root_click_matches_git": true,
        }),
    );
    h.capture(h.main, "history-scroll.png", cx)?;
    Ok(())
}
