//! Source breakpoints are host configuration data shared by every declared debugger provider.
use super::*;

impl EditorApp {
    /// F9 reads the canonical native cursor, persists a one-based source position and updates only
    /// its owning live debug session. It never changes document text or starts a language tool.
    pub(crate) fn toggle_source_breakpoint(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self
            .active_path
            .clone()
            .filter(|path| path.is_file() && path.starts_with(self.workspace.root()))
        else {
            self.status = t!("run.breakpoint_source_required").to_string();
            cx.notify();
            return;
        };
        let config = self
            .run_controls
            .debug_session()
            .filter(|(_, session)| session.state().is_connected())
            .map(|(id, _)| id.to_owned())
            .or_else(|| self.run_controls.selected().map(|config| config.id.clone()));
        let Some(mut configuration) =
            config.and_then(|id| self.run_controls.configuration(&id).cloned())
        else {
            self.status = t!("run.breakpoint_source_required").to_string();
            cx.notify();
            return;
        };
        let source = path.display().to_string();
        let line = self
            .editor
            .read(cx)
            .cursor_position()
            .line
            .saturating_add(1);
        let removed = configuration.breakpoints.remove(&source, line);
        if !removed && let Err(error) = configuration.breakpoints.insert(&source, line) {
            self.status = error.to_string();
            cx.notify();
            return;
        }
        let workspace = self.workspace_key();
        match self.run_controls.upsert(configuration, &workspace) {
            Ok(()) => {
                self.status = if removed {
                    t!(
                        "run.breakpoint_removed",
                        source = source,
                        line = line.to_string()
                    )
                    .to_string()
                } else {
                    t!(
                        "run.breakpoint_added",
                        source = source,
                        line = line.to_string()
                    )
                    .to_string()
                };
                self.send_debug_breakpoints(cx);
            }
            Err(error) => self.status = error,
        }
        cx.notify();
    }
}
