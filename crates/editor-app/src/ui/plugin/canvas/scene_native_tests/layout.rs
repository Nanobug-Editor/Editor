//! Native long-line shaping must remain responsive within the public 64 KiB text budget.
use super::*;

/// Exercise the real local paragraph API on a maximum-sized original line, with a tiny visible clip.
pub(super) fn stress(window: &Window) -> anyhow::Result<()> {
    let text = "i".repeat(65536);
    let started = Instant::now();
    let paragraph = editor_ui::text::paragraph::CanvasParagraph::new(
        text.into(),
        font("Segoe UI"),
        px(18.),
        rgb(0).into(),
        Bounds::new(point(px(0.), px(0.)), size(px(80.), px(27.))),
        &protocol::ui::ParagraphStyle {
            wrap: false,
            ..Default::default()
        },
        window,
    )
    .map_err(anyhow::Error::msg)?;
    anyhow::ensure!(
        paragraph.byte_for_point(point(px(2.), px(2.))) < 16,
        "long-line original source mapping changed"
    );
    anyhow::ensure!(
        paragraph.visible_lines().map_err(anyhow::Error::msg)?.len() == 1,
        "clipped long line must keep one source row"
    );
    let elapsed = started.elapsed();
    eprintln!("native 64 KiB paragraph layout: {elapsed:?}");
    anyhow::ensure!(
        elapsed < Duration::from_secs(2),
        "legal 64 KiB paragraph must not block native input for {elapsed:?}"
    );
    Ok(())
}
