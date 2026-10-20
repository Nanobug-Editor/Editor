//! Empty and unsupported ranges preserve the whole SFC and explicit no-change feedback.
use super::*;
use anyhow::{Context as _, ensure};
use gpui_kit::AsyncApp;

/// The bounded Windows child retains every empty/cross-block/attribute/external-src assertion.
#[test]
#[ignore = "set actual TS/Vue packages and NANOBUG_VUE_FIXTURES; requires Windows/D3D11"]
fn installed_vue_rejects_empty_cross_block_attribute_and_external_src_format_ranges() {
    native::run("boundaries");
}

/// Warm the same selected formatter, then prove rejected ranges never fall back to whole formatting.
pub(super) async fn exercise(h: &native::Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.catalog(cx).await?;
    let project = h.workspace.join("tests/projects/vue27");
    let path = project.join("App.vue").canonicalize()?;
    let disk = std::fs::read_to_string(&path)?;
    let style = h.external(&project, ".scss", EXTERNAL)?;
    const SCRIPT: &str = "import Vue from 'vue';export default Vue.extend({data(){return {n:1}}});";
    let script = h.external(&project, ".ts", SCRIPT)?;
    let style_name = style
        .file_name()
        .and_then(|value| value.to_str())
        .context("external style filename")?;
    let script_name = script
        .file_name()
        .and_then(|value| value.to_str())
        .context("external script filename")?;
    let sample = sample("vue27", "css", style_name);
    h.dirty(&path, &sample.source, cx).await?;
    h.dispatch("me_editor::FormatDocument", cx)?;
    h.changed(&sample.source, cx).await?;
    h.undo(&sample.source, cx).await?;
    for (range, empty) in [
        (sample.body.start + 2..sample.body.start + 2, true),
        (
            sample.source.find("export default").unwrap()..sample.body.start + 3,
            false,
        ),
        (
            sample.source.find("scoped").unwrap()..sample.source.find("scoped").unwrap() + 6,
            false,
        ),
        (
            sample.source.find("src=").unwrap()..sample.source.find("src=").unwrap() + 5,
            false,
        ),
    ] {
        h.select(range, cx)?;
        h.dispatch("me_editor::FormatSelection", cx)?;
        h.rejected(&sample.source, empty, cx).await?;
    }
    let source = format!(
        "<!-- 脏文本 中文😀e\u{301} -->\r\n<script src=\"./{script_name}\" lang=\"ts\" data-keep=\"external-script\"></script>\r\n<template><div :title=\"n\">中文😀e\u{301}</div></template>\r\n<style src=\"./{style_name}\" lang=\"scss\" data-keep=\"external-style\"></style>\r\n"
    );
    h.dirty(&path, &source, cx).await?;
    let start = source.find("src=").unwrap();
    h.select(start..start + 5, cx)?;
    h.dispatch("me_editor::FormatSelection", cx)?;
    h.rejected(&source, false, cx).await?;
    h.dispatch("me_editor::FormatDocument", cx)?;
    let formatted = h.changed(&source, cx).await?;
    for attribute in ["external-script", "external-style", script_name, style_name] {
        ensure!(
            formatted.contains(attribute),
            "lost external source attribute {attribute}"
        );
    }
    ensure!(
        !formatted.contains("Vue.extend") && !formatted.contains("untouched"),
        "whole formatting followed external source content"
    );
    preserved(&formatted)?;
    ensure!(
        std::fs::read_to_string(&style)? == EXTERNAL,
        "external style changed"
    );
    ensure!(
        std::fs::read_to_string(&script)? == SCRIPT,
        "external script changed"
    );
    ensure!(
        std::fs::read_to_string(&path)? == disk,
        "boundary test autosaved native text"
    );
    h.undo(&source, cx).await?;
    h.disks_unchanged()
}
