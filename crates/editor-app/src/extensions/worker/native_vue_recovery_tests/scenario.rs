//! Opposite template types distinguish restored Vue semantics from a live but unadapted TS pipe.
use super::*;
use crate::language::navigation::{self, ContextCapture, LanguageServer};
use plugin_runtime::plugin_protocol::api;
mod inner;
mod updates;

/// Observe source-level features, scoped EOF, normal reconnect and exact native owner cleanup.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let inner_failure = std::env::var(FAILURE_ENV).as_deref() == Ok("inner-adapter");
    let mut owned_outer = None;
    h.wait(
        "cold language catalog",
        30,
        |app, _| {
            app.providers
                .language_servers()
                .get("vue")
                .is_some_and(Option::is_some)
        },
        cx,
    )
    .await?;
    for index in 0..h.paths.len() {
        let path = h.paths[index].clone();
        h.main.update(cx, |_, window, cx| {
            h.app.update(cx, |app, cx| {
                app.open_file(path.clone(), window, cx);
            })
        })?;
        h.wait(
            "normal native text session",
            10,
            |app, _| {
                app.tabs
                    .iter()
                    .position(|tab| tab.path() == path)
                    .is_some_and(|index| app.plugin_document_version(index).is_ok())
            },
            cx,
        )
        .await?;
        h.replace(index, source(index == 2, false), cx)?;
        if index == 0 {
            // Establish the cold first source before opening the other two graphs. Candidate
            // and recovery checks start from usable semantics, rather than competing cold
            // preparations; each ordinary request retains its production deadline.
            h.wait(
                "first initialized adapter source",
                180,
                |app, _| {
                    // This phase needs the selected native plan; the next standard completion
                    // and diagnostic requests prove actual adapted type semantics before injection.
                    app.language_server_for_path(&path)
                        .is_some_and(|server| server.is_active())
                        && h.plan(0).is_ok()
                },
                cx,
            )
            .await?;
            h.analyze(0, cx)
                .await?
                .assert_semantics("toUpperCase", "toFixed", 2551)?;
            if inner_failure {
                // Only this mode needs OS parentage for the exact first outer worker.
                owned_outer = Some(
                    h.job("owned first stdio identity", inner::owned_outer, cx)
                        .await?,
                );
            }
        }
    }
    h.wait(
        "independent real Vue contexts",
        180,
        |app, _| {
            h.paths.iter().enumerate().all(|(index, path)| {
                app.language_server_for_path(path)
                    .is_some_and(|server| server.is_active())
                    && h.plan(index).is_ok()
            })
        },
        cx,
    )
    .await?;
    eprintln!("native Vue recovery phase: initial typed completion and diagnostics");
    if std::env::var(FAILURE_ENV).as_deref() == Ok("provider-update") {
        updates::provider::exercise(h, cx).await?;
        return Ok(());
    }
    if !inner_failure {
        updates::exercise(h, cx).await?;
    }
    let before_a = h.analyze(0, cx).await?;
    before_a.assert_semantics("toUpperCase", "toFixed", 2551)?;
    let before_b = h.analyze(2, cx).await?;
    before_b.assert_semantics("toFixed", "toUpperCase", 2339)?;
    let before_c = h.analyze(1, cx).await?;
    before_c.assert_semantics("toUpperCase", "toFixed", 2551)?;
    let plan_a = h.plan(0)?;
    let plan_b = h.plan(2)?;
    let plan_c = h.plan(1)?;
    ensure!(
        !Arc::ptr_eq(&plan_a, &plan_b)
            && !Arc::ptr_eq(&plan_a, &plan_c)
            && !Arc::ptr_eq(&plan_b, &plan_c),
        "Vue versions share one native context"
    );
    ensure!(
        plan_a.process_count() == 1 && plan_b.process_count() == 1 && plan_c.process_count() == 1,
        "initial semantic evidence needs three actual managed workers"
    );

    if let Some(owned) = &owned_outer {
        let owned = owned.clone();
        h.job(
            "terminate only admitted adapter child",
            move || inner::terminate_adapter(&owned),
            cx,
        )
        .await?;
    } else {
        plan_a.stop_processes();
        ensure!(
            plan_a.process_count() == 0
                && plan_b.process_count() == 1
                && plan_c.process_count() == 1,
            "stopping one backend affected the other context"
        );
    }
    eprintln!("native Vue recovery phase: observe actual EOF without replay");
    let server = before_a.server.clone();
    let text = before_a.source.clone();
    let uri = navigation::file_uri(&h.paths[0]).unwrap();
    let cursor =
        navigation::position_at_byte(&text, text.rfind("value.").unwrap() + "value.".len());
    h.job(
        "interrupted original completion",
        move || {
            let result = server.completions(uri, text, cursor);
            ensure!(
                result.is_err(),
                "an interrupted request was replayed as success"
            );
            Ok(())
        },
        cx,
    )
    .await?;
    let unaffected = h.analyze(2, cx).await?;
    unaffected.assert_semantics("toFixed", "toUpperCase", 2339)?;
    ensure!(
        Arc::ptr_eq(&before_b.server, &unaffected.server)
            && Arc::ptr_eq(&plan_b, &h.plan(2)?)
            && before_b.capture.is_live(),
        "the healthy version changed native incarnation during another backend's failure"
    );
    if let Some(owned) = owned_outer {
        h.job(
            "inner failure reached outer service EOF",
            move || inner::wait_retired(&owned),
            cx,
        )
        .await?;
    }

    eprintln!("native Vue recovery phase: normal bounded transport recovery");
    let recovering = before_a.server.clone();
    h.job(
        "bounded production recovery",
        move || recovering.prepare_until_ready(),
        cx,
    )
    .await?;
    // A delegated provider rebuilds its process-local adapter through normal published demand.
    // Wait for that ordinary native binding; the harness never invokes its domain control methods.
    h.wait(
        "reprepared original native context",
        60,
        |app, cx| {
            app.language_server_for_path(&h.paths[0]).is_some()
                && app
                    .tabs
                    .iter()
                    .find(|tab| tab.path() == h.paths[0])
                    .and_then(|tab| tab.text.as_ref())
                    .is_some_and(|text| text.editor.read(cx).lsp().completion_provider.is_some())
        },
        cx,
    )
    .await?;
    // Deliberately do not invoke a Vue/TS service or attach an adapter in this harness. A normal
    // user's next standard language operation must restore real semantics through production paths.
    let recovered = h.analyze(0, cx).await?;
    recovered.assert_semantics("toUpperCase", "toFixed", 2551)?;
    ensure!(
        !before_a.capture.is_live(),
        "pre-failure source results revived after reconnect"
    );
    ensure!(
        h.main.update(cx, |_, _, cx| {
            !h.app
                .read(cx)
                .language_context_is_current(&before_a.capture, cx)
        })?,
        "the native UI still accepts the old connection's result"
    );
    ensure!(
        recovered.capture.is_live(),
        "new semantic evidence has no current result seal"
    );
    let unaffected_c = h.analyze(1, cx).await?;
    unaffected_c.assert_semantics("toUpperCase", "toFixed", 2551)?;
    ensure!(
        Arc::ptr_eq(&before_c.server, &unaffected_c.server)
            && Arc::ptr_eq(&plan_c, &h.plan(1)?)
            && before_c.capture.is_live(),
        "Vue 2.7 changed incarnation during another backend's recovery"
    );

    eprintln!("native Vue recovery phase: new unsaved type invalidates prior semantics");
    h.replace(0, source(false, true), cx)?;
    h.analyze(0, cx)
        .await?
        .assert_semantics("toFixed", "toUpperCase", 2339)?;
    ensure!(
        before_b.capture.is_live() && plan_b.process_count() == 1,
        "changed recovery source retired an unrelated Vue backend"
    );
    let recovered_plan = h.plan(0)?;
    h.main.update(cx, |_, _, cx| {
        h.app.update(cx, |app, cx| {
            app.set_workspace_trusted(false, cx);
        })
    })?;
    h.wait(
        "trust revokes all three managed contexts",
        30,
        |app, _| {
            !app.session_state.workspace_trusted
                && !plan_a.is_active()
                && !recovered_plan.is_active()
                && !plan_b.is_active()
                && !plan_c.is_active()
                && plan_a.process_count() == 0
                && recovered_plan.process_count() == 0
                && plan_b.process_count() == 0
                && plan_c.process_count() == 0
        },
        cx,
    )
    .await?;
    Ok(())
}

