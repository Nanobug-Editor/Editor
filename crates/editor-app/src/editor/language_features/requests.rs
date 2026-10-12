//! One current native request owns immutable identity, a sealed provider and shared presentation.
use super::*;
use crate::language::{
    features::{LanguageFeatureEvent, LanguageFeatures},
    navigation, providers,
};
use gpui_kit::AnyElement;
use plugin_runtime::{
    DirectProvider, RequestCancellation, plugin_protocol::language::SourceSnapshot,
};
mod demand;
use demand::Choice;

/// No source text is retained here. Replacing or dropping a request cancels its readonly wait.
#[derive(Default)]
pub(crate) struct State {
    sequence: u64,
    active: Option<Active>,
    /// Escape also cancels cold activation and the preceding native-input drain.
    pending: Option<RequestCancellation>,
}

/// Provider identity and generation reject same-ID replacements and A→B→A selections.
struct Active {
    sequence: u64,
    language: String,
    feature: direct::Feature,
    generation: u64,
    target: Target,
    cancellation: RequestCancellation,
    view: Entity<LanguageFeatures>,
    _subscription: Subscription,
}

/// A readonly intention is reserved before draining pending native input. Its source incarnation
/// and exact provider remain immutable while the native revision is allowed to finish changing.
struct Intent {
    sequence: u64,
    editor: Entity<EditorState>,
    document: api::DocumentVersion,
    language: String,
    feature: direct::Feature,
    generation: u64,
    target: Option<Target>,
    choice: Choice,
}

/// Direct and LSP callbacks share one native consumer without lending host file authority.
#[derive(Clone)]
enum Target {
    Direct {
        key: String,
        provider: Arc<DirectProvider>,
    },
    Lsp {
        server: Arc<navigation::LanguageServer>,
        lease: navigation::DocumentLease,
    },
}
impl Target {
    /// Keep the exact original owner for later reference navigation or ordinary plan admission.
    fn origin(&self) -> LanguageFixOrigin {
        match self {
            Self::Direct { provider, .. } => LanguageFixOrigin::Direct(provider.clone()),
            Self::Lsp { server, .. } => server.fix_origin(),
        }
    }

    /// Run blocking bounded transports on a dedicated worker, never on the native window thread.
    fn request(
        self,
        source: SourceSnapshot,
        cursor: usize,
        feature: direct::Feature,
        cancellation: &RequestCancellation,
        workspace: &Path,
    ) -> anyhow::Result<direct::Result> {
        match self {
            Self::Direct { provider, .. } => {
                provider.request(feature, source, cursor, cancellation)
            }
            Self::Lsp { server, lease } => {
                server.feature_for(lease, source, cursor, feature, cancellation, workspace)
            }
        }
    }
}
impl Drop for Active {
    /// Even a closed window stops queued waiters; provider-owned transport remains independent.
    fn drop(&mut self) {
        // The view also cancels on Escape/prune. This metadata path handles app/window teardown.
        self.cancellation.cancel();
    }
}

impl EditorApp {
    /// Native semantic consumers share this monotonically revoked tab/window round for ABA checks.
    pub(crate) fn language_features_sequence(&self) -> u64 {
        self.language_features.sequence
    }

    /// The native Base action and context menu share the selected direct role or primary LSP.
    /// Without that public consumer, propagation preserves any existing Base-only provider.
    pub(crate) fn native_language_feature_available(
        &self,
        feature: direct::Feature,
        cx: &App,
    ) -> bool {
        self.session_state.workspace_trusted
            && self.active_path.as_deref().is_some_and(|path| {
                let language = editor::language_for_path(path);
                self.language_feature_choice(&language, feature, cx)
                    .is_some()
            })
    }

    /// Input's closer Ctrl+. binding emits this Base action before a shell binding can run.
    /// Capture only a supported native document consumer; guest inputs keep their own actions.
    pub(crate) fn native_code_actions(
        &mut self,
        _: &gpui_base::input::ToggleCodeActions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.native_language_feature_available(direct::Feature::CodeActions, cx) {
            cx.propagate();
            return;
        }
        self.request_language_feature(direct::Feature::CodeActions, window, cx);
        cx.stop_propagation();
    }

    /// Retire a request synchronously before tab switches, preventing A→B→A delayed delivery.
    pub(crate) fn cancel_language_features(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A pending bridge wait has no Active view yet. Advancing its reservation still cancels
        // it synchronously, including A→B→A and a newer request on the same editor entity.
        self.language_features.sequence = self.language_features.sequence.wrapping_add(1);
        if let Some(pending) = self.language_features.pending.take() {
            pending.cancel();
        }
        if let Some(active) = self.language_features.active.take() {
            active.view.update(cx, |view, cx| {
                view.prune(None, false, window, cx);
            });
            cx.notify();
        }
    }

