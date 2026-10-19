//! Plugin management and the native document surface share the editor's controls and theme.
use super::*;
use crate::ui::controls::ButtonCustomVariant;
use crate::ui::controls::dialog::DialogAction;
use crate::ui::controls::dialog::DialogFooter;
use crate::ui::controls::overlay::WindowExt as _;
use gpui_base::DialogClose;
use gpui_kit::rgb;
impl ExtensionPanel {
    /// A store recovery failure has no plugin log owner and remains a separate manager diagnostic.
    pub(crate) fn unowned_manager_error(&self) -> Option<&str> {
        if self.manager_error_confirmed.get() {
            return None;
        }
        self.status
            .as_ref()
            .filter(|status| status.plugin.is_none())?;
        self.manager_error()
    }

    /// Confirm only the currently displayed ownerless status; keep its details for plugin management.
    /// Worker statuses delivered after this click reset the flag even when their messages are identical.
    pub(crate) fn confirm_unowned_manager_error(&self) {
        if self.unowned_manager_error().is_some() {
            self.manager_error_confirmed.set(true);
        }
    }

    /// Return a current operation failure not already displayed by an installed entry.
    /// Recovery, inspection and first preparation may fail before an entry exists. Exact identity
    /// and message matching avoids counting the same failure twice while retaining newer operation errors.
    pub(crate) fn manager_error(&self) -> Option<&str> {
        self.status
            .as_ref()
            .filter(|status| {
                !self.entries.iter().any(|entry| {
                    status.plugin.as_deref() == Some(entry.manifest.id.as_str())
                        && entry.error.as_deref() == Some(status.message.as_str())
                })
            })
            .map(|status| status.message.as_str())
    }

    /// Both a preview body and a source-only toolbar deliver theme and locale to their live owner.
    /// Cold sampling still supplies native context, but cannot acknowledge delivery before birth.
    pub(super) fn native_environment(&mut self, cx: &mut Context<Self>) -> protocol::Environment {
        let environment = environment(&self.workspace, cx);
        if self.instance_epoch == 0 {
            return environment;
        }
        if self.last_theme.as_ref() != Some(&environment) {
            self.last_theme = Some(environment.clone());
            for entry in &self.entries {
                if entry.enabled && Some(&entry.manifest.id) == self.active.as_ref() {
                    self.send_to(
                        &entry.manifest.id,
                        None,
                        PluginEvent::Theme(environment.clone()),
                    );
                }
            }
        }
        environment
    }

