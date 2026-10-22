//! User-profile history configuration uses native inputs and a shared bounded storage worker.
#[cfg(test)]
mod tests;

use super::*;
use editor_history::{Policy, Stats, Worker};
use gpui_base::input::InputState;

/// Each settings window owns its own input entities; closed windows release their retained form.
struct Views {
    /// A profile may be shown in two native windows without sharing input/focus entities.
    forms: HashMap<(gpui_kit::WindowId, PathBuf), Entity<HistorySettings>>,
    /// Release forms when the dialog closes instead of retaining cancelled drafts indefinitely.
    _closed: Subscription,
}
impl gpui_kit::Global for Views {}

/// Resolve an owner once and retain the native form for that settings window.
pub(super) fn render(owner: &SettingsOwner, window: &mut Window, cx: &mut App) -> AnyElement {
    let profile = match owner.profile(cx) {
        Ok(profile) => profile,
        Err(error) => return div().child(error.to_string()).into_any_element(),
    };
    if !cx.has_global::<Views>() {
        let closed = cx.on_window_closed(|cx, id| {
            if cx.has_global::<Views>() {
                cx.global_mut::<Views>()
                    .forms
                    .retain(|(window, _), _| *window != id);
            }
        });
        cx.set_global(Views {
            forms: HashMap::new(),
            _closed: closed,
        });
    }
    let key = (window.window_handle().window_id(), profile.clone());
    if let Some(form) = cx.global::<Views>().forms.get(&key) {
        return form.clone().into_any_element();
    }
    let workspace = owner
        .project()
        .map(|view| view.read(cx).workspace.root().to_owned());
    let form = cx.new(|cx| HistorySettings::new(profile, workspace, window, cx));
    cx.global_mut::<Views>().forms.insert(key, form.clone());
    form.into_any_element()
}

/// A settings draft is independent from the active policy until the atomic user-profile write succeeds.
struct HistorySettings {
    /// Explicit user profile, never a project settings path.
    profile: PathBuf,
    /// Only used to reject data destinations inside the current project, never to load project policy.
    workspace: Option<PathBuf>,
    /// Policy destination and active profile worker may be retried after initialization failure.
    paths: Option<platform_windows::history::HistoryPaths>,
    worker: Option<Worker>,
    /// Native draft controls keep their own input/IME state.
    enabled: bool,
    days: Entity<InputState>,
    budget: Entity<InputState>,
    file_limit: Entity<InputState>,
    exclude: Entity<InputState>,
    /// Pending operations are bounded by the single apply/cleanup action in this form.
    busy: bool,
    error: Option<String>,
    stats: Option<Stats>,
    clear: Option<editor_history::Cleanup>,
}

