//! Native foreground layout uses the prepared scene and the same viewport/scroll projection as pixels.
use super::*;

/// One bounded presentation snapshot supplies popup placement but carries no input lease.
pub(super) struct AnchorGeometry {
    /// The same prepared geometry used by the successful native paint.
    graphics: std::sync::Arc<scene::PreparedScene>,
    /// Exact source and visual projection, independently of current modal input ownership.
    receipt: CanvasEvent,
    /// Canvas-local projection into native window logical coordinates.
    bounds: Bounds<Pixels>,
    basis: protocol::Rect,
    /// Coverage from the same successful native paint, including a partially waiting scroll buffer.
    content_clip: Option<protocol::Rect>,
}

/// The document owner supplies existing native controls; no GPUI object crosses the guest contract.
#[derive(Clone, Copy)]
pub(in crate::ui::plugin) struct OverlayLayout {
    /// Full control box, retained independently of clipping so editing/layout does not reflow.
    pub rect: protocol::Rect,
    /// Actual visible Canvas-local intersection of viewport, chosen-space clips and control box.
    pub clip: protocol::Rect,
    /// Scene/viewport zoom excluding OS DPI.
    pub scale: f32,
}
/// Native child construction consumes one immutable layout snapshot without owning scene state.
pub(in crate::ui::plugin) type OverlayRenderer = Rc<
    dyn Fn(
        &protocol::ui::CanvasOverlay,
        OverlayLayout,
        &mut Window,
        &mut App,
    ) -> gpui_kit::AnyElement,
>;

impl CanvasView {
    /// Retire presentation-only references when their native surface or source is withdrawn.
    pub(in crate::ui::plugin) fn discard_anchor_geometry(&mut self) {
        self.anchor_geometry = None;
    }
    /// Remember only successful displayed geometry and wake the document once for a changed anchor.
    pub(super) fn record_anchor_geometry(
        &mut self,
        receipt: &CanvasEvent,
        bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(graphics) = self.scene_graphics.clone() else {
            return;
        };
        if !matches!(receipt, CanvasEvent::ScenePresented { error: None, .. }) {
            if self.anchor_geometry.take().is_some() {
                if let Some(changed) = &self.anchor_changed {
                    changed(cx);
                }
            }
            return;
        }
        let basis = self.project(
            protocol::Rect {
                x: 0.,
                y: 0.,
                w: 1.,
                h: 1.,
            },
            bounds,
        );
        let same = self.anchor_geometry.as_ref().is_some_and(|old| {
            std::sync::Arc::ptr_eq(&old.graphics, &graphics)
                && old.receipt == *receipt
                && old.bounds == bounds
                && old.basis == basis
                && old.content_clip == self.retained_content_clip()
        });
        if !same {
            self.anchor_geometry = Some(AnchorGeometry {
                graphics,
                receipt: receipt.clone(),
                bounds,
                basis,
                content_clip: self.retained_content_clip(),
            });
            if let Some(changed) = &self.anchor_changed {
                changed(cx);
            }
        }
    }

