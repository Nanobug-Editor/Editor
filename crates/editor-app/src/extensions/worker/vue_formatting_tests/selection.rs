//! Native input after dispatch expires a format round, including equal-text/equal-selection IME ABA.
use super::*;
use anyhow::{Context as _, ensure};
use gpui_kit::{AsyncApp, EntityInputHandler as _};

/// Windows scheduling validates the original four same-turn action/input cases without a test scheduler.
#[test]
#[ignore = "set actual TS/Vue packages and NANOBUG_VUE_FIXTURES; requires Windows/D3D11"]
fn installed_vue_format_rounds_cannot_overwrite_same_turn_native_input_or_ime_aba() {
    native::run("aba");
}

/// This scenario covers dispatch-to-application; the generic delayed formatter separately proves pipe ingress.
pub(super) async fn exercise(h: &native::Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.catalog(cx).await?;
    let project = h.workspace.join("tests/projects/vue27");
    let path = project.join("App.vue").canonicalize()?;
    let disk = std::fs::read_to_string(&path)?;
    let dependency = h.external(&project, ".scss", EXTERNAL)?;
    let sample = sample(
        "vue27",
        "css",
        dependency
            .file_name()
            .and_then(|value| value.to_str())
            .context("external source filename")?,
    );
    h.dirty(&path, &sample.source, cx).await?;
    h.dispatch("me_editor::FormatDocument", cx)?;
    h.changed(&sample.source, cx).await?;
    h.undo(&sample.source, cx).await?;
    for action in ["me_editor::FormatSelection", "me_editor::FormatDocument"] {
        for ime in [false, true] {
            h.select(sample.body.start + 2..sample.body.end - 2, cx)?;
            let before = h.version(cx)?;
            h.main.update(cx, |_, window, cx| -> anyhow::Result<()> {
                let format = cx
                    .build_action(action, None)
                    .context("registered native format action")?;
                window.dispatch_action(format, cx);
                h.app
                    .read(cx)
                    .editor
                    .clone()
                    .update(cx, |editor, cx| -> anyhow::Result<()> {
                        editor.set_selected_range(0..0, cx);
                        if ime {
                            editor.replace_and_mark_text_in_range(
                                None,
                                "拼😀",
                                Some(0..3),
                                window,
                                cx,
                            );
                            ensure!(
                                editor.marked_text_range(window, cx).is_some(),
                                "preedit did not enter the original native editor"
                            );
                            editor.replace_and_mark_text_in_range(None, "", None, window, cx);
                            editor.unmark_text(window, cx);
                            ensure!(
                                editor.marked_text_range(window, cx).is_none(),
                                "IME cancellation retained marked text"
                            );
                            // Restore selection as well as bytes; a changed final target must not mask a revision bug.
                            editor
                                .set_selected_range(sample.body.start + 2..sample.body.end - 2, cx);
                        } else {
                            editor.replace_text_in_range(None, "X", window, cx);
                        }
                        cx.notify();
                        Ok(())
                    })
            })??;
            h.settle(cx).await?;
            if ime {
                let current = h.version(cx)?;
                ensure!(
                    current.id == before.id,
                    "IME replaced the original native incarnation"
                );
                ensure!(
                    current.revision > before.revision,
                    "cancelled preedit did not expire the capability with identical final bytes"
                );
            }
            let expected = if ime {
                sample.source.clone()
            } else {
                format!("X{}", sample.source)
            };
            ensure!(
                h.text(cx)? == expected,
                "obsolete {action} overwrote native input; ime={ime}"
            );
            ensure!(
                std::fs::read_to_string(&path)? == disk,
                "stale formatting autosaved native source"
            );
            if !ime {
                h.undo(&sample.source, cx).await?;
            }
        }
    }
    h.dispatch("me_editor::FormatDocument", cx)?;
    h.changed(&sample.source, cx).await?;
    h.undo(&sample.source, cx).await?;
    ensure!(
        std::fs::read_to_string(&dependency)? == EXTERNAL,
        "input ABA formatting changed an external source"
    );
    h.disks_unchanged()
}
