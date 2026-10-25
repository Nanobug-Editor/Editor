//! Real multiline input verifies the native two-to-four-row composer without assigning its text state.
use super::*;

/// Type short, multiline and wrapped drafts, observing actual input geometry and retained content.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.main.update(cx, |_, window, cx| {
        input::click(window, &input::field(window, "commit-message")?, cx)
    })??;
    h.until(
        h.main,
        "composer keyboard focus",
        |window, _| Ok(input::field(window, "commit-message")?.focused() == Some(true)),
        cx,
    )
    .await?;
    let min = h.main.update(cx, |_, window, _| {
        input::field(window, "commit-message").map(|field| field.bounds().size.height)
    })??;
    replace(h, "第一行\nSecond line\nThird line", cx).await?;
    let middle = height(h, cx)?;
    replace(h, "第一行\nSecond line\nThird line\nFourth line", cx).await?;
    let max = height(h, cx)?;
    ensure!(
        middle > min + px(5.) && max > middle + px(5.),
        "composer failed to grow from two to four rows: {min:?}/{middle:?}/{max:?}"
    );
    let line = max - middle;
    ensure!(
        ((middle - min) - line).abs() < px(2.),
        "native rows have inconsistent heights"
    );
    let long = "第一行\nSecond line\nThird line\nFourth line\n第五行\n第六行";
    replace(h, long, cx).await?;
    ensure!(
        (height(h, cx)? - max).abs() < px(1.),
        "composer grew beyond four rows"
    );
    h.main.update(cx, |_, window, _| {
        ensure!(
            input::field(window, "commit-message")?.value() == Some(long),
            "bounded composer dropped hidden lines"
        );
        let button = input::button(window, "commit")?;
        ensure!(
            (button.bounds().size.height - px(28.)).abs() < px(1.),
            "commit bypassed the shared default button size"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    h.capture(h.main, "commit-four-row-limit.png", cx)?;
    replace(h, &"中文软换行 wrapped text ".repeat(20), cx).await?;
    ensure!(
        (height(h, cx)? - max).abs() < px(1.),
        "soft-wrapped draft bypassed the visual-row limit"
    );
    replace(h, "", cx).await?;
    ensure!(
        (height(h, cx)? - min).abs() < px(1.),
        "empty composer failed to shrink to two rows"
    );
    h.record("composer_native_row_heights", serde_json::json!({"two": f32::from(min), "three": f32::from(middle), "four": f32::from(max), "hidden_lines_retained": true, "soft_wrap_bounded": true, "shared_button_height": 28}));
    h.capture(h.main, "commit-two-row-minimum.png", cx)?;
    Ok(())
}

/// Observe the painted Base input, including its real hit area rather than a guest-declared height.
fn height(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<gpui_kit::Pixels> {
    h.main.update(cx, |_, window, _| {
        input::field(window, "commit-message").map(|field| field.bounds().size.height)
    })?
}

/// Replace through normal select-all/backspace/text events so Base retains sole ownership of the draft.
async fn replace(h: &Harness, value: &str, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.main.update(cx, |_, window, cx| {
        editor_ui::testing::press_key(window, "ctrl-a", cx);
        editor_ui::testing::press_key(window, "backspace", cx);
        if !value.is_empty() {
            editor_ui::testing::input_text(window, value, cx);
        }
    })?;
    h.settle(cx).await?;
    Ok(())
}