    /// Resolve a live target against the actual displayed geometry, without authorizing any event.
    pub(in crate::ui::plugin) fn anchor_bounds(
        &self,
        anchor: &protocol::ui::CanvasAnchor,
        owner: &Document,
    ) -> Option<Bounds<Pixels>> {
        let shown = self.anchor_geometry.as_ref()?;
        if !shown.graphics.belongs_to(owner)
            || self.drawing.scene.as_ref() != Some(&shown.graphics.source)
            || !matches!(&shown.receipt,CanvasEvent::ScenePresented{projection,..} if *projection==self.viewport)
        {
            return None;
        }
        let placement = anchor.placement(&shown.graphics.source)?;
        if !placement.visible
            || shown
                .graphics
                .targets
                .bounds_in(
                    &anchor.target.id,
                    anchor.target.epoch,
                    targets::frame::clip_content(
                        targets::frame::content_viewport(shown.bounds, shown.basis),
                        shown.content_clip,
                    ),
                )
                .is_none()
        {
            return None;
        }
        let rect = anchor.rect;
        let points = [
            (rect.x, rect.y),
            (rect.x + rect.w, rect.y),
            (rect.x + rect.w, rect.y + rect.h),
            (rect.x, rect.y + rect.h),
        ]
        .map(|(x, y)| placement.transform.point(x, y));
        let min_x = points.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
        let min_y = points.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
        let max_x = points.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max);
        let max_y = points.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max);
        Some(Bounds::new(
            shown.bounds.origin
                + point(
                    px(shown.basis.x + min_x * shown.basis.w),
                    px(shown.basis.y + min_y * shown.basis.h),
                ),
            size(
                px((max_x - min_x) * shown.basis.w),
                px((max_y - min_y) * shown.basis.h),
            ),
        ))
    }
    /// Queue a host-originated focus return; the next actual display validates its target and source.
    pub(in crate::ui::plugin) fn request_overlay_focus(
        &mut self,
        identity: u64,
        target: protocol::ui::CanvasTargetRef,
    ) {
        if self.enabled
            && self.drawing.scene.as_ref().is_some_and(|scene| {
                scene.identity == identity
                    && scene.targets.iter().any(|item| {
                        item.id == target.id && item.epoch == target.epoch && item.focusable
                    })
            })
        {
            self.pending_overlay_focus = Some((identity, target));
        }
    }

    /// Construct foreground children only when their exact prepared artwork and live target are visible.
    pub(super) fn overlay_elements(
        &self,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<gpui_kit::AnyElement> {
        let Some(render) = &self.overlay_renderer else {
            return Vec::new();
        };
        let Some(graphics) = self
            .scene_graphics
            .as_ref()
            .filter(|scene| scene.drawing.is_ok())
        else {
            return Vec::new();
        };
        if !self.enabled
            || self.drawing.scene.as_ref() != Some(&graphics.source)
            || self.bounds.size.width <= px(0.)
            || self.bounds.size.height <= px(0.)
        {
            return Vec::new();
        }
        self.drawing
            .overlays
            .iter()
            .filter_map(|overlay| {
                let placement = overlay.anchor.placement(&graphics.source)?;
                if !placement.visible
                    || graphics
                        .targets
                        .bounds_in(
                            &overlay.anchor.target.id,
                            overlay.anchor.target.epoch,
                            self.target_content_viewport(),
                        )
                        .is_none()
                {
                    return None;
                }
                let (x, y) = placement.transform.point(
                    overlay.anchor.rect.x + overlay.rect.x,
                    overlay.anchor.rect.y + overlay.rect.y,
                );
                let scale = placement.transform.0[0];
                let content = protocol::Rect {
                    x,
                    y,
                    w: overlay.rect.w * scale,
                    h: overlay.rect.h * scale,
                };
                let rect = self.project(content, self.bounds);
                // A cropped field retains its editing state, but has no native element or Tab stop.
                if rect.w <= 0.
                    || rect.h <= 0.
                    || rect.x + rect.w <= 0.
                    || rect.y + rect.h <= 0.
                    || rect.x >= self.bounds.size.width / px(1.)
                    || rect.y >= self.bounds.size.height / px(1.)
                {
                    return None;
                }
                let basis = self.project(
                    protocol::Rect {
                        x: 0.,
                        y: 0.,
                        w: 1.,
                        h: 1.,
                    },
                    self.bounds,
                );
                let full = Bounds::new(point(px(rect.x), px(rect.y)), size(px(rect.w), px(rect.h)));
                let coverage = self.native_paint_clip(self.bounds);
                let mut visible = full.intersect(&Bounds::new(
                    coverage.origin - self.bounds.origin,
                    coverage.size,
                ));
                if let Some(clip) = overlay.anchor.native_clip(&graphics.source).ok()? {
                    // Intersect in content space first, so a huge harmless ancestor clip never
                    // overflows native projected bounds or changes the control's editing width.
                    let x = clip.x.max(content.x);
                    let y = clip.y.max(content.y);
                    let clip = protocol::Rect {
                        x,
                        y,
                        w: ((clip.x + clip.w).min(content.x + content.w) - x).max(0.),
                        h: ((clip.y + clip.h).min(content.y + content.h) - y).max(0.),
                    };
                    if clip.w <= 0. || clip.h <= 0. {
                        return None;
                    }
                    let clip = self.project(clip, self.bounds);
                    visible = visible.intersect(&Bounds::new(
                        point(px(clip.x), px(clip.y)),
                        size(px(clip.w), px(clip.h)),
                    ));
                }
                if visible.size.width <= px(0.) || visible.size.height <= px(0.) {
                    return None;
                }
                Some(render(
                    overlay,
                    OverlayLayout {
                        rect,
                        clip: protocol::Rect {
                            x: visible.origin.x / px(1.),
                            y: visible.origin.y / px(1.),
                            w: visible.size.width / px(1.),
                            h: visible.size.height / px(1.),
                        },
                        scale: scale * basis.w,
                    },
                    window,
                    cx,
                ))
            })
            .collect()
    }
}
