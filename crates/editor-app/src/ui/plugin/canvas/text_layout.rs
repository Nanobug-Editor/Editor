//! Native paragraph observations are sent only after the addressed source reached actual paint.
use super::*;

impl CanvasView {
    /// Resolve bounded declared point queries after paint and deduplicate identical native reports.
    pub(super) fn measure_paragraph_layouts(&mut self, window: &Window, cx: &mut Context<Self>) {
        if !self.enabled || self.drawing.text_queries.is_empty() {
            self.text_layout_result = None;
            return;
        }
        let Some(prepared) = &self.scene_graphics else {
            return;
        };
        if !matches!(&self.scene_presented,Some(CanvasEvent::ScenePresented{identity,scene_revision,error:None,..})
            if *identity==prepared.source.identity&&*scene_revision==prepared.source.revision)
        {
            return;
        }
        let report = CanvasEvent::TextLayout {
            identity: prepared.source.identity,
            scene_revision: prepared.source.revision,
            source: prepared.document_source.clone(),
            file: prepared.file_source.clone(),
            projection: self.viewport.clone(),
            results: scene::paragraph::query(self, window),
        };
        if !self
            .text_layout_result
            .as_ref()
            .is_some_and(|(revision, previous)| *revision == self.revision && *previous == report)
        {
            self.text_layout_result = Some((self.revision, report.clone()));
            self.emit(report, cx);
        }
    }
}
