//! Every first-batch Vue/style combination uses actual Packages on the real Windows scheduler.
use super::*;
use anyhow::{Context as _, ensure};
use gpui_kit::AsyncApp;

/// Parent/child isolation retains all twelve whole/range cases without TestScheduler wakeups.
#[test]
#[ignore = "set actual TS/Vue packages and NANOBUG_VUE_FIXTURES; requires Windows/D3D11"]
fn installed_vue_whole_and_selected_style_formatting_preserve_source_and_single_undo() {
    native::run("matrix");
}

/// Each partial selection can expand only within its style body; all sibling bytes remain exact.
pub(super) async fn exercise(h: &native::Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.catalog(cx).await?;
    for version in ["vue26", "vue27", "vue3"] {
        let project = h.workspace.join(format!("tests/projects/{version}"));
        let path = project.join("App.vue").canonicalize()?;
        let disk = std::fs::read_to_string(&path)?;
        let dependency = h.external(&project, ".scss", EXTERNAL)?;
        let name = dependency
            .file_name()
            .and_then(|value| value.to_str())
            .context("external style filename")?;
        for language in ["css", "scss", "sass", "less"] {
            eprintln!("S3 native formatting: {version}/{language} range and whole");
            let sample = sample(version, language, name);
            h.dirty(&path, &sample.source, cx).await?;
            h.select(sample.body.start + 2..sample.body.end - 2, cx)?;
            h.dispatch("me_editor::FormatSelection", cx)?;
            let formatted = h.changed(&sample.source, cx).await?;
            let body = formatted
                .strip_prefix(&sample.source[..sample.body.start])
                .and_then(|value| value.strip_suffix(&sample.source[sample.body.end..]))
                .context("range formatting changed a byte outside the selected style body")?;
            ensure!(
                body.contains(sample.color),
                "{version}/{language} style body: {body:?}"
            );
            preserved(&formatted)?;
            ensure!(
                std::fs::read_to_string(&path)? == disk,
                "range formatting autosaved the source"
            );
            ensure!(
                std::fs::read_to_string(&dependency)? == EXTERNAL,
                "range formatting followed external src"
            );
            h.undo(&sample.source, cx).await?;
            h.dispatch("me_editor::FormatDocument", cx)?;
            let whole = h.changed(&sample.source, cx).await?;
            ensure!(
                whole.contains("n: 1"),
                "the actual Options script was not formatted: {whole:?}"
            );
            ensure!(
                whole.contains(sample.color),
                "{version}/{language} whole style: {whole:?}"
            );
            for attribute in [
                "data-keep=\"script\"",
                "data-keep=\"range\"",
                "data-keep=\"external\"",
                "scoped",
            ] {
                ensure!(
                    whole.contains(attribute),
                    "whole formatting lost {attribute}: {whole:?}"
                );
            }
            ensure!(
                whole.contains(&format!("src=\"./{name}\"")),
                "external src attribute changed"
            );
            ensure!(
                whole.matches("<script").count() == 1 && whole.matches("</script>").count() == 1,
                "script boundary changed"
            );
            ensure!(
                whole.matches("<style").count() == 2 && whole.matches("</style>").count() == 2,
                "style boundary changed"
            );
            ensure!(
                !whole.contains("untouched"),
                "whole formatting followed an external source"
            );
            preserved(&whole)?;
            ensure!(
                std::fs::read_to_string(&path)? == disk,
                "manual whole formatting autosaved the source"
            );
            ensure!(
                std::fs::read_to_string(&dependency)? == EXTERNAL,
                "whole formatting changed external src bytes"
            );
            h.undo(&sample.source, cx).await?;
        }
    }
    h.disks_unchanged()
}

/// The selected independent TS formatter and main TS analysis survive an actual Vue installation.
#[test]
#[ignore = "set actual TS/Vue packages and NANOBUG_VUE_FIXTURES; requires Windows/D3D11"]
fn installing_vue_preserves_the_existing_typescript_formatter_and_primary_analysis() {
    native::run("provider");
}

/// Install Vue only after the real original TypeScript context is active and explicitly chosen.
pub(super) async fn provider(h: &native::Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let path = h
        .external(
            &h.workspace.join("target"),
            ".ts",
            "export const saved = 1;\r\n",
        )?
        .canonicalize()?;
    let source = "// 脏文本 中文😀e\u{301}\r\nexport const plain={value:42};\r\n";
    h.dirty(&path, source, cx).await?;
    h.wait(
        "original TS analysis",
        180,
        |app, cx| {
            native::preparation(app, &h.actor)?;
            Ok(app
                .language_server_for_path(&path)
                .is_some_and(|server| server.is_active())
                && app.editor.read(cx).lsp().hover_provider.is_some())
        },
        cx,
    )
    .await?;
    let (original, grammar) = h.main.update(cx, |_, _, cx| {
        let app = h.app.read(cx);
        (
            app.language_server_for_path(&path).unwrap().clone(),
            app.providers
                .grammars()
                .into_iter()
                .find(|provider| provider.declaration.language == "typescript"),
        )
    })?;
    crate::language::providers::choose(
        plugin_runtime::plugin_protocol::settings::Scope::User,
        "formatter:typescript",
        Some("typescript/analysis"),
    )
    .map_err(anyhow::Error::msg)?;
    h.actor
        .tx
        .send(Work::Install(
            Package::read(&input("NANOBUG_VUE_PACKAGE"))?,
            true,
        ))
        .map_err(|_| anyhow::anyhow!("worker stopped before Vue installation"))?;
    h.catalog(cx).await?;
    h.main.update(cx, |_, _, cx| -> anyhow::Result<()> {
        let app = h.app.read(cx);
        ensure!(
            app.providers
                .formatters()
                .get("typescript")
                .and_then(Option::as_deref)
                == Some("typescript/analysis"),
            "Vue replaced the selected TS formatter"
        );
        ensure!(
            app.providers
                .grammars()
                .into_iter()
                .find(|provider| provider.declaration.language == "typescript")
                == grammar,
            "Vue replaced the selected TS grammar"
        );
        ensure!(
            Arc::ptr_eq(
                app.language_server_for_path(&path)
                    .context("original TS primary disappeared")?,
                &original
            ),
            "Vue replaced the original TS primary"
        );
        Ok(())
    })??;
    h.dispatch("me_editor::FormatDocument", cx)?;
    let formatted = h.changed(source, cx).await?;
    ensure!(
        formatted.contains("plain = { value: 42 }"),
        "independent TS formatter output: {formatted:?}"
    );
    h.main.update(cx, |_, _, cx| -> anyhow::Result<()> {
        ensure!(
            Arc::ptr_eq(
                h.app
                    .read(cx)
                    .language_server_for_path(&path)
                    .context("TS primary disappeared after format")?,
                &original
            ),
            "formatting replaced the original TS primary"
        );
        Ok(())
    })??;
    ensure!(
        std::fs::read_to_string(&path)? == "export const saved = 1;\r\n",
        "TS formatting saved dirty input"
    );
    h.undo(source, cx).await?;
    h.disks_unchanged()
}
