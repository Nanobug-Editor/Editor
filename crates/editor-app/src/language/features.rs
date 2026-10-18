//! One native presentation seam consumes bounded direct and standard LSP language results.
use gpui_kit::{
    AppContext as _, Context, EventEmitter, FocusHandle, IntoElement, KeyDownEvent, Render,
    ScrollHandle, Window, div,
};
use plugin_runtime::{
    RequestCancellation,
    plugin_protocol::{api::DocumentVersion, diagnostics::Fix, direct},
};

mod presentation;

/// The application retains source/provider authority and handles these selected plain proposals.
#[derive(Clone, Debug)]
pub(crate) enum LanguageFeatureEvent {
    Navigate(direct::Location),
    Fix(Fix),
    Dismissed,
}

/// Presentation owns only source identity and bounded results, never text or a diagnostic model.
pub(crate) struct LanguageFeatures {
    owner: String,
    feature: direct::Feature,
    document: DocumentVersion,
    cancellation: RequestCancellation,
    result: Option<anyhow::Result<direct::Result>>,
    closed: bool,
    focus: FocusHandle,
    previous: Option<FocusHandle>,
    close_focus: FocusHandle,
    choose_focus: FocusHandle,
    rows: Vec<FocusHandle>,
    selected: usize,
    scroll: ScrollHandle,
    /// A first reply focuses its selected row only while this modal still owns native focus.
    focus_rows: bool,
}

impl EventEmitter<LanguageFeatureEvent> for LanguageFeatures {}

impl LanguageFeatures {
    /// Capture the request identity and shared cancellation lifetime before asynchronous dispatch.
    pub(crate) fn new(
        owner: String,
        feature: direct::Feature,
        document: DocumentVersion,
        cancellation: RequestCancellation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let previous = window.focused(cx);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            owner,
            feature,
            document,
            cancellation,
            result: None,
            closed: false,
            focus,
            previous,
            close_focus: cx.focus_handle(),
            choose_focus: cx.focus_handle(),
            rows: Vec::new(),
            selected: 0,
            scroll: ScrollHandle::new(),
            focus_rows: false,
        }
    }

    /// Receive the already version-checked result from the application's selected provider.
    pub(crate) fn complete(
        &mut self,
        result: anyhow::Result<direct::Result>,
        cx: &mut Context<Self>,
    ) {
        if self.closed || self.cancellation.is_cancelled() {
            return;
        }
        self.result = Some(result.and_then(|result| {
            if result.feature() != self.feature || !result.valid() {
                return Err(plugin_runtime::plugin_protocol::api::Failure::new(
                    plugin_runtime::plugin_protocol::api::ErrorCode::InvalidRequest,
                    "Language result has an invalid role or shape",
                )
                .into());
            }
            Ok(result)
        }));
        self.selected = 0;
        self.rows = (0..self.proposal_labels().len())
            .map(|_| cx.focus_handle())
            .collect();
        self.focus_rows = !self.rows.is_empty();
        cx.notify();
    }

    /// Expose identity so the application can revalidate the same native source/provider selection.
    pub(crate) fn document(&self) -> &DocumentVersion {
        &self.document
    }

    /// Source attribution remains visible while authority stays in the application context.
    pub(crate) fn owner(&self) -> &str {
        &self.owner
    }

    /// The application supplies current authority before rendering or delivering a user selection.
    pub(crate) fn prune(
        &mut self,
        current: Option<&DocumentVersion>,
        owner_active: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !owner_active || current != Some(&self.document) || self.cancellation.is_cancelled() {
            self.close(window, cx);
        }
        self.closed
    }

    /// Selectable results share one native list; readonly signatures expose no effect action.
    /// References retain their path and one-based coordinates even with a descriptive label.
    fn proposal_labels(&self) -> Vec<String> {
        let locations = match &self.result {
            Some(Ok(direct::Result::References { locations })) => locations,
            Some(Ok(direct::Result::CodeActions { actions })) => {
                return actions.iter().map(|action| action.title.clone()).collect();
            }
            _ => return Vec::new(),
        };
        locations
            .iter()
            .map(|location| {
                let plugin_runtime::plugin_protocol::api::ResourceIdentity::Local { path } =
                    &location.resource
                else {
                    unreachable!("complete validates every reference resource")
                };
                let target = format!(
                    "{}:{}:{}",
                    editor_core::display_path(std::path::Path::new(path)),
                    u64::from(location.position.line) + 1,
                    u64::from(location.position.character) + 1
                );
                location
                    .label
                    .as_ref()
                    .map_or(target.clone(), |label| format!("{label} — {target}"))
            })
            .collect()
    }

    /// Emit one selected proposal before dismissal; the app rechecks sealed source authority.
    fn choose(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.closed || self.cancellation.is_cancelled() {
            return false;
        }
        let event = match &self.result {
            Some(Ok(direct::Result::References { locations })) => locations
                .get(self.selected)
                .cloned()
                .map(LanguageFeatureEvent::Navigate),
            Some(Ok(direct::Result::CodeActions { actions })) => actions
                .get(self.selected)
                .cloned()
                .map(LanguageFeatureEvent::Fix),
            _ => None,
        };
        let Some(event) = event else {
            return false;
        };
        // Selection only returns the original proposal. The application admits effects through
        // the sealed provider's ordinary plan queue after rechecking source and provider identity.
        cx.emit(event);
        self.close(window, cx);
        true
    }

    /// Base owns each button's activation; this list only selects, focuses and reveals a row.
    fn navigate(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.modifiers != Default::default() || self.rows.is_empty() {
            return;
        }
        let count = self.rows.len();
        self.selected = match event.keystroke.key.as_str() {
            "up" => (self.selected + count - 1) % count,
            "down" => (self.selected + 1) % count,
            "home" => 0,
            "end" => count - 1,
            _ => return,
        };
        // Focus follows selection, so the next Enter activates this row's Base button exactly once.
        self.rows[self.selected].focus(window, cx);
        self.scroll.scroll_to_item(self.selected);
        cx.stop_propagation();
        cx.notify();
    }

    /// Closing an obsolete result must not pull focus back from an unrelated native control.
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.cancellation.cancel();
        let owns_focus = self.focus.is_focused(window)
            || self.focus.contains_focused(window, cx)
            || self.rows.iter().any(|focus| focus.is_focused(window))
            || self.close_focus.is_focused(window)
            || self.choose_focus.is_focused(window);
        if let Some(previous) = self.previous.take().filter(|_| owns_focus) {
            previous.focus(window, cx);
        }
        cx.emit(LanguageFeatureEvent::Dismissed);
        cx.notify();
    }
}

impl Render for LanguageFeatures {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.closed || self.cancellation.is_cancelled() {
            self.close(window, cx);
            return div().into_any_element();
        }
        // The deferred Base modal is absent from the previous dispatch tree on its first frame.
        // Exact handle focus still proves ownership before contains_focused can observe its children.
        if self.focus_rows
            && (self.focus.is_focused(window) || self.focus.contains_focused(window, cx))
        {
            self.rows[self.selected].focus(window, cx);
            self.focus_rows = false;
        }
        self.modal(window, cx)
    }
}

impl Drop for LanguageFeatures {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

#[cfg(test)]
mod tests;