    /// Current native session/revision and exact provider are checked for both delivery and selection.
    fn language_feature_is_current(&self, sequence: u64, cx: &App) -> bool {
        let Some(active) = self
            .language_features
            .active
            .as_ref()
            .filter(|active| active.sequence == sequence)
        else {
            return false;
        };
        if !self.session_state.workspace_trusted
            || self.language_features.sequence != sequence
            || providers::feature_generation() != active.generation
            || self
                .active_text_tab_index()
                .and_then(|index| self.plugin_document_version(index).ok())
                .as_ref()
                != Some(active.view.read(cx).document())
            || !active.target.origin().is_live()
        {
            return false;
        }
        self.language_feature_target_is_current(
            &active.language,
            active.feature,
            &active.target,
            cx,
        )
    }

    /// Reuse exact provider ownership checks before a pending wait and after asynchronous delivery.
    fn language_feature_target_is_current(
        &self,
        language: &str,
        feature: direct::Feature,
        target: &Target,
        cx: &App,
    ) -> bool {
        if !target.origin().is_live() {
            return false;
        }
        match target {
            Target::Direct { key, provider } => {
                providers::language_feature_provider(language, feature).as_ref() == Some(key)
                    && self
                        .extensions
                        .read(cx)
                        .direct_language_providers()
                        .get(key)
                        .and_then(|provider| provider.as_ref().ok())
                        .is_some_and(|current| Arc::ptr_eq(current, provider))
            }
            Target::Lsp { server, lease } => {
                !providers::has_language_feature(language, feature)
                    && lease.is_active()
                    && self
                        .language_servers
                        .get(language)
                        .is_some_and(|current| Arc::ptr_eq(current, server))
            }
        }
    }

    /// Pending native input can advance a revision, but cannot change document, provider or intent.
    fn language_intent_is_current(&self, intent: &Intent, cx: &App) -> bool {
        self.session_state.workspace_trusted
            && self.language_features.sequence == intent.sequence
            && self.editor == intent.editor
            && providers::feature_generation() == intent.generation
            && self
                .active_text_tab_index()
                .and_then(|index| self.plugin_document_version(index).ok())
                .is_some_and(|current| {
                    current.id == intent.document.id && current.path == intent.document.path
                })
            && self
                .language_feature_choice(&intent.language, intent.feature, cx)
                .as_ref()
                == Some(&intent.choice)
            && intent.target.as_ref().is_none_or(|target| {
                self.language_feature_target_is_current(
                    &intent.language,
                    intent.feature,
                    target,
                    cx,
                )
            })
    }

    /// Rendering revokes changed source/provider results instead of retaining a stale actionable card.
    pub(crate) fn render_language_features(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let sequence = self.language_features.active.as_ref()?.sequence;
        if !self.language_feature_is_current(sequence, cx) {
            self.cancel_language_features(window, cx);
            return None;
        }
        self.language_features
            .active
            .as_ref()
            .map(|active| active.view.clone().into_any_element())
    }

