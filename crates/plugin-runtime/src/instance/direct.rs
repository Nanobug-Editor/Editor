//! Experimental pure language callbacks cannot publish through another hook's typed channel.
use super::*;
impl Instance {
    /// Native actions use exactly the ordinary owned Workspace request and shared completion gate.
    pub(crate) fn submit_language_fix(
        &mut self,
        document: api::DocumentVersion,
        fix: plugin_protocol::diagnostics::Fix,
    ) -> Result<api::Value, api::Failure> {
        let plan = crate::workspace_resources::language_plan(document, fix.plan)?;
        plan.validate()?;
        self.store.data_mut().editor_request(
            api::EditorOperation::Workspace {
                operation: plugin_protocol::workspace::Operation::ApplyPlan { plan },
            },
            120_000,
        )
    }
    pub(crate) fn direct_snapshot(
        &mut self,
        request: plugin_protocol::direct::Request,
    ) -> anyhow::Result<api::Output> {
        anyhow::ensure!(
            self.store.data().language_pure,
            "Direct results require pure workers"
        );
        self.store.data_mut().language_direct_call = true;
        let result = self.notify(None, api::Notification::LanguageDirect(request));
        self.store.data_mut().language_direct_call = false;
        result
    }
}
