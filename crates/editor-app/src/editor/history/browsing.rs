//! Metadata queries and independent preview reads cross only the bounded history worker.

use super::model::{Page, Selection, Target};
use super::*;
use editor_history::{LegacyQuery, Query, Scope};
use gpui_base::input::InputState;

impl EditorApp {
    /// Navigation chrome returns focus to the mounted history filter instead of a hidden source.
    pub(crate) fn history_focus_handle(&self, cx: &App) -> Option<FocusHandle> {
        self.history
            .panel
            .as_ref()
            .map(|panel| panel.filter.focus_handle(cx))
    }

    /// File-tree/tab entry follows the file's persistent identity across all recorded path changes.
    pub(crate) fn open_file_history(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let path = path.canonicalize().unwrap_or(path);
        self.observe_history_path(path.clone(), Reason::External, cx);
        self.open_history_category(Category::File(path), window, cx);
    }

    /// Application entry includes deleted files; standalone and legacy sources have explicit categories.
    pub(crate) fn open_workspace_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_history_category(Category::Workspace, window, cx);
    }

    /// Replace the old view after retiring its native comparison and temporary retention lease.
    pub(super) fn open_history_category(
        &mut self,
        category: Category,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_history(window, cx);
        let return_focus = window.focused(cx);
        let return_source = return_focus.as_ref().is_some_and(|focus| {
            self.tabs.iter().any(|file| {
                file.text
                    .as_ref()
                    .is_some_and(|tab| tab.editor.focus_handle(cx) == *focus)
            })
        });
        let filter =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("history.filter").to_string()));
        let subscription = cx.subscribe(&filter, |app, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                if let Some(panel) = &mut app.history.panel {
                    panel.pages = vec![None];
                }
                app.refresh_history(cx);
            }
        });
        self.history.panel = Some(Panel {
            category,
            filter: filter.clone(),
            _filter_subscription: subscription,
            rows: Vec::new(),
            selected: None,
            preview: None,
            selection: None,
            pages: vec![None],
            busy: false,
            error: None,
            notice: None,
            return_focus,
            return_source,
            scroll: ScrollHandle::new(),
            clear: None,
            comparison: None,
        });
        filter.read(cx).focus_handle(cx).focus(window, cx);
        self.refresh_history(cx);
    }

    /// Closing/cancelling invalidates every old reply without waiting for storage or codec work.
    pub(super) fn close_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.history.generation = self.history.generation.wrapping_add(1);
        self.history.picker.take();
        if let Some(mut panel) = self.history.panel.take() {
            if let Some(comparison) = panel.comparison.take() {
                comparison.retire(cx);
            }
            if panel.return_source {
                self.editor.focus_handle(cx).focus(window, cx);
            } else if let Some(focus) = panel.return_focus {
                focus.focus(window, cx);
            }
        }
        cx.notify();
    }

    /// Reset one view's derived resources; callers separately preserve the requested page cursor.
    pub(super) fn clear_history_preview(&mut self, cx: &mut Context<Self>) {
        if let Some(panel) = &mut self.history.panel {
            if let Some(comparison) = panel.comparison.take() {
                comparison.retire(cx);
            }
            panel.preview = None;
            panel.selection = None;
            panel.clear = None;
            panel.notice = None;
        }
    }

    /// Query only metadata; stale filter/page replies cannot replace a newer selection.
    pub(super) fn refresh_history(&mut self, cx: &mut Context<Self>) {
        self.reconnect_history(cx);
        self.clear_history_preview(cx);
        self.history.generation = self.history.generation.wrapping_add(1);
        let generation = self.history.generation;
        let Some(panel) = &mut self.history.panel else {
            return;
        };
        panel.selected = None;
        panel.rows.clear();
        panel.busy = true;
        panel.error = None;
        panel.notice = self
            .history
            .skipped
            .as_ref()
            .filter(|(path, _)| matches!(&panel.category, Category::File(target) if target == path))
            .map(|(path, reason)| skip_message(path, *reason));
        panel.scroll.set_offset(point(px(0.), px(0.)));
        let category = panel.category.clone();
        let filter = panel.filter.read(cx).value().to_string();
        let before = panel.pages.last().cloned().flatten();
        let root = self.workspace.root().to_owned();
        let identity = if let Category::File(path) = &category {
            self.tabs
                .iter()
                .find(|file| file.path() == path)
                .and_then(|file| self.history.identities.get(&file.file_id).copied())
        } else {
            None
        };
        let Some(worker) = self.history.worker.clone() else {
            panel.busy = false;
            panel.error = self.history.last_error.clone();
            cx.notify();
            return;
        };
        let listed = worker.call(0, move |history| {
            if matches!(category, Category::Legacy) {
                let before = match before {
                    Some(Page::Legacy(cursor)) => Some(cursor),
                    _ => None,
                };
                return history
                    .legacy(&LegacyQuery {
                        filter,
                        before,
                        limit: 50,
                    })
                    .map(|rows| rows.into_iter().map(Item::Legacy).collect::<Vec<_>>());
            }
            let mut query = Query {
                path: filter,
                before: match before {
                    Some(Page::Version(cursor)) => Some(cursor),
                    _ => None,
                },
                ..Default::default()
            };
            query.scope = match &category {
                Category::Workspace | Category::Deleted => Scope::Workspace(root.clone()),
                Category::Standalone => Scope::Standalone,
                _ => Scope::All,
            };
            query.deleted_only = matches!(category, Category::Deleted);
            if let Category::File(path) = &category {
                history.import_known(path, path.starts_with(&root).then_some(root.as_path()))?;
                query.file = identity.or(history.file_at(path)?);
                if query.file.is_none() {
                    return Ok(Vec::new());
                }
            }
            history
                .list(&query)
                .map(|rows| rows.into_iter().map(Item::Version).collect())
        });
        cx.spawn(async move |app, cx| {
            let rows = listed.await;
            let _ = app.update(cx, |app, cx| {
                if app.history.generation != generation {
                    return;
                }
                let Some(panel) = &mut app.history.panel else {
                    return;
                };
                panel.busy = false;
                match rows {
                    Ok(rows) => panel.rows = rows,
                    Err(error) => panel.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Page navigation retains only cursor positions, so long timelines do not accumulate decoded data.
    pub(super) fn history_page(&mut self, next: bool, cx: &mut Context<Self>) {
        let Some(panel) = &mut self.history.panel else {
            return;
        };
        if next {
            if panel.rows.len() < 50 {
                return;
            }
            let cursor = panel.rows.last().map(|item| match item {
                Item::Version(row) => Page::Version(row.cursor()),
                Item::Legacy(row) => Page::Legacy(row.cursor()),
            });
            panel.pages.push(cursor);
        } else if panel.pages.len() > 1 {
            panel.pages.pop();
        } else {
            return;
        }
        self.refresh_history(cx);
    }

    /// Read and verify exactly the selected complete object; metadata-only events remain visibly unavailable.
    pub(crate) fn select_history_item(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clear_history_preview(cx);
        self.history.generation = self.history.generation.wrapping_add(1);
        let generation = self.history.generation;
        let Some(panel) = &mut self.history.panel else {
            return;
        };
        let Some(item) = panel.rows.get(index).cloned() else {
            return;
        };
        panel.selected = Some(index);
        panel.busy = true;
        panel.error = None;
        if !item.has_content() {
            panel.busy = false;
            panel.notice = Some(t!("history.no_content").to_string());
            cx.notify();
            return;
        }
        let Some(worker) = self.history.worker.clone() else {
            return;
        };
        let selected = item.clone();
        let previewed = worker.call(32 * 1024 * 1024, move |history| {
            let (bytes, lease, location) = match &selected {
                Item::Version(row) => {
                    let lease = history.pin(row.id)?;
                    (
                        history.read(row.id)?,
                        Some(lease),
                        Some(history.location(row.file)?),
                    )
                }
                Item::Legacy(row) => (history.read_legacy(row)?, None, None),
            };
            let disk = location
                .as_ref()
                .map(|location| platform_windows::history::FileStamp::read(&location.path))
                .transpose()?
                .flatten();
            Ok((bytes, lease, location, disk))
        });
        cx.spawn_in(window, async move |app, cx| {
            let result = previewed.await;
            let _ = app.update_in(cx, |app, window, cx| {
                if app.history.generation != generation {
                    return;
                }
                let result = result.and_then(|(bytes, lease, location, disk)| {
                    let contents =
                        String::from_utf8(bytes).map_err(|_| editor_history::Error::Corrupt)?;
                    if contents.contains('\0') {
                        return Err(editor_history::Error::Corrupt);
                    }
                    let active_document =
                        app.active_tab_index().map(|index| app.tabs[index].file_id);
                    let target = location
                        .as_ref()
                        .and_then(|location| {
                            app.tabs.iter().find(|file| file.path() == location.path)
                        })
                        .and_then(|file| {
                            file.text.as_ref().map(|tab| Target {
                                id: file.file_id,
                                revision: tab.session.revision(),
                                capability: tab.capability_revision,
                                identity: super::super::native_text::NativeTextIdentity::new(
                                    tab.editor.read(cx).text(),
                                ),
                            })
                        });
                    // Unwrapped readonly source avoids eagerly laying out thousands of visual rows
                    // for a large single-line version; native horizontal scrolling retains all bytes.
                    let preview = cx.new(|cx| {
                        EditorState::new(window, cx)
                            .language("text")
                            .line_number(true)
                            .soft_wrap(false)
                    });
                    preview.update(cx, |editor, cx| {
                        editor.set_value(contents, window, cx);
                        editor.set_readonly(true, cx);
                    });
                    let panel = app.history.panel.as_mut().expect("current history view");
                    panel.preview = Some(preview);
                    panel.selection = Some(Selection {
                        item,
                        active_document,
                        _lease: lease,
                        location,
                        disk,
                        target,
                    });
                    Ok(())
                });
                if let Some(panel) = &mut app.history.panel {
                    panel.busy = false;
                    if let Err(error) = result {
                        panel.error = Some(error.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
