//! A Base dock leaf forwards to its editor owner without retaining document or diagnostic state.
use super::*;

pub(crate) struct ProblemsPanel {
    parent: WeakEntity<EditorApp>,
    open: Rc<Cell<bool>>,
    focus: FocusHandle,
    close_focus: FocusHandle,
    /// Publications and tab changes repaint this separate entity through the owning app.
    _owner_observer: Subscription,
}

impl ProblemsPanel {
    /// Share visibility and focus with the controller; Base owns layout, dragging and resizing.
    pub(super) fn new(
        parent: WeakEntity<EditorApp>,
        open: Rc<Cell<bool>>,
        focus: FocusHandle,
        close_focus: FocusHandle,
        cx: &mut Context<Self>,
    ) -> Self {
        let owner = parent.upgrade().expect("Problems belongs to a live editor");
        let observer = cx.observe(&owner, |_, _, cx| cx.notify());
        Self {
            parent,
            open,
            focus,
            close_focus,
            _owner_observer: observer,
        }
    }
}

impl EventEmitter<PanelEvent> for ProblemsPanel {}

impl Focusable for ProblemsPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl dock::BasePanel for ProblemsPanel {
    fn panel_name(&self) -> &'static str {
        "Problems"
    }

    fn visible(&self, _: &App) -> bool {
        self.open.get()
    }

    fn closable(&self, _: &App) -> bool {
        false
    }

    fn zoomable(&self, _: &App) -> bool {
        false
    }
}

impl DockPanel for ProblemsPanel {
    /// Reuse the heading's actual diagnostic count without rendering its interactive controls.
    fn drag_title(&self, cx: &App) -> Option<gpui_kit::SharedString> {
        let count = self
            .parent
            .upgrade()
            .map_or(0, |app| app.read(cx).problem_rows(cx).len());
        Some(t!("problems.title", count = count).to_string().into())
    }

    fn title(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self
            .parent
            .upgrade()
            .map_or(0, |app| app.read(cx).problem_rows(cx).len());
        let parent = self.parent.clone();
        crate::ui::controls::problems::problem_heading(
            count,
            &self.close_focus,
            move |_, window, cx| {
                cx.stop_propagation();
                let _ = parent.update(cx, |app, cx| app.close_problems(window, cx));
            },
        )
    }
}

impl Render for ProblemsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.parent
            .update(cx, |app, cx| app.render_problems(window, cx))
            .unwrap_or_else(|_| div().into_any_element())
    }
}