    /// Preview content and source-mode overlays share one keyed native view and event ownership.
    pub(super) fn native_document(
        &mut self,
        mut document: protocol::ui::Document,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<crate::ui::plugin::PluginView> {
        document.editor_toolbar = None;
        let environment = self.native_environment(cx);
        if let Some(view) = &self.native_ui {
            view.update(cx, |view, cx| {
                view.update_document(document, environment, window, cx)
            });
        } else {
            let tx = self.worker.tx.clone();
            let plugin = self
                .active
                .clone()
                .expect("an approved document has an owner");
            let panel = self
                .surface_id
                .clone()
                .expect("an approved document has a surface");
            let visible = self.visible.clone();
            let epoch = self.instance_epoch;
            let form_changes =
                self.worker
                    .form_change_sink(plugin.clone(), panel.clone(), epoch, visible.clone());
            self.native_ui = Some(cx.new(|cx| {
                crate::ui::plugin::PluginView::new(
                    plugin.clone(),
                    document,
                    environment,
                    move |event, _| {
                        if visible.get() {
                            let _ = tx.send(Work::Event(
                                plugin.clone(),
                                epoch,
                                Some(panel.clone()),
                                PluginEvent::Ui(event),
                            ));
                        }
                    },
                    window,
                    cx,
                )
                .with_form_changes(form_changes)
            }));
        }
        let key = format!(
            "{}/{}",
            self.active.as_deref().unwrap_or_default(),
            self.surface_id.as_deref().unwrap_or_default()
        );
        let view = self.native_ui.as_ref().unwrap().clone();
        let interactive = self.current_document().is_some();
        let parent = self.parent.clone();
        let expected_key = key.clone();
        let epoch = self.instance_epoch;
        let work_window = self.work_window;
        let window_binding = self.work_window_source.clone();
        let sync_source = window_binding
            .as_ref()
            .filter(|binding| {
                self.visible.get()
                    && binding
                        .virtual_resource
                        .as_ref()
                        .is_none_or(|resource| resource.is_live())
            })
            .map(|binding| (binding.document.clone(), binding.editor.clone()))
            .or_else(|| {
                (!work_window)
                    .then(|| self.main_code_source.clone())
                    .flatten()
            });
        let visibility = self.visible.clone();
        let editor: crate::ui::plugin::NativeEditorRenderer =
            Rc::new(move |target, actions, alignment, window, cx| {
                if work_window {
                    if !visibility.get() {
                        return None;
                    }
                    let Some(binding) = &window_binding else {
                        return Some(
                            div()
                                .p_4()
                                .child(t!("editor.detached_source").to_string())
                                .into_any_element(),
                        );
                    };
                    if binding.document.id != target.id
                        || binding.document.path != target.path
                        || binding.document.revision < target.revision
                        || binding
                            .virtual_resource
                            .as_ref()
                            .is_some_and(|resource| !resource.is_live())
                    {
                        return None;
                    }
                    // PluginView disables stale buttons and rechecks exact authority on dispatch;
                    // retaining their rails prevents a native edit from changing pane width.
                    let body = alignment
                        .as_ref()
                        .and_then(|(group, index)| group.pane(*index));
                    let source = crate::ui::controls::window_editor_body(&binding.editor, body, cx);
                    return Some(if let Some(actions) = actions {
                        let projection = alignment.map(|(group, index)| {
                            Rc::new(move |offset, cx: &App| {
                                group.project_range(index, offset..offset, cx)
                            })
                                as crate::ui::controls::EditorActionProjection
                        });
                        crate::ui::controls::with_projected_editor_actions(
                            source,
                            &binding.editor,
                            actions,
                            projection,
                        )
                    } else {
                        source
                    });
                }
                parent
                    .update(cx, |app, cx| {
                        let index = app.active_text_tab_index()?;
                        let selected = app.active_editor_preview(cx)?;
                        let current = app.plugin_document_version(index).ok()?;
                        // Borrowing current text for retained geometry grants no stale guest edit authority.
                        if current.id != target.id
                            || current.path != target.path
                            || target.revision > current.revision
                            || app.editor_preview_owner_key(&selected, cx).as_ref()
                                != Some(&expected_key)
                            || selected.read(cx).instance_epoch != epoch
                        {
                            return None;
                        }
                        let source = if let Some((group, index)) = &alignment {
                            app.render_native_editor_aligned(group, *index, window, cx)
                        } else {
                            app.render_native_editor(window, cx)
                        };
                        let source = if let Some(actions) = actions {
                            let projection = alignment.map(|(group, index)| {
                                Rc::new(move |offset, cx: &App| {
                                    group.project_range(index, offset..offset, cx)
                                })
                                    as crate::ui::controls::EditorActionProjection
                            });
                            crate::ui::controls::with_projected_editor_actions(
                                source,
                                &app.editor,
                                actions,
                                projection,
                            )
                        } else {
                            source
                        };
                        Some(app.render_source_viewport_probe(source, selected, cx))
                    })
                    .ok()
                    .flatten()
            });
        let action_parent = self.parent.clone();
        view.update(cx, |view, cx| {
            view.set_scene_current(interactive, cx);
            view.native_editor = Some(editor);
            view.set_code_sync_source(sync_source, cx);
            view.action_source_current = Some(Rc::new(move |source, cx| {
                action_parent.upgrade().is_some_and(|parent| {
                    let app = parent.read(cx);
                    (0..app.tabs.len()).any(|index| {
                        app.plugin_document_version(index)
                            .is_ok_and(|current| current == *source)
                    })
                })
            }));
            view.set_viewport_enabled(self.viewport_sync_enabled, cx);
            view.update_trees(&self.trees, cx);
            view.update_images(&key, &self.images, window, cx)
        });
        view
    }

    /// Show package origin and requested capabilities above the manager's README.
    pub(super) fn open_install_dialog(
        &mut self,
        package: impl Into<Arc<Package>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Repaints retain one immutable package; only an accepted manual install needs an owned clone.
        let package = package.into();
        let executable =
            package.manifest.component.is_some() || !package.manifest.services.is_empty();
        let name = package.manifest.name.clone();
        let version = package.manifest.version.clone();
        let current = self
            .entries
            .iter()
            .find(|entry| entry.manifest.id == package.manifest.id)
            .map(|entry| entry.manifest.version.as_str());
        let action = match package_action(&version, current) {
            "已安装" => "重新安装",
            action => action,
        };
        let action_label = match action {
            "更新" => t!("plugins.update"),
            "降级安装" => t!("plugins.downgrade"),
            "重新安装" => t!("plugins.reinstall"),
            _ => t!("plugins.install"),
        }
        .to_string();
        let title = t!(
            "plugins.consent_title",
            action = action_label.clone(),
            name = name.clone(),
            version = version.clone()
        )
        .to_string();
        let confirm_label =
            t!("plugins.consent_confirm", action = action_label.clone()).to_string();
        let source = package.source.clone();
        let permissions = package.manifest.permissions.clone();
        let services = package.manifest.services.clone();
        let replacement_ids: Vec<_> = package
            .manifest
            .replaces
            .iter()
            .map(|rule| rule.id.clone())
            .collect();
        // Updating/reinstalling retires the same original owners as disable/uninstall. Display their
        // targets in this confirmation too; preparing a failed update must leave them running.
        let impact_parent = self.parent.clone();
        let mut impact_ids = replacement_ids.clone();
        impact_ids.push(package.manifest.id.clone());
        let owner = cx.entity().downgrade();
        let install_owner = owner.clone();
        // First-use may ask during EditorApp::render, when the parent entity is borrowed.
        // Read session impact and open the native confirmation only after that borrow has ended.
        window.defer(cx, move |window, cx| {
            let impact = impact_parent.upgrade().and_then(|parent| {
                parent
                    .read(cx)
                    .run_controls
                    .debug_availability()
                    .ok()
                    .map(str::to_owned)
            });
            let impact = impact_parent
                .upgrade()
                .map(|parent| {
                    impact_ids
                        .iter()
                        .filter_map(|id| {
                            parent
                                .read(cx)
                                .run_controls
                                .plugin_session_impact(id, impact.as_deref())
                                .summary()
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .filter(|text| !text.is_empty());
            window.open_dialog(cx, move |dialog, _, cx| {
                let source = source.clone();
                let permissions = permissions.clone();
                let services = services.clone();
                let impact = impact.clone();
                let replacement_ids = replacement_ids.clone();
                let install_package = package.clone();
                let action_label = action_label.clone();
                let install_owner = install_owner.clone();
                let cancel_owner = owner.clone();
                dialog
                    .title(title.clone())
                    .width(px(520.))
                    .overlay_closable(false)
                    .close_button(false)
                    .content(move |content, _, cx| {
                        let mut details = v_flex()
                            .id("plugin-install-consent")
                            .debug_selector(|| "plugin-install-consent".into())
                            .gap_3()
                            .child(
                                div()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(t!("plugins.consent_unsigned").to_string()),
                            );
                        if let Some(source) = &source {
                            details = details
                                .child(t!("plugins.consent_source", source = source).to_string());
                        }
                        if let Some(impact) = &impact {
                            details = details.child(
                                div()
                                    .debug_selector(|| "plugin-install-session-impact".into())
                                    .child(impact.clone()),
                            );
                        }
                        if !replacement_ids.is_empty() {
                            details = details.child(
                                div()
                                    .debug_selector(|| "plugin-install-replacements".into())
                                    .child(
                                        t!(
                                            "plugins.consent_replacements",
                                            ids = replacement_ids.join(", ")
                                        )
                                        .to_string(),
                                    ),
                            );
                        }
                        details = details.child(if executable {
                            t!(
                                "plugins.consent_capabilities",
                                action = action_label.clone()
                            )
                            .to_string()
                        } else {
                            t!("plugins.consent_resources").to_string()
                        });
                        for permission in &permissions {
                            let explanation = match permission.as_str() {
                                "assets.read" => t!("plugins.permission_assets"),
                                "process.exec" => t!("plugins.permission_process"),
                                "native.modules" => t!("plugins.permission_native_modules"),
                                "dependencies.prepare" => {
                                    t!("plugins.permission_dependencies_prepare")
                                }
                                "dependencies.install" => {
                                    t!("plugins.permission_dependencies_install")
                                }
                                value if value.starts_with("process.service.") => {
                                    t!("plugins.permission_service")
                                }
                                "editor.read" => t!("plugins.permission_editor_read"),
                                "editor.write" => t!("plugins.permission_editor_write"),
                                "workspace.read" => t!("plugins.permission_workspace_read"),
                                "workspace.write" => t!("plugins.permission_workspace_write"),
                                "network.images" => t!("plugins.permission_network_images"),
                                "navigation.external" => {
                                    t!("plugins.permission_external_navigation")
                                }
                                "clipboard" => t!("plugins.permission_clipboard"),
                                "storage" => t!("plugins.permission_storage"),
                                _ => std::borrow::Cow::Borrowed(permission.as_str()),
                            }
                            .to_string();
                            details = details.child(format!("• {explanation}"));
                            if let Some(service) = permission
                                .strip_prefix("process.service.")
                                .and_then(|id| services.get(id))
                            {
                                // Show the approved executable and argument vector separately from prose.
                                details = details.child(
                                    t!(
                                        "plugins.consent_program",
                                        program = service.program.clone(),
                                        args = format!("{:?}", service.args)
                                    )
                                    .to_string(),
                                );
                                if let Some(plan) = &service.installation {
                                    for artifact in &plan.artifacts {
                                        details = details.child(
                                            t!(
                                                "plugins.consent_dependency",
                                                id = artifact.id.clone(),
                                                version = artifact.version.clone(),
                                                platform = artifact.platform.clone(),
                                                source = format!("{:?}", artifact.source),
                                                sha256 = artifact.sha256.clone()
                                            )
                                            .to_string(),
                                        );
                                    }
                                }
                            }
                        }
                        details = details.child(if executable {
                            if action == "安装" {
                                t!("plugins.consent_after_install").to_string()
                            } else {
                                t!("plugins.consent_after_update").to_string()
                            }
                        } else {
                            t!("plugins.consent_resource_install", action = action_label)
                                .to_string()
                        });
                        content.child(details)
                    })
                    // Base Dialog needs an explicit footer; button props only label actions.
                    .footer(
                        DialogFooter::new()
                            .child(
                                div()
                                    .id("plugin-install-cancel")
                                    .debug_selector(|| "plugin-install-cancel".into())
                                    .child(gpui_base::DialogClose::new().trigger(|base| {
                                        Button::new("plugin-install-cancel-button")
                                            .with_base(base)
                                            .label(t!("plugins.cancel").to_string())
                                    })),
                            )
                            .child(
                                div()
                                    .id("plugin-install-confirm")
                                    .debug_selector(|| "plugin-install-confirm".into())
                                    .child(
                                        DialogAction::new().child(
                                            Button::new("confirm-plugin-install")
                                                .label(confirm_label.clone())
                                                .primary()
                                                .when(action == "更新", |button| {
                                                    button.custom(update_button_style(cx))
                                                })
                                                .outline(),
                                        ),
                                    ),
                            ),
                    )
                    .on_ok(move |_, _, cx| {
                        install_owner
                            .update(cx, |this, cx| {
                                // Explicit consent submits the same inspected package through the ordinary lifecycle.
                                let work = Work::Install(install_package.as_ref().clone());
                                if this.queue_lifecycle(work) {
                                    this.pending = None;
                                    this.pending_dialog_open = false;
                                    cx.notify();
                                    true
                                } else {
                                    false
                                }
                            })
                            .unwrap_or(false)
                    })
                    .on_cancel(move |_, _, cx| {
                        let _ = cancel_owner.update(cx, |this, cx| {
                            this.pending = None;
                            this.pending_dialog_open = false;
                            cx.notify();
                        });
                        true
                    })
            });
        });
    }

    /// Confirm the uninstall impact and keep both data-retention choices visible.
    ///
    /// `extra_impact` names the sessions the plugin is serving, which the caller computes because
    /// only it can see the run controls. Naming them before the change is what lets a user cancel
    /// instead of discovering afterwards that a program or a debug session was taken away.
    pub(super) fn open_remove_dialog(
        &mut self,
        id: String,
        remove: bool,
        extra_impact: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entry = self.entries.iter().find(|entry| entry.manifest.id == id);
        let name = entry
            .map(|entry| entry.manifest.name.clone())
            .unwrap_or_else(|| id.clone());
        let executable = entry.is_some_and(|entry| {
            entry.manifest.component.is_some() || !entry.manifest.services.is_empty()
        });
        let count = self.processes.get(&id).copied().unwrap_or(0);
        let mut impact = if executable {
            t!("plugins.remove_programs", count = count.to_string()).to_string()
        } else {
            t!("plugins.remove_resources").to_string()
        };
        // Sessions this plugin is serving are named before the change rather than discovered
        // afterwards. Cancelling leaves both the sessions and the plugin exactly as they are, so this
        // only informs the decision.
        if let Some(summary) = extra_impact {
            impact.push('\n');
            impact.push_str(&summary);
        }
        let owner = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let preserve_owner = owner.clone();
            let delete_owner = owner.clone();
            let cancel_owner = owner.clone();
            let preserve_id = id.clone();
            let delete_id = id.clone();
            let impact = impact.clone();
            dialog
                .title(
                    t!(
                        "plugins.remove_title",
                        action = if remove {
                            t!("plugins.uninstall")
                        } else {
                            t!("plugins.disable")
                        },
                        name = &name
                    )
                    .to_string(),
                )
                .width(px(520.))
                .overlay_closable(false)
                .close_button(false)
                .content(move |content, _, cx| {
                    content.child(
                        v_flex()
                            .id("plugin-remove-consent")
                            .debug_selector(|| "plugin-remove-consent".into())
                            .gap_3()
                            .child(impact.clone())
                            .when(remove, |details| {
                                details.child(
                                    div()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(t!("plugins.remove_data_hint").to_string()),
                                )
                            }),
                    )
                })
                // Base Dialog renders these explicit choices in its visible footer.
                .footer(
                    DialogFooter::new()
                        .child(
                            div()
                                .id("plugin-remove-cancel")
                                .debug_selector(|| "plugin-remove-cancel".into())
                                .child(DialogClose::new().trigger(|base| {
                                    Button::new("plugin-remove-cancel-action")
                                        .with_base(base)
                                        .label(t!("plugins.cancel"))
                                })),
                        )
                        .child(
                            div()
                                .id("plugin-remove-preserve")
                                .debug_selector(|| "plugin-remove-preserve".into())
                                .child(
                                    DialogAction::new().child(
                                        Button::new("confirm-plugin-preserve")
                                            .label(if remove {
                                                t!("plugins.remove_preserve")
                                            } else {
                                                t!("plugins.confirm_disable")
                                            })
                                            .primary()
                                            .when(remove, |button| {
                                                button.custom(uninstall_button_style(cx))
                                            })
                                            .outline(),
                                    ),
                                ),
                        )
                        .when(remove, |footer| {
                            footer.child(
                                div()
                                    .id("plugin-remove-delete")
                                    .debug_selector(|| "plugin-remove-delete".into())
                                    .child(
                                        Button::new("confirm-plugin-delete")
                                            .label(t!("plugins.remove_delete"))
                                            .custom(uninstall_button_style(cx))
                                            .outline()
                                            .on_click(move |_, window, cx| {
                                                let queued = delete_owner
                                                    .update(cx, |this, cx| {
                                                        let queued =
                                                            this.queue_lifecycle(Work::Uninstall(
                                                                delete_id.clone(),
                                                                true,
                                                            ));
                                                        if queued {
                                                            this.confirm = None;
                                                            this.confirm_dialog_open = false;
                                                            cx.notify();
                                                        }
                                                        queued
                                                    })
                                                    .unwrap_or(false);
                                                if queued {
                                                    window.close_dialog(cx);
                                                }
                                            }),
                                    ),
                            )
                        }),
                )
                .on_ok(move |_, _, cx| {
                    preserve_owner
                        .update(cx, |this, cx| {
                            let queued = if remove {
                                this.queue_lifecycle(Work::Uninstall(preserve_id.clone(), false))
                            } else {
                                this.worker
                                    .tx
                                    .send(Work::Disable(preserve_id.clone()))
                                    .is_ok()
                            };
                            if queued {
                                this.confirm = None;
                                this.confirm_dialog_open = false;
                                cx.notify();
                            }
                            queued
                        })
                        .unwrap_or(false)
                })
                .on_cancel(move |_, _, cx| {
                    let _ = cancel_owner.update(cx, |this, cx| {
                        this.confirm = None;
                        this.confirm_dialog_open = false;
                        cx.notify();
                    });
                    true
                })
        });
    }
}

/// Amber accents distinguish upgrade outlines and their subtle interaction states.
pub(super) fn update_button_style(cx: &App) -> ButtonCustomVariant {
    let (background, foreground, hover, active) = if cx.theme().is_dark() {
        (0x524234, 0xffd29d, 0x614c39, 0x705740)
    } else {
        (0xfff0d9, 0x9c570d, 0xffe5bd, 0xffd99f)
    };
    ButtonCustomVariant::new(cx)
        .color(rgb(background).into())
        .foreground(rgb(foreground).into())
        .hover(rgb(hover).into())
        .active(rgb(active).into())
}

/// Rose accents keep removal outlines readable in both editor themes.
pub(super) fn uninstall_button_style(cx: &App) -> ButtonCustomVariant {
    let (background, foreground, hover, active) = if cx.theme().is_dark() {
        (0x553b43, 0xffbdc9, 0x65424b, 0x764953)
    } else {
        (0xffe6ea, 0xb93851, 0xffd3dc, 0xffc1ce)
    };
    ButtonCustomVariant::new(cx)
        .color(rgb(background).into())
        .foreground(rgb(foreground).into())
        .hover(rgb(hover).into())
        .active(rgb(active).into())
}

/// Compare versions for update availability and explicitly chosen local-package confirmations.
pub(super) fn package_action(available: &str, installed: Option<&str>) -> &'static str {
    let Some(current) = installed else {
        return "安装";
    };
    match (
        semver::Version::parse(available),
        semver::Version::parse(current),
    ) {
        (Ok(available), Ok(installed)) if available > installed => "更新",
        (Ok(available), Ok(installed)) if available < installed => "降级安装",
        _ => "已安装",
    }
}

impl Render for ExtensionPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.manager_open || self.surface_id.is_none() {
            return self.manager(window, cx);
        }
        // Every retained callback belongs to this publication, not a future replacement.
        let epoch = self.instance_epoch;
        if self._focus_events.is_empty() {
            self._focus_events
                .push(cx.on_focus(&self.focus, window, move |this, _, _| {
                    if this.instance_epoch != epoch {
                        return;
                    }
                    this.send(PluginEvent::Focus(true))
                }));
            self._focus_events
                .push(cx.on_blur(&self.focus, window, move |this, _, cx| {
                    if this.instance_epoch != epoch {
                        return;
                    }
                    this.send(PluginEvent::Focus(false));
                    cx.notify();
                }));
        }
        self.native_environment(cx);
        if let Some(error) = &self.preview_error {
            // Host-side admission failures are readable UI, with no stale guest input target underneath.
            return div()
                .size_full()
                .p_4()
                .text_color(cx.theme().muted_foreground)
                .debug_selector(|| "plugin-preview-error".into())
                .child(error.clone())
                .into_any_element();
        }
        if let Some(mut document) = self
            .renderable_document()
            .map(|document| (*document).clone())
        {
            if self.current_document().is_none() {
                // Old modals/menus cannot capture current input while a source refresh is pending.
                document.dialog = None;
                document.menu = None;
            }
            let view = self.native_document(document, window, cx);
            return div()
                .size_full()
                .relative()
                .key_context("PluginSurface")
                // The viewport has a definite extent; its rich content must not participate in the
                // parent's intrinsic sizing. Keep normal native paint/event registration each frame.
                .child(div().absolute().inset_0().size_full().child(view))
                .child(self.view_demand_probe(cx))
                .children(self.command_popup(window, cx))
                .into_any_element();
        }
        self.retire_unowned_native_view(cx);
        // An unpublished or retired document has no focusable guest controls.
        div()
            .size_full()
            .relative()
            .child(self.view_demand_probe(cx))
            .children(self.command_popup(window, cx))
            .into_any_element()
    }
}
