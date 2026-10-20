//! Context refreshes use native session/revision checks and the runtime's non-forgeable selection.
use super::*;
impl EditorApp {
    pub(super) fn perform_language_context_refresh(
        &mut self,
        plugin: &str,
        request: EditorRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Op::RefreshLanguageContext { provider, document } = request.operation() else {
            unreachable!()
        };
        let result = self
            .refresh_registered_language_context(
                &format!("{plugin}/{provider}"),
                document,
                &request,
                window,
                cx,
            )
            .map(|()| Value::Unit);
        request.finish(result);
    }
}
