//! A cold feature keeps its original reservation and source while preparing exactly one selected role.
use super::*;
use plugin_runtime::{Availability, LanguageRole};
use std::time::Instant;

/// Static selection survives a cold first birth; metadata, scope and configuration changes revoke it.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct Choice {
    key: String,
    availability: Availability,
    role: LanguageRole,
}

impl EditorApp {
    /// Availability exposes a declared role before a live Arc exists, without preparing other LSPs.
    pub(super) fn language_feature_choice(
        &self,
        language: &str,
        feature: direct::Feature,
        cx: &App,
    ) -> Option<Choice> {
        let (key, role) = if providers::has_language_feature(language, feature) {
            (
                providers::language_feature_provider(language, feature)?,
                LanguageRole::Direct(feature),
            )
        } else {
            (
                providers::language_servers()
                    .get(language)?
                    .as_ref()?
                    .clone(),
                LanguageRole::Lsp,
            )
        };
        let plugin = key
            .split_once('/')
            .map_or(key.as_str(), |(plugin, _)| plugin);
        let availability = self.extensions.read(cx).availability(plugin)?;
        Some(Choice {
            key,
            availability,
            role,
        })
    }

    /// Capture happens after the native bridge drained and before this wait. Completion may bind a
    /// first-born provider once, but cannot call the public entry again or read a new document/source.
    pub(super) fn demand_language_feature(
        &mut self,
        intent: Intent,
        source: SourceSnapshot,
        cursor: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let guard = match self.capture_current_command_guard(cx) {
            Ok(guard) => guard,
            Err(error) => {
                self.status = error.to_string();
                cx.notify();
                return;
            }
        };
        let mut wait = match self.extensions.read(cx).demand_entry(
            intent.choice.availability.clone(),
            plugin_runtime::DemandReason::Language {
                provider: intent.choice.key.clone(),
                role: intent.choice.role.clone(),
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
        self.language_features.pending = Some(cancellation.clone());
        cx.spawn_in(window, async move |this, cx| {
            if let Err(error) = wait.wait().await {
                let _ = this.update_in(cx, |app, _, cx| {
                    if app.language_features.sequence == intent.sequence {
                        app.language_features.pending.take();
                        app.status = error.to_string();
                        cx.notify();
                    }
                });
                return;
            }
            // Receipt can precede native publication/LSP binding by a turn. Read only the original
            // role's prepared lease while the exact captured source, scope and sequence stay valid.
            let deadline = Instant::now() + Duration::from_secs(60);
            loop {
                let target = this.update_in(cx, |app, _, cx| {
                    if cancellation.is_cancelled()
                        || guard.admit().is_err()
                        || !app.language_intent_is_current(&intent, cx)
                        || app
                            .active_text_tab_index()
                            .and_then(|index| app.plugin_document_version(index).ok())
                            .as_ref()
                            != Some(&source.document)
                    {
                        return Err(());
                    }
                    app.sync_dynamic_language_servers(cx);
                    Ok(app.language_feature_target(&intent.language, intent.feature, cx))
                });
                match target {
                    Ok(Ok(Some(target))) => {
                        let _ = this.update_in(cx, |app, window, cx| {
                            if app.language_intent_is_current(&intent, cx) && guard.admit().is_ok()
                            {
                                app.dispatch_language_feature(
                                    intent, target, source, cursor, window, cx,
                                );
                            }
                        });
                        return;
                    }
                    Ok(Ok(None)) if Instant::now() < deadline => {}
                    _ => {
                        // A stale native target or bounded publication timeout seals this original
                        // round; it must not leave Escape/pending state attached to a later request.
                        let _ = this.update_in(cx, |app, _, cx| {
                            if app.language_features.sequence == intent.sequence {
                                app.language_features.pending.take();
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

    /// Observe Escape through the existing shell's native capture, without replacing Base behavior.
    pub(crate) fn cancel_pending_language_feature(
        &mut self,
        event: &gpui_kit::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key == "escape" && event.keystroke.modifiers == Default::default() {
            if self.language_features.pending.is_some() {
                self.cancel_language_features(window, cx);
            }
            self.cancel_pending_format();
        }
    }
}
