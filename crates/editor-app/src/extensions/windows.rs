//! Plugin work windows reuse native panel publication and the project's existing document sessions.
use super::*;

/// A render-only binding borrows one native entity after lifecycle, ownership and version checks.
#[derive(Clone)]
pub(super) struct SourceMount {
    pub(super) document: protocol::api::DocumentVersion,
    pub(super) editor: Entity<EditorState>,
    pub(super) virtual_resource: Option<Arc<plugin_runtime::VirtualDocumentResource>>,
}

/// Root chrome is local; the reused panel retains all ordinary native control behavior.
struct WorkWindow {
    panel: Entity<ExtensionPanel>,
    title: String,
    parent: WeakEntity<EditorApp>,
    feedback: String,
    overwrite: Option<protocol::api::DocumentVersion>,
}
impl Render for WorkWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let panel_focus = self.panel.read(cx).focus.clone();
        div()
            .track_focus(&panel_focus)
            .key_context("EditorShell")
            .on_action(cx.listener(|this, _: &SaveDocument, window, cx| {
                let Some(source) = this.panel.read(cx).window_source_id() else {
                    return;
                };
                if this
                    .overwrite
                    .as_ref()
                    .is_some_and(|document| document.id == source)
                {
                    return;
                }
                let _ = this.parent.update(cx, |app, cx| {
                    if let Some(index) = app.tabs.iter().position(|tab| {
                        tab.text.as_ref().is_some_and(|text| {
                            format!("{:?}", text.editor.entity_id()) == source
                                && text
                                    .editor
                                    .read(cx)
                                    .focus_handle(cx)
                                    .contains_focused(window, cx)
                        })
                    }) {
                        app.save_text_tab(index, cx);
                        this.feedback = app.status.clone();
                        if app.tabs[index]
                            .text
                            .as_ref()
                            .is_some_and(|text| text.overwrite_confirmed)
                        {
                            this.overwrite = app.plugin_document_version(index).ok();
                            app.tabs[index].text.as_mut().unwrap().overwrite_confirmed = false;
                            this.feedback = t!("editor.window_save_conflict").to_string();
                        }
                    }
                });
                cx.notify();
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .child(
                div()
                    .h(px(PANEL_HEADER_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .child(
                        // Native hit testing chooses the first matching caption region. Keep
                        // dragging beside the buttons so it cannot mask their OS actions.
                        div()
                            .flex_1()
                            .h_full()
                            .pl_3()
                            .flex()
                            .items_center()
                            .window_control_area(gpui_kit::WindowControlArea::Drag)
                            .child(self.title.clone()),
                    )
                    .child(editor_ui::window::native_window_controls(window, cx)),
            )
            .child(div().flex_1().min_h_0().child(self.panel.clone()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .min_h(px(26.))
                    .child(div().flex_1().child(self.feedback.clone()))
                    .when_some(self.overwrite.clone(), |footer, document| {
                        footer.child(
                            Button::new("window-confirm-overwrite")
                                .debug_selector(|| "window-confirm-overwrite".into())
                                .label(t!("editor.window_confirm_overwrite").to_string())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let document = document.clone();
                                    this.overwrite = None;
                                    let _ = this.parent.update(cx, |app, cx| {
                                        let Some(index) = (0..app.tabs.len()).find(|index| {
                                            app.plugin_document_version(*index)
                                                .is_ok_and(|version| version == document)
                                        }) else {
                                            this.feedback =
                                                t!("editor.window_save_changed").to_string();
                                            return;
                                        };
                                        app.tabs[index]
                                            .text
                                            .as_mut()
                                            .unwrap()
                                            .overwrite_confirmed = true;
                                        app.save_text_tab(index, cx);
                                        this.feedback = app.status.clone();
                                    });
                                    cx.notify();
                                })),
                        )
                    }),
            )
    }
}

impl ExtensionPanel {
    /// Only a visible current work-window publication may borrow an editor's sole native mount.
    pub(crate) fn window_source_id(&self) -> Option<String> {
        if !self.work_window || !self.visible.get() {
            return None;
        }
        self.current_document()?
            .active_native_editor()
            .map(|source| source.id.clone())
    }
}

impl EditorApp {
    /// Visibility is an explicit plugin/host action. Native close hides the surface without
    /// cancelling external operations or destroying a user's document/Undo history.
    pub(super) fn sync_plugin_windows(&mut self, cx: &mut Context<Self>) {
        // Shutdown retains saved panel visibility after draining native owners. Retirement
        // frames still render the shell, but must never recreate those independent windows.
        if self.shutting_down {
            return;
        }
        let panels: Vec<_> = self
            .plugin_panels
            .iter()
            .filter(|(_, panel)| panel.read(cx).work_window)
            .map(|(key, panel)| (key.clone(), panel.clone()))
            .collect();
        for (key, panel) in panels {
            let visible = panel.read(cx).visible.get();
            if !visible {
                if let Some(handle) = self.plugin_windows.remove(&key) {
                    panel.update(cx, |panel, cx| panel.retire_native_inputs(cx));
                    remove_work_window_after_release(handle, cx);
                }
                continue;
            }
            if self.plugin_windows.contains_key(&key) {
                continue;
            }
            let title = panel.read(cx).panel_title.clone();
            let visibility = panel.read(cx).visible.clone();
            let owner = cx.entity().downgrade();
            let closed_key = key.clone();
            let mut options = TitleBar::window_options();
            options.window_background = crate::theme::window_background(cx);
            options.window_bounds = Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1280.), px(800.)),
                cx,
            )));
            options.window_min_size = Some(size(px(900.), px(520.)));
            match cx.open_window(options, move |window, cx| {
                window.set_window_title(&title);
                // The same handle is mounted by WorkWindow's root. Initial focus belongs only
                // to this native window; later scene publications preserve its focused child.
                let panel_focus = panel.read(cx).focus.clone();
                panel_focus.focus(window, cx);
                // The root owns the live panel. Win32 may retain its close callback until
                // asynchronous HWND destruction, so that callback cannot prolong panel ownership.
                let close_panel = panel.downgrade();
                window.on_window_should_close(cx, move |window, cx| {
                    let _ = close_panel.update(cx, |panel, cx| panel.retire_native_inputs(cx));
                    if let Some(router) = editor_ui::aligned_pointer::for_window(window, cx)
                        && !router.flush(window, cx)
                    {
                        let owner = owner.clone();
                        let key = closed_key.clone();
                        let visibility = visibility.clone();
                        router.after_pending_release(window, cx, move |window, cx| {
                            record_work_window_close(&visibility, &owner, &key, cx);
                            window.remove_window();
                        });
                        // An input callback may still own the old listener table. Preserve that
                        // root until deferred MouseUp finishes instead of closing ahead of it.
                        return false;
                    }
                    record_work_window_close(&visibility, &owner, &closed_key, cx);
                    true
                });
                let parent = panel.read(cx).parent.clone();
                let content = cx.new(|_| WorkWindow {
                    panel,
                    title,
                    parent,
                    feedback: String::new(),
                    overwrite: None,
                });
                cx.new(|cx| Root::new(content, window, cx))
            }) {
                Ok(handle) => {
                    self.plugin_windows.insert(key, handle);
                }
                Err(error) => {
                    self.report_host_message(
                        app::messages::MessageLevel::Error,
                        error.to_string(),
                        cx,
                    );
                }
            }
        }
        // Stable ownership prevents two native windows from overwriting one EditorState layout/IME
        // geometry. A contender can retain its view, but it never mounts the shared editor twice.
        let mut candidates = self
            .plugin_panels
            .iter()
            .filter_map(|(key, panel)| {
                if !self.plugin_windows.contains_key(key)
                    || panel.read(cx).window_source_id().is_none()
                {
                    return None;
                }
                let target = panel
                    .read(cx)
                    .current_document()?
                    .active_native_editor()?
                    .clone();
                let index = self.tabs.iter().position(|tab| {
                    tab.text
                        .as_ref()
                        .is_some_and(|text| format!("{:?}", text.editor.entity_id()) == target.id)
                })?;
                let current = self.plugin_document_version(index).ok()?;
                if current.path != target.path || current.revision < target.revision {
                    return None;
                }
                let virtual_resource = self.tabs[index]
                    .virtual_document
                    .as_ref()
                    .map(|tab| tab.resource.clone());
                if virtual_resource
                    .as_ref()
                    .is_some_and(|resource| !resource.is_live())
                {
                    return None;
                }
                Some((
                    key.clone(),
                    SourceMount {
                        document: current,
                        editor: self.tabs[index].text.as_ref()?.editor.clone(),
                        virtual_resource,
                    },
                ))
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|(a, _), (b, _)| a.cmp(b));
        self.plugin_window_mounts.retain(|source, key| {
            candidates
                .iter()
                .any(|(candidate, mount)| candidate == key && &mount.document.id == source)
        });
        for (key, mount) in &candidates {
            self.plugin_window_mounts
                .entry(mount.document.id.clone())
                .or_insert(key.clone());
        }
        for (key, panel) in &self.plugin_panels {
            if !panel.read(cx).work_window {
                continue;
            }
            let binding = candidates
                .iter()
                .find(|(candidate, mount)| {
                    candidate == key
                        && self.plugin_window_mounts.get(&mount.document.id) == Some(key)
                })
                .map(|(_, mount)| mount.clone());
            panel.update(cx, |panel, cx| {
                let changed = panel
                    .work_window_source
                    .as_ref()
                    .map(|mount| &mount.document)
                    != binding.as_ref().map(|mount| &mount.document);
                panel.work_window_source = binding;
                if changed {
                    cx.notify();
                }
            });
        }
    }
}

/// Remove an already retired work window only after its old frame releases native gestures.
/// The erased handle avoids borrowing Root while the router reads its presentation plugin.
pub(super) fn remove_work_window_after_release(handle: WindowHandle<Root>, cx: &mut App) {
    let handle: gpui_kit::AnyWindowHandle = handle.into();
    cx.defer(move |cx| {
        let _ = handle.update(cx, |_, window, cx| {
            if let Some(router) = editor_ui::aligned_pointer::for_window(window, cx) {
                router.after_pending_release(window, cx, |window, _| window.remove_window());
            } else {
                window.remove_window();
            }
        });
    });
}

/// Persist a completed close after pointer release; native document/session ownership is unchanged.
fn record_work_window_close(
    visibility: &std::cell::Cell<bool>,
    owner: &WeakEntity<EditorApp>,
    key: &str,
    cx: &mut App,
) {
    visibility.set(false);
    let _ = owner.update(cx, |app, cx| {
        app.plugin_windows.remove(key);
        app.session_state
            .plugin_panel_visibility
            .insert(key.to_owned(), false);
        app.persist_session();
        cx.notify();
    });
}
