//! Drag hit testing uses exactly the successful native input frame's paths, clips and inverse basis.
use super::*;
use protocol::{
    transfer::{DropTarget, Position},
    ui::{CanvasAnchor, CanvasTargetRef},
};

/// Immutable geometry used by the window's OLE router; it carries no filesystem or guest authority.
#[derive(Clone)]
pub(in crate::ui::plugin) struct TransferFrame {
    /// The outer revision belongs to the successful paint, even if a newer drawing is already accepted.
    pub revision: u64,
    graphics: std::sync::Arc<scene::PreparedScene>,
    bounds: Bounds<Pixels>,
    basis: protocol::Rect,
}
impl TransferFrame {
    /// Borrow immutable geometry through the existing target module's native presentation boundary.
    pub(in crate::ui::plugin::canvas) fn from_frame(
        frame: &super::targets::frame::InputFrame,
    ) -> Self {
        Self {
            revision: frame.revision,
            graphics: frame.graphics.clone(),
            bounds: frame.bounds,
            basis: frame.basis,
        }
    }
    /// A native source uses the same frontmost path, clips and target epoch as ordinary pointer input.
    pub(in crate::ui::plugin) fn source_at(
        &self,
        x: f32,
        y: f32,
    ) -> Option<(CanvasAnchor, protocol::transfer::DragSource)> {
        if !self.bounds.contains(&point(px(x), px(y))) {
            return None;
        }
        let position = self.position(x, y);
        let (id, epoch) = self.graphics.targets.hit(position.x, position.y)?;
        let target = self
            .graphics
            .source
            .targets
            .iter()
            .find(|target| target.id == id && target.epoch == epoch)?;
        let source = target.drag_source.clone()?;
        let rect = self.graphics.targets.bounds_in(
            &id,
            epoch,
            super::targets::frame::content_viewport(self.bounds, self.basis),
        )?;
        Some((
            CanvasAnchor {
                identity: self.graphics.source.identity,
                scene_revision: self.graphics.source.revision,
                target: CanvasTargetRef { id, epoch },
                space: None,
                rect,
            },
            source,
        ))
    }
    /// Convert actual window logical coordinates back into the displayed Canvas content coordinates.
    pub(in crate::ui::plugin) fn position(&self, x: f32, y: f32) -> Position {
        Position {
            x: (x - self.bounds.origin.x / px(1.) - self.basis.x) / self.basis.w,
            y: (y - self.bounds.origin.y / px(1.) - self.basis.y) / self.basis.h,
        }
    }
    /// Hit the same frontmost path as pointer input, preserving target epoch and displayed scene version.
    pub(in crate::ui::plugin) fn drop_at(
        &self,
        x: f32,
        y: f32,
    ) -> Option<(CanvasAnchor, DropTarget)> {
        if !self.bounds.contains(&point(px(x), px(y))) {
            return None;
        }
        let position = self.position(x, y);
        let (id, epoch) = self.graphics.targets.hit(position.x, position.y)?;
        let target = self
            .graphics
            .source
            .targets
            .iter()
            .find(|target| target.id == id && target.epoch == epoch)?;
        let declaration = target.drop_target.clone()?;
        let rect = self.graphics.targets.bounds_in(
            &id,
            epoch,
            super::targets::frame::content_viewport(self.bounds, self.basis),
        )?;
        Some((
            CanvasAnchor {
                identity: self.graphics.source.identity,
                scene_revision: self.graphics.source.revision,
                target: CanvasTargetRef { id, epoch },
                space: None,
                rect,
            },
            declaration,
        ))
    }
}