impl HistorySettings {
    /// Start loading off-thread without making the settings render perform any storage/codec work.
    fn new(
        profile: PathBuf,
        workspace: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let state = crate::editor::history::State::new(Some(&profile), workspace.as_deref(), cx);
        let mut input = |value: &str, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).default_value(value))
        };
        let mut form = Self {
            profile,
            workspace,
            paths: state.paths,
            worker: state.worker,
            enabled: true,
            days: input("30", cx),
            budget: input("1000", cx),
            file_limit: input("8", cx),
            exclude: input(&Policy::default().exclude.join("; "), cx),
            busy: false,
            error: state.last_error,
            stats: None,
            clear: None,
        };
        form.reload(window, cx);
        form
    }

    /// Populate once from the worker's validated user policy and physical usage counters.
    fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(worker) = self.worker.clone() else {
            return;
        };
        self.busy = true;
        let loaded = worker.call(0, |history| Ok((history.policy()?, history.stats()?)));
        cx.spawn_in(window, async move |form, cx| {
            let result = loaded.await;
            let _ = form.update_in(cx, |form, window, cx| {
                form.busy = false;
                match result {
                    Ok((policy, stats)) => {
                        form.enabled = policy.enabled;
                        for (input, value) in [
                            (&form.days, policy.retention_days.to_string()),
                            (
                                &form.budget,
                                (policy.max_total_bytes / 1_000_000).to_string(),
                            ),
                            (
                                &form.file_limit,
                                (policy.max_file_bytes / (1024 * 1024)).to_string(),
                            ),
                            (&form.exclude, policy.exclude.join("; ")),
                        ] {
                            input.update(cx, |input, cx| input.set_value(value, window, cx));
                        }
                        form.stats = Some(stats);
                        form.error = None;
                    }
                    Err(error) => form.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Parse the whole draft before publishing any setting; invalid input preserves the active policy.
    fn draft(&self, cx: &App) -> anyhow::Result<Policy> {
        let integer = |input: &Entity<InputState>| {
            input
                .read(cx)
                .value()
                .trim()
                .parse::<u64>()
                .map_err(|_| anyhow::anyhow!(t!("history.invalid_settings")))
        };
        let policy = Policy {
            enabled: self.enabled,
            retention_days: u32::try_from(integer(&self.days)?)
                .map_err(|_| anyhow::anyhow!(t!("history.invalid_settings")))?,
            max_total_bytes: integer(&self.budget)?
                .checked_mul(1_000_000)
                .ok_or_else(|| anyhow::anyhow!(t!("history.invalid_settings")))?,
            max_file_bytes: integer(&self.file_limit)?
                .checked_mul(1024 * 1024)
                .ok_or_else(|| anyhow::anyhow!(t!("history.invalid_settings")))?,
            exclude: self
                .exclude
                .read(cx)
                .value()
                .split(';')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .collect(),
        };
        policy.validate()?;
        Ok(policy)
    }

    /// Persist valid settings first, then update the shared policy and perform quota/expiry maintenance.
    fn apply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let policy = match self.draft(cx) {
            Ok(policy) => policy,
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        let Some(paths) = self.paths.clone() else {
            return;
        };
        self.busy = true;
        self.error = None;
        self.clear = None;
        let profile = self.profile.clone();
        let workspace = self.workspace.clone();
        cx.spawn_in(window, async move |form, cx| {
            let written = cx
                .background_executor()
                .spawn(async move {
                    use editor_core::DocumentStore as _;
                    std::fs::create_dir_all(
                        paths
                            .settings
                            .parent()
                            .ok_or_else(|| anyhow::anyhow!("Missing profile"))?,
                    )?;
                    NativeFileStore
                        .write_utf8(&paths.settings, &serde_json::to_string_pretty(&policy)?)?;
                    Ok::<_, anyhow::Error>(policy)
                })
                .await;
            let applied = form
                .update_in(cx, |form, _, cx| {
                    let policy = match written {
                        Ok(policy) => policy,
                        Err(error) => {
                            form.busy = false;
                            form.error = Some(error.to_string());
                            cx.notify();
                            return None;
                        }
                    };
                    let state = crate::editor::history::State::new(
                        Some(&profile),
                        workspace.as_deref(),
                        cx,
                    );
                    form.worker = state.worker.clone();
                    state.worker.map(|worker| {
                        worker.call(0, move |history| {
                            history.set_policy(policy)?;
                            history.maintain(crate::editor::history::now_ms())?;
                            history.stats()
                        })
                    })
                })
                .ok()
                .flatten();
            if let Some(applied) = applied {
                let result = applied.await;
                let _ = form.update_in(cx, |form, _, cx| {
                    form.busy = false;
                    match result {
                        Ok(stats) => form.stats = Some(stats),
                        Err(error) => form.error = Some(error.to_string()),
                    }
                    cx.notify();
                });
            }
        })
        .detach();
        cx.notify();
    }

    /// Empty-window settings can clear profile history using the same preview/confirmation contract.
    fn cleanup(&mut self, confirm: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(worker) = self.worker.clone() else {
            return;
        };
        let previous = self.clear.take();
        self.busy = true;
        let result = worker.call(0, move |history| {
            if confirm {
                history.clear_confirmed(
                    &editor_history::ClearScope::All,
                    previous.ok_or(editor_history::Error::Stale)?,
                )?;
            }
            Ok((
                (!confirm)
                    .then(|| history.preview_clear(&editor_history::ClearScope::All))
                    .transpose()?,
                history.stats()?,
            ))
        });
        cx.spawn_in(window, async move |form, cx| {
            let result = result.await;
            let _ = form.update_in(cx, |form, _, cx| {
                form.busy = false;
                match result {
                    Ok((preview, stats)) => {
                        form.clear = preview;
                        form.stats = Some(stats);
                    }
                    Err(error) => form.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

impl Render for HistorySettings {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let fields = [
            ("history-days", "history.days", &self.days),
            ("history-budget", "history.budget", &self.budget),
            ("history-file-limit", "history.file_limit", &self.file_limit),
            ("history-exclude", "history.exclude", &self.exclude),
        ];
        v_flex()
            .debug_selector(|| "history-settings".into())
            .w_full()
            .gap_3()
            .child(
                Checkbox::new("history-enabled")
                    .label(t!("history.enabled").to_string())
                    .checked(self.enabled)
                    .disabled(self.busy)
                    .on_change(move |checked, _, cx| {
                        let _ = owner.update(cx, |form, cx| {
                            form.enabled = *checked;
                            cx.notify();
                        });
                    }),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("history.settings_hint").to_string()),
            )
            .children(fields.into_iter().map(|(id, label, input)| {
                v_flex()
                    .gap_1()
                    .child(div().text_sm().child(t!(label).to_string()))
                    .child(div().id(id).debug_selector(move || id.into()).child(
                        editor_ui::Input::new(input).accessibility_label(t!(label).to_string()),
                    ))
            }))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("history-settings-apply")
                            .debug_selector(|| "history-settings-apply".into())
                            .label(t!("history.apply_settings").to_string())
                            .primary()
                            .disabled(self.busy)
                            .on_click(cx.listener(|form, _, window, cx| form.apply(window, cx))),
                    )
                    .child(
                        Button::new("history-settings-clear")
                            .label(t!("history.clear_all").to_string())
                            .disabled(self.busy)
                            .on_click(
                                cx.listener(|form, _, window, cx| form.cleanup(false, window, cx)),
                            ),
                    ),
            )
            .children(self.stats.map(|stats| {
                div().text_sm().child(
                    t!(
                        "history.usage",
                        versions = stats.versions,
                        objects = stats.objects,
                        bytes = stats.disk_bytes + stats.legacy_bytes,
                        wal = stats.wal_bytes,
                        pending = stats.pending_gc_bytes
                    )
                    .to_string(),
                )
            }))
            .children(self.clear.map(|preview| {
                v_flex()
                    .gap_2()
                    .child(
                        t!(
                            "history.clear_preview",
                            scope = t!("history.clear_all"),
                            count = preview.versions + preview.legacy,
                            bytes = preview.reclaimable_bytes
                        )
                        .to_string(),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("history-settings-clear-confirm")
                                    .label(t!("history.confirm_clear").to_string())
                                    .danger()
                                    .on_click(cx.listener(|form, _, window, cx| {
                                        form.cleanup(true, window, cx)
                                    })),
                            )
                            .child(
                                Button::new("history-settings-clear-cancel")
                                    .label(t!("history.cancel").to_string())
                                    .on_click(cx.listener(|form, _, _, cx| {
                                        form.clear = None;
                                        cx.notify();
                                    })),
                            ),
                    )
            }))
            .children(
                self.error
                    .clone()
                    .map(|error| div().text_sm().text_color(cx.theme().danger).child(error)),
            )
    }
}