    /// The sole pending native input bridge drains before snapshotting the source revision and caret.
    pub(crate) fn request_language_feature(
        &mut self,
        feature: direct::Feature,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_language_features(window, cx);
        let Some(index) = self
            .active_text_tab_index()
            .filter(|_| self.session_state.workspace_trusted)
        else {
            return;
        };
        let Ok(document) = self.plugin_document_version(index) else {
            return;
        };
        let language = editor::language_for_path(self.tabs[index].path());
        let Some(choice) = self.language_feature_choice(&language, feature, cx) else {
            self.status = t!("language_features.unavailable").to_string();
            cx.notify();
            return;
        };
        let target = self.language_feature_target(&language, feature, cx);
        let intent = Intent {
            sequence: self.language_features.sequence,
            editor: self.editor.clone(),
            document,
            language,
            feature,
            generation: providers::feature_generation(),
            target,
            choice,
        };
        self.language_features.pending = Some(RequestCancellation::default());
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
                        .update_in(cx, |app, _, cx| app.language_intent_is_current(&intent, cx))
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
                    if app.language_intent_is_current(&intent, cx) {
                        // Continue this reservation; recapturing through the public entry point
                        // would give an obsolete intention a fresh sequence and a new provider.
                        app.start_language_feature(intent, window, cx);
                    }
                });
            })
            .detach();
            return;
        }
        self.start_language_feature(intent, window, cx);
    }

    /// Select the persisted direct role or the existing single primary LSP; ambiguity stays visible.
    fn language_feature_target(
        &self,
        language: &str,
        feature: direct::Feature,
        cx: &App,
    ) -> Option<Target> {
        if providers::has_language_feature(language, feature) {
            let key = providers::language_feature_provider(language, feature)?;
            let provider = self
                .extensions
                .read(cx)
                .direct_language_providers()
                .get(&key)?
                .as_ref()
                .ok()?
                .clone();
            Some(Target::Direct { key, provider })
        } else {
            let server = self.language_servers.get(language)?.clone();
            if !server.supports_language_features() {
                return None;
            }
            let path = self.active_path.as_ref()?;
            // All native consumers use the same Windows drive normalization as the shared
            // document registry; a raw file URL would miss its already-open lifetime.
            let uri = navigation::file_uri(path)?;
            let lease = server.document(&uri)?;
            Some(Target::Lsp { server, lease })
        }
    }

    /// Capture one bounded immutable native snapshot, then deliver only under its original lifetime.
    fn start_language_feature(
        &mut self,
        intent: Intent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.language_intent_is_current(&intent, cx) {
            return;
        }
        let index = self.active_text_tab_index().unwrap();
        let Ok(document) = self.plugin_document_version(index) else {
            return;
        };
        let state = self.editor.read(cx);
        if state.text().len() > 1024 * 1024 {
            self.status = t!("language_features.unavailable").to_string();
            cx.notify();
            return;
        }
        let source = SourceSnapshot {
            document: document.clone(),
            text: state.text().to_string(),
        };
        let cursor = state.cursor();
        let Some(target) = intent.target.clone() else {
            self.demand_language_feature(intent, source, cursor, window, cx);
            return;
        };
        self.dispatch_language_feature(intent, target, source, cursor, window, cx);
    }

    /// After demand, dispatch the frozen source to its exact selected live lease, without resampling.
    fn dispatch_language_feature(
        &mut self,
        intent: Intent,
        target: Target,
        source: SourceSnapshot,
        cursor: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.language_features.pending.take();
        let cancellation = RequestCancellation::default();
        let sequence = intent.sequence;
        let feature = intent.feature;
        let document = source.document.clone();
        let view = cx.new(|cx| {
            LanguageFeatures::new(
                target.origin().owner().into(),
                feature,
                document,
                cancellation.clone(),
                window,
                cx,
            )
        });
        let subscription = cx.subscribe_in(&view, window, move |app, _, event, window, cx| {
            app.select_language_feature(sequence, event, window, cx)
        });
        self.language_features.active = Some(Active {
            sequence,
            language: intent.language,
            feature,
            generation: intent.generation,
            target: target.clone(),
            view,
            _subscription: subscription,
            cancellation: cancellation.clone(),
        });
        let workspace = self.workspace.root().to_path_buf();
        cx.spawn_in(window, async move |this, cx| {
            let worker_cancel = cancellation.clone();
            let result = cx
                .background_executor()
                .scheduler_executor()
                .spawn_dedicated(move |_| async move {
                    target.request(source, cursor, feature, &worker_cancel, &workspace)
                })
                .await;
            let _ = this.update_in(cx, |app, window, cx| {
                if cancellation.is_cancelled() || !app.language_feature_is_current(sequence, cx) {
                    if app
                        .language_features
                        .active
                        .as_ref()
                        .is_some_and(|active| active.sequence == sequence)
                    {
                        app.cancel_language_features(window, cx);
                    }
                    return;
                }
                app.language_features
                    .active
                    .as_ref()
                    .unwrap()
                    .view
                    .update(cx, |view, cx| view.complete(result, cx));
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Selection ends the readonly wait but re-enters the original owner's independent effect queue.
    fn select_language_feature(
        &mut self,
        sequence: u64,
        event: &LanguageFeatureEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, LanguageFeatureEvent::Dismissed) {
            if self
                .language_features
                .active
                .as_ref()
                .is_some_and(|active| active.sequence == sequence)
            {
                self.language_features.active.take();
                cx.notify();
            }
            return;
        }
        if !self.language_feature_is_current(sequence, cx) {
            self.cancel_language_features(window, cx);
            return;
        }
        let active = self.language_features.active.as_ref().unwrap();
        let origin = active.target.origin();
        let document = active.view.read(cx).document().clone();
        let result = match event {
            LanguageFeatureEvent::Navigate(location) => {
                self.navigate_language_location(&origin, &document, location.clone(), window, cx)
            }
            LanguageFeatureEvent::Fix(fix) => {
                self.submit_native_language_fix(origin, document, fix.clone(), cx)
            }
            LanguageFeatureEvent::Dismissed => unreachable!(),
        };
        if let Err(error) = result {
            self.status = error.to_string();
            cx.notify();
        }
    }
}
