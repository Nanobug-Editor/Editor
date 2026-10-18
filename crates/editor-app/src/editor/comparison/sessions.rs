//! Retained comparison tabs share ordinary document sessions and release only derived presentation.
use super::*;

/// A view owns identities, authorization and native display choices; EditorState owns all text.
#[derive(Clone)]
pub(crate) struct ComparisonView {
    pub(crate) id: String,
    owner: String,
    pub(crate) title: String,
    authority: plugin_runtime::EditorAuthority,
    left: api::DocumentVersion,
    right: api::DocumentVersion,
    pub(crate) preview: bool,
    pub(crate) unified: bool,
    pub(crate) hunk: usize,
}
impl EditorApp {
    /// Resolve the same incarnation at its current revision. No path can substitute a reopened ID.
    fn current_comparison_source(
        &self,
        source: &api::DocumentVersion,
    ) -> Option<api::DocumentVersion> {
        (0..self.tabs.len()).find_map(|index| {
            let current = self.plugin_document_version(index).ok()?;
            (current.id == source.id
                && current.path == source.path
                && self.tabs[index]
                    .virtual_document
                    .as_ref()
                    .is_none_or(|tab| tab.resource.is_live()))
            .then_some(current)
        })
    }
    /// Admit exact versions once, then keep this view distinct from either ordinary file tab.
    pub(crate) fn open_comparison_view(
        &mut self,
        key: &str,
        title: &str,
        left: &api::DocumentVersion,
        right: &api::DocumentVersion,
        preview: bool,
        request: &plugin_runtime::EditorRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<api::EditorValue, api::Failure> {
        let owner = format!("{}/{}", request.handle().instance, request.handle().scope);
        let id = format!("{owner}/{key}");
        self.sync_comparison_views(window, cx);
        if self.comparison_views.len() >= 32
            && !self.comparison_views.iter().any(|view| view.id == id)
        {
            return Err(api::Failure::new(
                api::ErrorCode::LimitExceeded,
                "Comparison tab quota exceeded",
            ));
        }
        let result = self.install_document_comparison(
            left,
            right,
            request.authority().clone(),
            Some(request),
            window,
            cx,
        )?;
        let retired = {
            self.comparison_views
                .iter()
                .filter(|view| view.id == id || (preview && view.owner == owner && view.preview))
                .flat_map(|view| [view.left.clone(), view.right.clone()])
                .collect::<Vec<_>>()
        };
        if preview {
            self.comparison_views
                .retain(|view| view.id == id || view.owner != owner || !view.preview);
        }
        if let Some(view) = self.comparison_views.iter_mut().find(|view| view.id == id) {
            view.left = left.clone();
            view.right = right.clone();
            view.title = title.into();
            if !preview {
                view.preview = false;
            }
        } else {
            self.comparison_views.push(ComparisonView {
                id: id.clone(),
                owner,
                title: title.into(),
                authority: request.authority().clone(),
                left: left.clone(),
                right: right.clone(),
                preview,
                unified: false,
                hunk: 0,
            });
        }
        self.active_comparison = Some(id.clone());
        self.document_comparison.as_mut().unwrap().persistent = Some(id);
        if self
            .comparison_views
            .iter()
            .any(|view| self.active_comparison.as_ref() == Some(&view.id) && view.unified)
        {
            self.prepare_unified_comparison(window, cx);
        }
        self.release_comparison_sources(retired, window, cx);
        let count = self
            .tabs
            .iter()
            .filter(|tab| {
                tab.virtual_document
                    .as_ref()
                    .is_none_or(|source| !source.hidden)
            })
            .count()
            + self.comparison_views.len();
        let viewport = self.tabs_scroll.bounds().size.width.max(px(190.));
        self.tabs_scroll
            .set_offset(point(-(px(190.) * count - viewport).max(px(0.)), px(0.)));
        cx.notify();
        Ok(result)
    }
    /// Reopening a retained tab reacquires current versions of the same native sessions.
    pub(crate) fn activate_comparison_view(
        &mut self,
        id: &str,
        pin: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(view) = self
            .comparison_views
            .iter()
            .find(|view| view.id == id)
            .cloned()
        else {
            return;
        };
        let result = (|| {
            let left = self.current_comparison_source(&view.left).ok_or_else(|| {
                api::Failure::new(api::ErrorCode::StaleRevision, "Comparison source closed")
            })?;
            let right = self.current_comparison_source(&view.right).ok_or_else(|| {
                api::Failure::new(api::ErrorCode::StaleRevision, "Comparison source closed")
            })?;
            self.install_document_comparison(
                &left,
                &right,
                view.authority.clone(),
                None,
                window,
                cx,
            )?;
            self.active_comparison = Some(id.into());
            self.document_comparison.as_mut().unwrap().persistent = Some(id.into());
            if view.unified {
                self.prepare_unified_comparison(window, cx);
            }
            if let Some(view) = self.comparison_views.iter_mut().find(|view| view.id == id) {
                view.left = left;
                view.right = right;
                if pin {
                    view.preview = false;
                }
            }
            Ok::<_, api::Failure>(())
        })();
        if let Err(error) = result {
            self.report_host_message(app::messages::MessageLevel::Warning, error.to_string(), cx);
        }
        cx.notify();
    }
    /// Closing a derived view does not discard a local source or its unsaved Undo state.
    pub(crate) fn close_comparison_view(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let retired = self
            .comparison_views
            .iter()
            .filter(|view| view.id == id)
            .flat_map(|view| [view.left.clone(), view.right.clone()])
            .collect();
        self.comparison_views.retain(|view| view.id != id);
        if self.active_comparison.as_deref() == Some(id) {
            self.active_comparison = None;
            self.close_document_comparison(cx);
            if let Some(index) = self.tabs.iter().rposition(|tab| {
                tab.virtual_document
                    .as_ref()
                    .is_none_or(|source| !source.hidden)
            }) {
                self.activate_tab(index, window, cx);
            } else {
                self.active_path = None;
            }
        }
        self.release_comparison_sources(retired, window, cx);
        cx.notify();
    }
    /// Only snapshots belonging to a retired view are eligible. A just-created hidden source must
    /// survive the interval before its comparison is opened, and local documents always survive.
    fn release_comparison_sources(
        &mut self,
        retired: Vec<api::DocumentVersion>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for source in retired {
            if self
                .comparison_views
                .iter()
                .any(|view| view.left.id == source.id || view.right.id == source.id)
                || self
                    .document_comparison
                    .as_ref()
                    .is_some_and(|view| view.left.id == source.id || view.right.id == source.id)
                || self.plugin_window_mounts.contains_key(&source.id)
            {
                continue;
            }
            if let Some(index) = (0..self.tabs.len()).find(|index| {
                self.tabs[*index]
                    .virtual_document
                    .as_ref()
                    .is_some_and(|tab| tab.hidden)
                    && self
                        .plugin_document_version(*index)
                        .is_ok_and(|version| version.id == source.id)
            }) {
                self.close_tab(self.tabs[index].path().to_path_buf(), window, cx);
            }
        }
    }
    /// Retiring a caller or either source revokes the view before any next native mount.
    pub(super) fn sync_comparison_views(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let keep = self
            .comparison_views
            .iter()
            .filter(|view| {
                view.authority.is_live()
                    && self.current_comparison_source(&view.left).is_some()
                    && self.current_comparison_source(&view.right).is_some()
            })
            .map(|view| view.id.clone())
            .collect::<Vec<_>>();
        let retired = self
            .comparison_views
            .iter()
            .filter(|view| !keep.contains(&view.id))
            .flat_map(|view| [view.left.clone(), view.right.clone()])
            .collect();
        self.comparison_views.retain(|view| keep.contains(&view.id));
        if self
            .active_comparison
            .as_ref()
            .is_some_and(|id| !keep.contains(id))
        {
            self.active_comparison = None;
            self.close_document_comparison(cx);
        }
        self.release_comparison_sources(retired, window, cx);
    }
    /// A local edit advances the same session; only decorations and immutable diff ranges change.
    pub(super) fn refresh_persistent_comparison(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(comparison) = &self.document_comparison else {
            return;
        };
        let Some(id) = comparison.persistent.clone() else {
            return;
        };
        let (Some(left), Some(right)) = (
            self.current_comparison_source(&comparison.left),
            self.current_comparison_source(&comparison.right),
        ) else {
            return;
        };
        if comparison.left == left && comparison.right == right {
            return;
        }
        let oversized = comparison.left_editor.read(cx).text().len() > 1024 * 1024
            || comparison.right_editor.read(cx).text().len() > 1024 * 1024;
        let hunks = if oversized {
            Err(())
        } else {
            diff::compare(
                &comparison.left_editor.read(cx).value().to_string(),
                &comparison.right_editor.read(cx).value().to_string(),
            )
        };
        let Ok(hunks) = hunks else {
            self.active_comparison = None;
            self.close_document_comparison(cx);
            self.report_host_message(
                app::messages::MessageLevel::Warning,
                t!("editor.diff_limit").to_string(),
                cx,
            );
            return;
        };
        let comparison = self.document_comparison.as_mut().unwrap();
        comparison.left = left.clone();
        comparison.right = right.clone();
        comparison.hunks = hunks;
        super::presentation::refresh_inline_marks(comparison, cx);
        comparison.left_marks.set(
            comparison_marks(&comparison.hunks, comparison.colors, true),
            cx,
        );
        comparison.right_marks.set(
            comparison_marks(&comparison.hunks, comparison.colors, false),
            cx,
        );
        *comparison.navigation.borrow_mut() = [None, None];
        configure_alignment(comparison, window, cx);
        if let Some(view) = self.comparison_views.iter_mut().find(|view| view.id == id) {
            view.left = left;
            view.right = right;
        }
        if self
            .document_comparison
            .as_ref()
            .is_some_and(|comparison| comparison.unified_editor.is_some())
        {
            self.prepare_unified_comparison(window, cx);
        }
    }
}