/// This immutable observation is evidence only; all live text and undo remain in native sessions.
struct Observation {
    server: Arc<LanguageServer>,
    capture: ContextCapture,
    source: String,
    completions: Vec<String>,
    diagnostics: Vec<lsp_types::Diagnostic>,
}

impl Observation {
    /// Precise type members and original-source compiler ranges rule out empty/fallback replies.
    /// TS offers the similar string member `fixed` (2551), while number has no such hint (2339).
    fn assert_semantics(&self, allowed: &str, invalid: &str, code: i32) -> anyhow::Result<()> {
        ensure!(
            self.completions.iter().any(|label| label == allowed)
                && self.completions.iter().all(|label| label != invalid),
            "typed template completion lacks {allowed} or contains {invalid}: {:?}",
            self.completions
        );
        let diagnostic = self
            .diagnostics
            .iter()
            .find(|item| {
                matches!(&item.code, Some(lsp_types::NumberOrString::Number(found)) if *found == code)
                    && item.message.contains(invalid)
            })
            .with_context(|| {
                format!(
                    "template type diagnostic for {invalid} absent: {:?}",
                    self.diagnostics
                )
            })?;
        let start = navigation::editing::byte_at_position(&self.source, diagnostic.range.start)?;
        let end = navigation::editing::byte_at_position(&self.source, diagnostic.range.end)?;
        ensure!(
            self.source.get(start..end) == Some(invalid),
            "template type diagnostic does not select original {invalid} source text"
        );
        Ok(())
    }
}

