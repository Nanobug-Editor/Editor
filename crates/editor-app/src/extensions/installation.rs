//! Installation feedback remains in a dialog while work runs independently from the UI event loop.
use super::*;
use editor_ui::scroll::ScrollableElement as _;

/// Render a shared left-aligned footer choice backed by this confirmation's `selected` value.
/// The caller creates a fresh unchecked value whenever it opens an installation confirmation.
pub(super) fn global_enable_option(selected: Rc<Cell<bool>>) -> impl IntoElement {
    h_flex()
        .id("plugin-install-global-option")
        .debug_selector(|| "plugin-install-global-option".into())
        .flex_1()
        .items_center()
        .child(
            editor_ui::Checkbox::new("plugin-install-global-enabled")
                .label(t!("plugins.install_global_enabled").to_string())
                .checked(selected.get())
                .on_change(move |checked, _, cx| {
                    selected.set(*checked);
                    cx.refresh_windows();
                }),
        )
}

/// Observe the manager's publications while the native modal window owns its local controls and focus.
struct InstallationView {
    worker: Arc<Worker>,
    control: Option<plugin_runtime::InstallControl>,
    /// SDK consent is local to one displayed prompt, never carried into another installer step.
    sdk_prompt: Option<u64>,
    sdk_selected: bool,
    /// Expand the initially compact native window once when asynchronous consent becomes visible.
    consent_expanded: bool,
    _subscription: Subscription,
}
impl Drop for InstallationView {
    fn drop(&mut self) {
        // Native close and Escape are also explicit cancellation while preparation is in flight.
        if self
            .worker
            .state
            .lock()
            .unwrap()
            .installation
            .as_ref()
            .is_some_and(|report| report.cancellable)
        {
            if let Some(control) = &self.control {
                control.cancel();
            }
        }
    }
}
impl Render for InstallationView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let report = self.worker.state.lock().unwrap().installation.clone();
        let message = report
            .as_ref()
            .map(|report| report.message.clone())
            .unwrap_or_else(|| t!("plugin_installation.finished").to_string());
        let cancellable = report.is_some_and(|report| report.cancellable);
        let prompt = self
            .control
            .as_ref()
            .and_then(|control| control.installer_prompt());
        if prompt.is_some() && !self.consent_expanded {
            // Preparation can publish consent after opening. Keep plain progress compact, then
            // give the fixed consent footer room without shrinking a window the user enlarged.
            self.consent_expanded = true;
            let viewport = window.viewport_size();
            window.resize(size(
                viewport.width.max(px(520.)),
                viewport.height.max(px(360. + PANEL_HEADER_HEIGHT)),
            ));
        }
        if self.sdk_prompt != prompt.as_ref().map(|prompt| prompt.id) {
            self.sdk_prompt = prompt.as_ref().map(|prompt| prompt.id);
            self.sdk_selected = false;
        }
        v_flex()
            .size_full()
            .p_4()
            .gap_4()
            .child(
                v_flex()
                    .id("plugin-install-progress")
                    .debug_selector(|| "plugin-install-progress".into())
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .gap_3()
                    .child(message)
                    .when_some(prompt.clone(), |view, prompt| {
                        view.child(
                            v_flex()
                                .debug_selector(|| "plugin-install-prompt-details".into())
                                // Preserve the natural detail height inside the bounded scroller.
                                .flex_shrink_0()
                                .w_full()
                                .gap_2()
                                .when_some(prompt.source, |details, source| {
                                    details.child(
                                        t!("plugin_installation.source", value = source)
                                            .to_string(),
                                    )
                                })
                                .child(
                                    t!("plugin_installation.purpose", value = prompt.purpose)
                                        .to_string(),
                                )
                                .child(
                                    t!(
                                        "plugin_installation.program",
                                        value =
                                            editor_core::display_path(&prompt.program).to_string()
                                    )
                                    .to_string(),
                                )
                                .child(
                                    t!(
                                        "plugin_installation.arguments",
                                        value = format!("{:?}", prompt.args)
                                    )
                                    .to_string(),
                                )
                                .child(
                                    t!(
                                        "plugin_installation.target",
                                        value =
                                            editor_core::display_path(&prompt.target).to_string()
                                    )
                                    .to_string(),
                                )
                                .child(t!("plugin_installation.native_notice").to_string()),
                        )
                    }),
            )
            // Consent stays outside the details scroller: an off-screen button cannot authorize execution.
            .when_some(prompt, |view, prompt| {
                let entity = cx.entity();
                let sdk_required = prompt.project_sdk;
                let id = prompt.id;
                let preparation = prompt.preparation_only;
                view.child(
                    v_flex()
                        .flex_shrink_0()
                        .gap_2()
                        .when(sdk_required, |details| {
                            details.child(
                                div()
                                    .id("plugin-install-sdk-region")
                                    .debug_selector(|| "plugin-install-sdk-region".into())
                                    .child(
                                        editor_ui::Checkbox::new("plugin-install-sdk-opt-in")
                                            .label(t!("plugin_installation.sdk_opt_in").to_string())
                                            .checked(self.sdk_selected)
                                            .on_change(move |checked, _, cx| {
                                                entity.update(cx, |this, cx| {
                                                    this.sdk_selected = *checked;
                                                    cx.notify();
                                                })
                                            }),
                                    ),
                            )
                        })
                        .child(
                            div()
                                .id("plugin-install-native-confirm-region")
                                .debug_selector(|| "plugin-install-native-confirm-region".into())
                                .child(
                                    Button::new("plugin-install-native-confirm")
                                        .label(
                                            t!(if preparation {
                                                "plugin_installation.confirm_sdk"
                                            } else {
                                                "plugin_installation.confirm_step"
                                            })
                                            .to_string(),
                                        )
                                        .outline()
                                        .disabled(sdk_required && !self.sdk_selected)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if let Some(control) = &this.control {
                                                control.approve_installer(id, this.sdk_selected);
                                            }
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
            })
            .child(
                // Keep the action visible while long status or consent details scroll above it.
                h_flex().flex_shrink_0().justify_end().child(
                    div()
                        .id("plugin-install-progress-close-region")
                        .debug_selector(|| "plugin-install-progress-close-region".into())
                        .child(
                            Button::new("plugin-install-progress-close")
                                .label(if cancellable {
                                    t!("plugin_installation.cancel").to_string()
                                } else {
                                    t!("plugin_installation.close").to_string()
                                })
                                .outline()
                                .on_click(cx.listener(move |this, _, window, _| {
                                    if cancellable {
                                        if let Some(control) = &this.control {
                                            control.cancel();
                                        }
                                    } else {
                                        window.remove_window();
                                    }
                                })),
                        ),
                ),
            )
    }
}

impl ExtensionPanel {
    pub(super) fn open_installation_progress(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let worker = self.worker.clone();
        let owner = cx.entity();
        let view = cx.new(|cx| InstallationView {
            control: worker.state.lock().unwrap().install_control.clone(),
            sdk_prompt: None,
            sdk_selected: false,
            consent_expanded: false,
            worker: worker.clone(),
            _subscription: cx.observe(&owner, |_, _, cx| cx.notify()),
        });
        app_dialog::open_dialog_sized(
            t!("plugin_installation.title").to_string(),
            // Ordinary preparation stays compact; the view expands when consent arrives asynchronously.
            520.,
            200.,
            // Bound the flex content to the viewport so long messages cannot push out the footer.
            move |content, _, _| content.min_h_0().overflow_hidden().child(view.clone()),
            cx,
        );
    }

    /// Only the current immutable plan may report readiness; a late previous-version callback is ignored.
    pub(crate) fn language_service_status(
        &self,
        key: &str,
        plan: &Arc<plugin_runtime::LanguageService>,
        level: plugin_runtime::logs::LogLevel,
        message: String,
        cx: &mut Context<Self>,
    ) {
        // A retired plan can remain in an older publication while cancellation finishes asynchronously.
        // Its matching Arc is no longer authority to turn an intentional stop into a new error.
        if !plan.is_active() {
            return;
        }
        let mut state = self.worker.state.lock().unwrap();
        if !state
            .language_services
            .get(key)
            .and_then(|item| item.as_ref().ok())
            .is_some_and(|current| Arc::ptr_eq(current, plan))
        {
            return;
        }
        if state.service_states.get(key) == Some(&message) {
            return;
        }
        // The provider lease makes retirement and append atomic. This host status has no separate
        // transport epoch; its authority is the current published plan checked above.
        if plan
            .append_runtime_log(
                &std::sync::atomic::AtomicBool::new(false),
                level,
                &format!("language.status:{key}"),
                message.clone(),
            )
            .is_none()
        {
            return;
        }
        state.service_states.insert(key.into(), message);
        let text = state
            .service_states
            .iter()
            .filter(|(key, _)| key.starts_with(&format!("{}/", plan.owner)))
            .map(|(key, value)| format!("{key}：{value}"))
            .collect::<Vec<_>>()
            .join("\n");
        if let Some(report) = &mut state.installation {
            if report.id == plan.owner && report.installed {
                report.message = format!("插件已安装。\n{text}");
            }
        }
        drop(state);
        cx.notify();
    }
}
