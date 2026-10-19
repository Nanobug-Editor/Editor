//! Formatter demand retains the pre-await selection and freezes source only after native input drains.
use super::*;
use plugin_runtime::{
    Availability, LanguageRole, RequestCancellation, plugin_protocol::language::SourceSnapshot,
};
use std::time::Instant;

/// A format action is one original native round; same-ID provider/document ABA cannot revive it.
pub(super) struct Reservation {
    pub request: u64,
    pub language: String,
    pub save: bool,
    editor: Entity<EditorState>,
    document: DocumentVersion,
    key: String,
    availability: Availability,
    generation: u64,
    window_epoch: u64,
}

impl EditorApp {
    /// Reserve the chosen formatter before the first wait, rather than repeating the public action.
    pub(super) fn request_format(
        &mut self,
        save: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self
            .active_text_tab_index()
            .filter(|_| self.session_state.workspace_trusted)
        else {
            return;
        };
        let language = self.language_for_path(self.tabs[index].path());
        if let Some(error) = self.providers.formatter_error(&language) {
            let message = error.message();
            if save {
                self.save_current(cx);
                self.status = format!("{} · {message}", self.status);
            } else {
                self.status = message;
            }
            cx.notify();
            return;
        }
        let key = self
            .providers
            .formatters()
            .get(&language)
            .and_then(Option::as_ref)
            .cloned();
        let Some(key) = key else {
            self.status = t!("editor.format_unavailable").to_string();
            cx.notify();
            return;
        };
        let plugin = key
            .split_once('/')
            .map_or(key.as_str(), |(plugin, _)| plugin);
        let Some(availability) = self.extensions.read(cx).availability(plugin) else {
            return;
        };
        let Ok(document) = self.plugin_document_version(index) else {
            return;
        };
        if let Some(pending) = self.language_edits.format_pending.take() {
            pending.cancel();
        }
        self.language_edits.request = self.language_edits.request.wrapping_add(1);
        let reservation = Reservation {
            request: self.language_edits.request,
            language,
            save,
            editor: self.editor.clone(),
            document,
            key,
            availability,
            generation: self.providers.feature_generation(),
            window_epoch: self.language_features_sequence(),
        };
        self.language_edits.format_pending = Some(RequestCancellation::default());
        if let Some(bridge) = self
            .language_edits
            .bridge
            .clone()
            .filter(|bridge| bridge.read(cx).has_pending())
        {
            cx.spawn_in(window, async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(10))
                        .await;
                    if !this
                        .update_in(cx, |app, _, cx| {
                            app.format_reservation_is_current(&reservation, cx)
                        })
                        .unwrap_or(false)
                        || !bridge.read_with(cx, |bridge, _| bridge.document_is_open())
                    {
                        return;
                    }
                    if !bridge.read_with(cx, |bridge, _| bridge.has_pending()) {
                        break;
                    }
                }
                let _ = this.update_in(cx, |app, window, cx| {
                    app.continue_format(reservation, window, cx)
                });
            })
            .detach();
        } else {
            self.continue_format(reservation, window, cx);
        }
    }

    /// During input drain only revision may advance; provider, window round and original entity stay fixed.
    pub(super) fn format_reservation_is_current(
        &self,
        reservation: &Reservation,
        cx: &App,
    ) -> bool {
        self.session_state.workspace_trusted
            && self.editor == reservation.editor
            && self.language_edits.request == reservation.request
            && self.language_features_sequence() == reservation.window_epoch
            && self.providers.feature_generation() == reservation.generation
            && self
                .providers
                .formatters()
                .get(&reservation.language)
                .and_then(Option::as_ref)
                == Some(&reservation.key)
            && self
                .extensions
                .read(cx)
                .availability(reservation.availability.plugin())
                .as_ref()
                == Some(&reservation.availability)
            && self
                .active_text_tab_index()
                .and_then(|index| self.plugin_document_version(index).ok())
                .is_some_and(|document| {
                    document.id == reservation.document.id
                        && document.path == reservation.document.path
                })
    }

    /// Freeze the drained native source once, then either reuse its live role or demand that exact role.
    fn continue_format(
        &mut self,
        reservation: Reservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.format_reservation_is_current(&reservation, cx) {
            return;
        }
        let index = self.active_text_tab_index().unwrap();
        let Ok(document) = self.plugin_document_version(index) else {
            return;
        };
        let Some(uri) = navigation::file_uri(self.tabs[index].path()) else {
            return;
        };
        let source = SourceSnapshot {
            document,
            text: self.editor.read(cx).text().to_string(),
        };
        let cursor = self.editor.read(cx).cursor();
        if let Some(server) = self
            .language_edits
            .formatters
            .get(&reservation.language)
            .cloned()
        {
            self.dispatch_format(reservation, server, source, cursor, uri, window, cx);
            return;
        }
        let guard = match self.capture_current_command_guard(cx) {
            Ok(guard) => guard,
            Err(_) => return,
        };
        let context_document = self
            .extensions
            .read(cx)
            .language_provider_has_contexts(&reservation.key)
            .then(
                || plugin_runtime::plugin_protocol::language::DocumentContext {
                    path: source.document.path.clone(),
                    generation: reservation.availability.configuration_generation(),
                },
            );
        if let Some(document) = &context_document {
            self.retain_auxiliary_language_context(&reservation.key, document, cx);
        }
        let mut wait = match self.extensions.read(cx).demand_entry(
            reservation.availability.clone(),
            if let Some(document) = &context_document {
                plugin_runtime::DemandReason::LanguageDocument {
                    provider: reservation.key.clone(),
                    role: LanguageRole::Formatter,
                    document: document.clone(),
                }
            } else {
                plugin_runtime::DemandReason::Language {
                    provider: reservation.key.clone(),
                    role: LanguageRole::Formatter,
                }
            },
            Some(guard.clone()),
        ) {
            Ok(wait) => wait,
            Err(error) => {
                self.status = error.to_string();
                cx.notify();
                return;
            }
        };
        let cancellation = wait.cancellation();
        self.language_edits.format_pending = Some(cancellation.clone());
        cx.spawn_in(window, async move |this, cx| {
            if let Err(error) = wait.wait().await {
                let _ = this.update_in(cx, |app, _, cx| {
                    if app.language_edits.request == reservation.request {
                        app.language_edits.format_pending.take();
                        app.status = error.to_string();
                        cx.notify();
                    }
                });
                return;
            }
            let deadline = Instant::now() + Duration::from_secs(60);
            loop {
                let server = this.update_in(cx, |app, _, cx| {
                    if cancellation.is_cancelled()
                        || guard.admit().is_err()
                        || !app.format_reservation_is_current(&reservation, cx)
                        || app
                            .active_text_tab_index()
                            .and_then(|index| app.plugin_document_version(index).ok())
                            .as_ref()
                            != Some(&source.document)
                    {
                        return Err(());
                    }
                    app.sync_dynamic_language_servers(cx);
                    if let Some(document) = &context_document {
                        return Ok(app
                            .extensions
                            .read(cx)
                            .language_document_service(&reservation.key, document)
                            .and_then(Result::ok)
                            .and_then(|plan| app.acquire_language_context_server(plan)));
                    }
                    Ok(app
                        .language_edits
                        .formatters
                        .get(&reservation.language)
                        .cloned())
                });
                match server {
                    Ok(Ok(Some(server))) => {
                        let _ = this.update_in(cx, |app, window, cx| {
                            if app.format_reservation_is_current(&reservation, cx)
                                && guard.admit().is_ok()
                            {
                                app.dispatch_format(
                                    reservation,
                                    server,
                                    source,
                                    cursor,
                                    uri,
                                    window,
                                    cx,
                                );
                            }
                        });
                        return;
                    }
                    Ok(Ok(None)) if Instant::now() < deadline => {}
                    _ => {
                        // Do not strand the completed original activation wait on a replacement
                        // format round when the native adapter cannot publish before its deadline.
                        let _ = this.update_in(cx, |app, _, cx| {
                            if app.language_edits.request == reservation.request {
                                app.language_edits.format_pending.take();
                                if Instant::now() >= deadline && !cancellation.is_cancelled() {
                                    app.status = t!("plugins.language_demand_timeout").to_string();
                                }
                                cx.notify();
                            }
                        });
                        return;
                    }
                }
                cx.background_executor()
                    .timer(Duration::from_millis(10))
                    .await;
            }
        })
        .detach();
    }
}
