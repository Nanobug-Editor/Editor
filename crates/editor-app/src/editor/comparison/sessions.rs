//! Retained comparison tabs share ordinary document sessions and release only derived presentation.
use super::*;

/// A view owns identities, authorization and native display choices; EditorState owns all text.
#[derive(Clone)]
pub(crate) struct ComparisonView {
    pub(crate) id: String,
    owner: String,
    pub(crate) title: String,
    pub(super) authority: plugin_runtime::EditorAuthority,
    left: api::DocumentVersion,
    right: api::DocumentVersion,
    /// The optional third readonly source participates in the same retained-view lifetime.
    third: Option<api::DocumentVersion>,
    /// Frozen provider actions retain their exact issuer, independently of later panel selections.
    navigation: Option<(String, api::ComparisonNavigation)>,
    /// Optional generic unchanged-region controls supplied by this view's provider.
    pub(super) folding: Option<folding::FoldControls>,
    pub(crate) preview: bool,
    pub(crate) unified: bool,
    pub(crate) hunk: usize,
}
impl ComparisonView {
    /// Enumerate borrowed source identities so revocation and reclamation cover every pane.
    fn sources(&self) -> impl Iterator<Item = &api::DocumentVersion> {
        [&self.left, &self.right]
            .into_iter()
            .chain(self.third.as_ref())
    }
}
impl EditorApp {
    /// Source reclamation follows all retained panes, including hidden/inactive comparisons.
    pub(crate) fn comparison_holds_source(&self, id: &str) -> bool {
        self.comparison_views
            .iter()
            .any(|view| view.authority.is_live() && view.sources().any(|source| source.id == id))
            || self.document_comparison.as_ref().is_some_and(|view| {
                view.authority.is_live()
                    && (view.left.id == id
                        || view.right.id == id
                        || view
                            .third
                            .as_ref()
                            .is_some_and(|third| third.version.id == id))
            })
            || self.plugin_window_mounts.contains_key(id)
    }
    /// The unified view replaces both source panes, so navigation must focus its readonly editor.
    pub(crate) fn unified_comparison_focus_handle(&self, cx: &App) -> Option<FocusHandle> {
        let id = self.active_comparison.as_ref()?;
        let comparison = self.document_comparison.as_ref()?;
        if comparison.third.is_some()
            || !self
                .comparison_views
                .iter()
                .any(|view| &view.id == id && view.unified)
        {
            return None;
        }
        comparison
            .unified_editor
            .as_ref()
            .map(|editor| editor.focus_handle(cx))
    }

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
        third: Option<&api::DocumentVersion>,
        navigation: Option<&api::ComparisonNavigation>,
        folding: Option<&api::ComparisonFolding>,
        preview: bool,
        request: &plugin_runtime::EditorRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<api::EditorValue, api::Failure> {
        let owner = format!("{}/{}", request.handle().instance, request.handle().scope);
        let id = format!("{owner}/{key}");
        let navigation =
            navigation.map(|navigation| (request.handle().instance.clone(), navigation.clone()));
        let folding = folding
            .map(|descriptor| {
                let admitted = folding::FoldControls::admit(descriptor, request)?;
                Ok(folding::retain_choices(
                    self.comparison_views
                        .iter()
                        .find(|view| view.id == id)
                        .and_then(|view| view.folding.as_ref()),
                    admitted,
                ))
            })
            .transpose()?;
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
            third,
            request.authority().clone(),
            Some(request),
            window,
            cx,
        )?;
        let retired = {
            self.comparison_views
                .iter()
                .filter(|view| view.id == id || (preview && view.owner == owner && view.preview))
                .flat_map(|view| view.sources().cloned())
                .collect::<Vec<_>>()
        };
        if preview {
            self.comparison_views
                .retain(|view| view.id == id || view.owner != owner || !view.preview);
        }
        if let Some(view) = self.comparison_views.iter_mut().find(|view| view.id == id) {
            view.left = left.clone();
            view.right = right.clone();
            view.third = third.cloned();
            view.navigation = navigation;
            view.folding = folding;
            // An explicit changed-file open begins at its first difference, even for a reused key.
            view.hunk = 0;
            if third.is_some() {
                view.unified = false;
            }
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
                third: third.cloned(),
                navigation,
                folding,
                preview,
                unified: false,
                hunk: 0,
            });
        }
        self.active_comparison = Some(id.clone());
        self.document_comparison.as_mut().unwrap().persistent = Some(id);
        self.mount_comparison_folding(cx);
        self.reveal_comparison_hunk(0, cx);
        if self
            .comparison_views
            .iter()
            .any(|view| self.active_comparison.as_ref() == Some(&view.id) && view.unified)
        {
            self.prepare_unified_comparison(window, cx);
        }
        self.release_comparison_sources(retired, window, cx);
        let count = self.tabs.iter().filter(|tab| tab.is_visible()).count();
        // Reused previews can sit before the last tab; reveal the active view's measured bounds.
        if let Some(index) = self
            .comparison_views
            .iter()
            .position(|view| self.active_comparison.as_ref() == Some(&view.id))
        {
            self.tabs_scroll.scroll_to_item(count + index);
        }
        cx.notify();
        Ok(result)
    }

    /// Borrow only this tab's frozen neighbors; a missing sequence or boundary disables its arrow.
    pub(super) fn comparison_file_navigation(&self, next: bool) -> Option<&api::ComparisonCommand> {
        let view = self
            .comparison_views
            .iter()
            .find(|view| Some(&view.id) == self.active_comparison.as_ref())?;
        if !view.authority.is_live() {
            return None;
        }
        let (_, navigation) = view.navigation.as_ref()?;
        if next {
            navigation.next.as_ref()
        } else {
            navigation.previous.as_ref()
        }
    }

    /// Queue a provider-owned adjacent item using its original source/lifetime and frozen arguments.
    pub(super) fn navigate_comparison_file(&mut self, next: bool, cx: &mut Context<Self>) {
        let Some(action) = self.comparison_file_navigation(next).cloned() else {
            return;
        };
        let Some(view) = self
            .comparison_views
            .iter()
            .find(|view| Some(&view.id) == self.active_comparison.as_ref())
            .cloned()
        else {
            return;
        };
        let result = (|| {
            let document = self
                .current_comparison_source(&view.right)
                .ok_or_else(|| t!("plugins.document_target_changed").to_string())?;
            let guard = self
                .capture_document_intent_guard(document)
                .map_err(|error| error.to_string())?
                .with_authority(view.authority.clone());
            self.extensions.read(cx).enqueue_owned_command(
                &view.navigation.as_ref().unwrap().0,
                &action.command,
                action.arguments,
                guard,
            )
        })();
        if let Err(message) = result {
            self.report_host_message(app::messages::MessageLevel::Warning, message, cx);
        }
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
            let third = view
                .third
                .as_ref()
                .map(|source| {
                    self.current_comparison_source(source).ok_or_else(|| {
                        api::Failure::new(
                            api::ErrorCode::StaleRevision,
                            "Third comparison source closed",
                        )
                    })
                })
                .transpose()?;
            self.install_document_comparison(
                &left,
                &right,
                third.as_ref(),
                view.authority.clone(),
                None,
                window,
                cx,
            )?;
            self.active_comparison = Some(id.into());
            self.document_comparison.as_mut().unwrap().persistent = Some(id.into());
            self.mount_comparison_folding(cx);
            if view.unified {
                self.prepare_unified_comparison(window, cx);
            }
            if let Some(view) = self.comparison_views.iter_mut().find(|view| view.id == id) {
                view.left = left;
                view.right = right;
                view.third = third;
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
            .flat_map(|view| view.sources().cloned())
            .collect();
        self.comparison_views.retain(|view| view.id != id);
        if self.active_comparison.as_deref() == Some(id) {
            self.active_comparison = None;
            self.close_document_comparison(cx);
            if let Some(index) = self.tabs.iter().rposition(|tab| tab.is_visible()) {
                self.activate_tab(index, window, cx);
            } else {
                self.active_path = None;
            }
        }
        self.release_comparison_sources(retired, window, cx);
        cx.notify();
    }
    /// Only snapshots belonging to a retired view are eligible. A just-created hidden source must
    /// survive the interval before its comparison is opened. Prepared local sources follow their
    /// bounded lease; user-visible local documents always survive.
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
                .any(|view| view.sources().any(|held| held.id == source.id))
                || self.document_comparison.as_ref().is_some_and(|view| {
                    view.left.id == source.id
                        || view.right.id == source.id
                        || view
                            .third
                            .as_ref()
                            .is_some_and(|third| third.version.id == source.id)
                })
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
        self.sync_prepared_document_sources(window, cx);
    }
    /// Retiring a caller or either source revokes the view before any next native mount.
    pub(super) fn sync_comparison_views(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let keep = self
            .comparison_views
            .iter()
            .filter(|view| {
                view.authority.is_live()
                    && view
                        .sources()
                        .all(|source| self.current_comparison_source(source).is_some())
            })
            .map(|view| view.id.clone())
            .collect::<Vec<_>>();
        let retired = self
            .comparison_views
            .iter()
            .filter(|view| !keep.contains(&view.id))
            .flat_map(|view| view.sources().cloned())
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
        let third = comparison
            .third
            .as_ref()
            .and_then(|third| self.current_comparison_source(&third.version));
        if comparison.left == left
            && comparison.right == right
            && comparison.third.as_ref().map(|third| &third.version) == third.as_ref()
        {
            return;
        }
        let third_source = third
            .as_ref()
            .map(|third| self.prepare_third_comparison(third, &left, &right, cx))
            .transpose();
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
        let (Ok(hunks), Ok(third_source)) = (hunks, third_source) else {
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
        if let (Some(current), Some(source)) = (&mut comparison.third, third_source) {
            current.version = source.version;
            current.hunks = source.hunks;
            current.refresh_marks(&comparison.right_editor, comparison.colors, cx);
        }
        super::presentation::refresh_inline_marks(comparison, cx);
        comparison.left_marks.set(
            comparison_marks(&comparison.hunks, comparison.colors, true),
            cx,
        );
        comparison.right_marks.set(
            comparison_marks(&comparison.hunks, comparison.colors, false),
            cx,
        );
        *comparison.navigation.borrow_mut() = [None, None, None];
        configure_alignment(comparison, window, cx);
        if let Some(view) = self.comparison_views.iter_mut().find(|view| view.id == id) {
            view.left = left;
            view.right = right;
            view.third = third;
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
