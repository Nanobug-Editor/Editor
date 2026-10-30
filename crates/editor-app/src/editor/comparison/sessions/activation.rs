//! Reactivate retained comparison identities and their viewports through the ordinary session admission.
use super::*;

impl EditorApp {
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
            if self.active_comparison.as_deref() == Some(id)
                && self.document_comparison_is_current()
            {
                if pin
                    && let Some(view) = self.comparison_views.iter_mut().find(|view| view.id == id)
                {
                    view.preview = false;
                }
                return Ok(());
            }
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
            if let Some(viewport) = &view.viewport {
                self.restore_comparison_viewport(viewport, cx);
            }
            if let Some(view) = self.comparison_views.iter_mut().find(|view| view.id == id) {
                view.boundary = None;
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
}