impl Harness {
    /// Replace through the existing Atomic native application and its DocumentSession subscriptions.
    fn replace(&self, index: usize, value: String, cx: &mut AsyncApp) -> anyhow::Result<()> {
        self.main.update(cx, |_, window, cx| {
            self.app.update(cx, |app, cx| {
                let editor = app
                    .tabs
                    .iter()
                    .find(|tab| tab.path() == self.paths[index])
                    .and_then(|tab| tab.text.as_ref())
                    .context("native text target absent")?
                    .editor
                    .clone();
                let current = editor.read(cx).text().to_string();
                let prepared = crate::editor::text_transactions::PreparedTextEdits::new(
                    current.clone(),
                    &[api::TextEdit {
                        range: api::TextRange {
                            start: 0,
                            end: current.len(),
                        },
                        text: value.clone(),
                    }],
                    api::TextRange {
                        start: value.len(),
                        end: value.len(),
                    },
                )?;
                editor
                    .update(cx, |editor, cx| prepared.apply(editor, window, cx))
                    .map(|_| ())
            })
        })??;
        Ok(())
    }

    /// Capture only current EditorState snapshots, then use ordinary standard language requests.
    async fn analyze(&self, index: usize, cx: &mut AsyncApp) -> anyhow::Result<Observation> {
        self.frame(cx)?;
        let (server, document, uri, source, capture) = self.main.update(cx, |_, _, cx| {
            let app = self.app.read(cx);
            let tab = app
                .tabs
                .iter()
                .position(|tab| tab.path() == self.paths[index])
                .context("source tab missing")?;
            let version = app.plugin_document_version(tab)?;
            let server = app
                .language_server_for_path(&self.paths[index])
                .context("native Vue server not prepared")?
                .clone();
            let source = app.tabs[tab]
                .text
                .as_ref()
                .unwrap()
                .editor
                .read(cx)
                .text()
                .to_string();
            let capture = app
                .capture_language_context(&server, &version, &source, cx)?
                .context("real Vue provider omitted related source synchronization")?;
            let uri = navigation::file_uri(&self.paths[index]).unwrap();
            let document = server.open_document_for_language(uri.clone(), "vue");
            Ok::<_, anyhow::Error>((server, document, uri, source, capture))
        })??;
        let observed_server = server.clone();
        self.job(
            "standard source completion and diagnostics",
            move || {
                let position = navigation::position_at_byte(
                    &source,
                    source
                        .rfind("value.")
                        .context("typed template fixture missing")?
                        + "value.".len(),
                );
                let (completions, diagnostics) = server.with_context_snapshot(&capture, || {
                    let response = server.completions(uri, source.clone(), position)?;
                    let items = match response {
                        lsp_types::CompletionResponse::Array(items) => items,
                        lsp_types::CompletionResponse::List(list) => list.items,
                    };
                    let diagnostics = server
                        .diagnostics_for(document, &source)?
                        .context("real backend did not return current diagnostics")?;
                    Ok((
                        items.into_iter().map(|item| item.label).collect(),
                        diagnostics,
                    ))
                })?;
                Ok(Observation {
                    server: observed_server,
                    capture,
                    source,
                    completions,
                    diagnostics,
                })
            },
            cx,
        )
        .await
    }

    /// Runtime-published bindings identify the actual delegated version plan, without native paths.
    fn plan(&self, index: usize) -> anyhow::Result<Arc<plugin_runtime::LanguageService>> {
        let path = self.paths[index]
            .strip_prefix(&self.workspace)?
            .to_string_lossy()
            .replace('\\', "/");
        let state = self.actor.state.lock().unwrap();
        let (_, key) = state
            .language_document_bindings
            .iter()
            .find(|((provider, document), _)| provider == "vue/vue" && document.path == path)
            .context("runtime did not publish the Vue source binding")?;
        state
            .language_services
            .get(key)
            .context("runtime native version plan missing")?
            .as_ref()
            .map(Arc::clone)
            .map_err(|error| anyhow::anyhow!("runtime native version plan failed: {error}"))
    }
}

/// The two actual Vue versions intentionally need opposite methods, despite sharing a field name.
fn source(modern: bool, changed: bool) -> String {
    let numeric = modern || changed;
    let value = if numeric { "42" } else { "'recovery string'" };
    let method = if numeric { "toUpperCase" } else { "toFixed" };
    let script = if modern {
        format!("<script setup lang=\"ts\">\r\nconst value = {value};\r\n</script>")
    } else {
        format!(
            "<script lang=\"ts\">\r\nimport Vue from 'vue';\r\nexport default Vue.extend({{data(){{return {{value:{value}}}}}}});\r\n</script>"
        )
    };
    format!(
        "<!-- recovery 未保存 中文😀e\u{301} -->\r\n{script}\r\n<template><p>{{{{ value.{method}() }}}}</p></template>\r\n"
    )
}
