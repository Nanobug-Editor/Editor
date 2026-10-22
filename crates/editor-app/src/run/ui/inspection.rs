//! Native actions request only missing bounded descriptions from the current selected inspection.
use super::*;

impl EditorApp {
    /// Each staged request captures its method's thread/frame/reference; delayed answers keep that key.
    pub(super) fn fetch_tree_inspection(&mut self, cx: &mut Context<Self>) {
        if self.run_controls.debug_action_pending() {
            return;
        }
        if let Some((method, name, args, frame)) = self.run_controls.next_tree_request()
            && let Some(request) = self.run_controls.begin_debug_request(method, frame)
        {
            self.stage_debug_requests(cx, vec![(request, name, args)]);
        }
    }
    /// Thread selection replaces the projected stack, then asynchronously fills the new thread.
    pub(super) fn select_inspection_thread(&mut self, thread: u32, cx: &mut Context<Self>) {
        if let Err(error) = self.run_controls.select_debug_thread(thread) {
            self.status = super::super::debug_presentation::inspection_error(&error);
        } else {
            self.fetch_tree_inspection(cx);
        }
        cx.notify();
    }
    /// A toggle never traverses children automatically; More requests precisely the next page.
    pub(super) fn act_inspection_variable(
        &mut self,
        action: crate::run::inspection::VariableAction,
        cx: &mut Context<Self>,
    ) {
        match self.run_controls.act_debug_variable(action) {
            Ok(Some((method, args, frame))) => {
                if let Some(request) = self.run_controls.begin_debug_request(method, Some(frame)) {
                    self.stage_debug_requests(cx, vec![(request, "variable_children", args)]);
                }
            }
            Ok(None) => self.fetch_tree_inspection(cx),
            Err(error) => self.status = super::super::debug_presentation::inspection_error(&error),
        }
        cx.notify();
    }
}
