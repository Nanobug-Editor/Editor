//! Native host diagnostics render safe immutable metadata, independently from plugin UI and logs.
use super::*;
use gpui_kit::PathPromptOptions;

/// Export provenance is host-generated; plugin text, native arguments and logs are never attached.
#[derive(serde::Serialize)]
struct DiagnosticExport {
    /// Independently versioned host diagnostics schema.
    schema_version: u32,
    /// Exact host executable and embedded SDK source identities.
    tool: plugin_runtime::validation::ToolIdentity,
    /// Compiled operating system and architecture, with no machine/user identifier.
    platform: String,
    /// Filtered immutable metadata with capture coverage and package/incarnation identities.
    observations: plugin_runtime::observability::Snapshot,
}

impl ExtensionPanel {
    /// Read at most one snapshot per second for `id`; hidden pages stop refreshing this cache.
    pub(super) fn runtime_diagnostics(&mut self, id: &str, cx: &mut Context<Self>) -> AnyElement {
        let sink = self.worker.state.lock().unwrap().observations.clone();
        if self
            .manager_observation
            .as_ref()
            .is_none_or(|(selected, time, _)| {
                selected != id || time.elapsed() >= Duration::from_secs(1)
            })
        {
            self.manager_observation = Some((id.into(), Instant::now(), sink.snapshot(Some(id))));
        }
        let snapshot = &self.manager_observation.as_ref().unwrap().2;
        let mut body = v_flex()
            .p_5()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        Button::new("plugin-diagnostics-start")
                            .label(t!("plugins.diagnostics_start").to_string())
                            .debug_selector(|| "plugin-diagnostics-start".into())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.worker
                                    .state
                                    .lock()
                                    .unwrap()
                                    .observations
                                    .start_capture();
                                this.manager_observation = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("plugin-diagnostics-stop")
                            .label(t!("plugins.diagnostics_stop").to_string())
                            .debug_selector(|| "plugin-diagnostics-stop".into())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.worker
                                    .state
                                    .lock()
                                    .unwrap()
                                    .observations
                                    .stop_capture();
                                this.manager_observation = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("plugin-diagnostics-export")
                            .label(t!("plugins.diagnostics_export").to_string())
                            .debug_selector(|| "plugin-diagnostics-export".into())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.export_diagnostics(window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("plugins.diagnostics_scope").to_string()),
            );
        if snapshot.owners.is_empty() {
            body = body.child(
                div()
                    .debug_selector(|| "plugin-diagnostics-dormant".into())
                    .child(t!("plugins.diagnostics_dormant").to_string()),
            );
        }
        if snapshot.partial {
            body =
                body.child(div().text_color(cx.theme().muted_foreground).child(
                    t!("plugins.diagnostics_partial", count = snapshot.dropped).to_string(),
                ));
        }
        for owner in &snapshot.owners {
            body = body.child(
                div()
                    .font_semibold()
                    .child(format!("{} · {}", owner.owner.plugin, owner.owner.instance)),
            );
            if let Some(resources) = &owner.resources {
                body = body.child(
                    div().child(
                        t!(
                            "plugins.diagnostics_resources",
                            handles = resources.handles,
                            requests = resources.requests,
                            http = resources.http,
                            timers = resources.timers,
                            subscriptions = resources.subscriptions,
                            processes = resources.processes,
                            pending = resources.native_pending
                        )
                        .to_string(),
                    ),
                );
                if resources.native_cleanup_unconfirmed {
                    body = body.child(
                        div()
                            .text_color(cx.theme().danger)
                            .child(t!("plugins.diagnostics_cleanup_unknown").to_string()),
                    );
                }
            }
            for (stage, calls) in &owner.calls {
                let elapsed = owner.elapsed_us.get(stage).copied().unwrap_or(0);
                body = body.child(
                    div().child(
                        t!(
                            "plugins.diagnostics_stage",
                            stage = stage_label(stage),
                            count = calls,
                            elapsed = elapsed
                        )
                        .to_string(),
                    ),
                );
            }
        }
        let mut slow = snapshot.records.iter().collect::<Vec<_>>();
        slow.sort_by_key(|record| std::cmp::Reverse(record.elapsed_us));
        for record in slow.into_iter().take(20) {
            body = body.child(div().text_sm().child(format!(
                "#{} · {} · {} μs · {}",
                record.id,
                stage_label(&format!("{:?}", record.stage).to_lowercase()),
                record.elapsed_us,
                outcome_label(record.outcome)
            )));
        }
        body.into_any_element()
    }

    /// Select a directory before exporting; only the safe snapshot is written off the UI thread.
    fn export_diagnostics(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let snapshot = self
            .worker
            .state
            .lock()
            .unwrap()
            .observations
            .snapshot(self.manager_selected.as_deref());
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t!("plugins.diagnostics_export").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(directory) = paths.into_iter().next() else {
                return;
            };
            let name = format!(
                "nanobug-plugin-diagnostics-{}.json",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |time| time.as_millis())
            );
            let result = cx
                .background_executor()
                .spawn(async move {
                    let report = DiagnosticExport {
                        schema_version: 1,
                        tool: crate::plugin_development::report::tool(),
                        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
                        observations: snapshot,
                    };
                    plugin_runtime::observability::write_report(&directory.join(name), &report)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if result.is_err() {
                    this.accept_operation_status(
                        worker::OperationStatus {
                            plugin: this.manager_selected.clone(),
                            message: t!("plugins.diagnostics_export_failed").to_string(),
                        },
                        cx,
                    );
                }
                cx.notify();
            });
        })
        .detach();
    }
}

/// These closed host labels never incorporate an arbitrary guest operation or error string.
fn stage_label(stage: &str) -> String {
    t!(match stage {
        "queue" => "plugins.diagnostics_queue",
        "preparation" => "plugins.diagnostics_preparation",
        "activation" => "plugins.diagnostics_activation",
        "guest" => "plugins.diagnostics_guest",
        "host_call" | "hostcall" => "plugins.diagnostics_host_call",
        "request" => "plugins.diagnostics_request",
        "http" => "plugins.diagnostics_http",
        "native" => "plugins.diagnostics_native",
        "publication" => "plugins.diagnostics_publication",
        _ => "plugins.diagnostics_unknown",
    })
    .to_string()
}

/// Human-readable terminal labels remain bilingual; the JSON enum stays a stable machine value.
fn outcome_label(outcome: plugin_runtime::observability::Outcome) -> String {
    use plugin_runtime::observability::Outcome;
    t!(match outcome {
        Outcome::Completed => "plugins.diagnostics_completed",
        Outcome::Failed => "plugins.diagnostics_failed",
        Outcome::Cancelled => "plugins.diagnostics_cancelled",
        Outcome::TimedOut => "plugins.diagnostics_timed_out",
        Outcome::Unknown => "plugins.diagnostics_unknown",
    })
    .to_string()
}
