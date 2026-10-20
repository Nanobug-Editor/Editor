//! Actual Vue format actions are verified in bounded Windows children with the production scheduler.
#![cfg(windows)]
use super::vue_test_support::input;
use super::*;
use crate::*;
use std::ops::Range;

mod boundaries;
mod matrix;
mod native;
mod selection;

/// The expected external file remains deliberately unformatted throughout native formatting.
const EXTERNAL: &str = "/* 外部 中文😀e\u{301} */.untouched{color:blue;margin:0}";

/// Original input and one block's immutable extent are test preimages, never another editable Rope.
struct Sample {
    /// The exact dirty native preimage, independent from the original file on disk.
    source: String,
    /// UTF-8 extent of one style body; its tags and every sibling belong outside the edit.
    body: Range<usize>,
    /// A visible semantic formatting effect rather than a copy of the formatter's implementation.
    color: &'static str,
}

/// Use legal Options exports in every version; Vue 2.6 never receives a script-setup fixture.
fn sample(version: &str, language: &str, external: &str) -> Sample {
    let script = if version == "vue3" {
        "import {defineComponent} from 'vue';export default defineComponent({data(){return{n:1}}});"
    } else {
        "import Vue from 'vue';export default Vue.extend({data(){return{n:1}}});"
    };
    let (style, color) = match language {
        "css" => ("/* 中文😀e\u{301} */.x{color:red;margin:0}", "color: red"),
        "scss" => (
            "/* 中文😀e\u{301} */$tone:red;.x{color:$tone;margin:0}",
            "color: $tone",
        ),
        "less" => (
            "/* 中文😀e\u{301} */@tone:red;.x{color:@tone;margin:0}",
            "color: @tone",
        ),
        "sass" => (
            "// 中文😀e\u{301}\r\n$tone: red\r\n.x\r\n    color:$tone\r\n    margin:0",
            "color: $tone",
        ),
        _ => unreachable!("the declared style matrix is explicit"),
    };
    let prefix = format!(
        "<!-- 脏文本 中文😀e\u{301} -->\r\n<script lang=\"ts\" data-keep=\"script\">{script}</script>\r\n<template><div class=\"keep\" :title=\"n\">中文😀e\u{301}</div></template>\r\n<style lang=\"{language}\" scoped data-keep=\"range\">"
    );
    let body = format!("\r\n{style}\r\n");
    let extent = prefix.len()..prefix.len() + body.len();
    Sample {
        source: format!(
            "{prefix}{body}</style>\r\n<style src=\"./{external}\" lang=\"scss\" data-keep=\"external\"></style>\r\n"
        ),
        body: extent,
        color,
    }
}

/// Original Unicode code points and CRLF boundaries survive both native formatting scopes.
fn preserved(source: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        source.contains("中文😀e\u{301}"),
        "formatting changed Unicode content: {source:?}"
    );
    let stripped = source.replace("\r\n", "");
    anyhow::ensure!(
        !stripped.contains('\r') && !stripped.contains('\n'),
        "formatting mixed line endings: {source:?}"
    );
    Ok(())
}
