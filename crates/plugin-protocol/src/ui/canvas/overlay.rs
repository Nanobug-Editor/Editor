//! Foreground native children reuse document controls and retain scene-target lifetimes.
use super::{Canvas, CanvasPlacement, CanvasScene};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Stable scene-local interactive object; epochs prevent a removed ID acquiring old authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasTargetRef {
    /// Target ID in the enclosing Canvas scene.
    pub id: String,
    /// Nonzero target incarnation.
    pub epoch: u64,
}

/// A scene-space rectangle whose lifetime is bounded by an existing interactive target.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasAnchor {
    /// Scene semantic identity; replacement identities cannot inherit this anchor.
    pub identity: u64,
    /// Published drawing revision defining the anchor; never a future revision.
    pub scene_revision: u64,
    /// Owning interactive object; removal or epoch replacement retires the anchor.
    pub target: CanvasTargetRef,
    /// Optional group coordinate space; omission means Canvas content coordinates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space: Option<String>,
    /// Local logical rectangle used for placement, independent of the target hit shape.
    pub rect: crate::Rect,
}

/// Address scene geometry from an ordinary node tooltip or the document's existing popup.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasNodeAnchor {
    /// Canvas node in this same document; this reference grants no input or source authority.
    pub canvas: String,
    /// Target lifetime and local scene rectangle. A retired target closes the native popup.
    pub anchor: CanvasAnchor,
}

impl CanvasNodeAnchor {
    /// Resolve an enabled Canvas in the selected root/dialog branch, including while a popup owns input.
    /// This checks structural liveness only; native placement still requires a successful display.
    pub fn resolve<'a>(&self, document: &'a crate::ui::Document) -> Option<&'a crate::ui::Node> {
        let node = if let Some(dialog) = &document.dialog {
            dialog.content.find(&self.canvas)
        } else {
            document.root.find(&self.canvas)
        }?;
        (!node.disabled
            && matches!(&node.kind,crate::ui::Kind::Canvas(canvas)
            if canvas.scene.as_ref().and_then(|scene|self.anchor.placement(scene))
                .is_some_and(|placement|placement.visible)))
        .then_some(node)
    }

    /// Check bounded wire coordinates. Liveness is checked again against the actual native display.
    pub fn validate(&self) -> Result<(), String> {
        let identity =
            |id: &str| !id.is_empty() && id.len() <= 128 && !id.chars().any(char::is_control);
        if !identity(&self.canvas)
            || self.anchor.identity == 0
            || self.anchor.scene_revision == 0
            || self.anchor.target.epoch == 0
            || !identity(&self.anchor.target.id)
            || self.anchor.space.as_deref().is_some_and(|id| !identity(id))
            || !valid_rect(self.anchor.rect)
        {
            return Err("Invalid native Canvas popup anchor".into());
        }
        Ok(())
    }
}

/// An ordinary document subtree placed above artwork, clipped to the Canvas viewport and every
/// axis-aligned rectangular clip in its chosen space. Curved/path/non-axis-aligned native clips
/// are rejected explicitly; artwork keeps its full scene clipping support.
/// Effective native scale (group × VisualViewport, excluding OS DPI) is bounded to 1/64..8;
/// projected boxes remain within the same one-million-logical-pixel scene coordinate budget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasOverlay {
    /// Optional focus destination after removal; honored only for the focused native subtree
    /// and a still-live, actually displayed focusable target in this same Canvas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_return: Option<CanvasTargetRef>,
    /// Stable identity in this Canvas's foreground layer; at most 64 overlays.
    pub id: String,
    /// Target lifetime and coordinate space to which this control belongs.
    pub anchor: CanvasAnchor,
    /// Control box relative to the anchor rectangle's origin, in its local logical pixels.
    pub rect: crate::Rect,
    /// Existing native controls; their IDs participate in the whole document namespace.
    pub child: Box<crate::ui::Node>,
}

impl CanvasAnchor {
    /// Intersect native-representable ancestor clips in Canvas content coordinates.
    /// Curved clips or a rotated/skewed rectangle return an error before UI publication;
    /// `None` means this coordinate space adds no clip beyond the native Canvas viewport.
    pub fn native_clip(&self, scene: &CanvasScene) -> Result<Option<crate::Rect>, String> {
        let placement = self.placement(scene).ok_or("Stale Canvas native anchor")?;
        let mut result: Option<crate::Rect> = None;
        for (transform, geometry) in placement.clips {
            let super::SceneGeometry::Rect { rect, radii } = geometry else {
                return Err("Native Canvas controls require rectangular ancestor clips".into());
            };
            let [a, b, c, d, _, _] = transform.0;
            if radii.iter().any(|radius| *radius != 0.)
                || !((b == 0. && c == 0.) || (a == 0. && d == 0.))
            {
                return Err(
                    "Native Canvas controls require axis-aligned rectangular ancestor clips".into(),
                );
            }
            let (x0, y0) = transform.point(rect.x, rect.y);
            let (x1, y1) = transform.point(rect.x + rect.w, rect.y + rect.h);
            if [x0, y0, x1, y1].iter().any(|value| !value.is_finite()) {
                return Err("Native Canvas clip projection is nonfinite".into());
            }
            let next = crate::Rect {
                x: x0.min(x1),
                y: y0.min(y1),
                w: (x1 - x0).abs(),
                h: (y1 - y0).abs(),
            };
            if !valid_rect(next)
                || ![next.x + next.w, next.y + next.h]
                    .iter()
                    .all(|value| value.is_finite())
            {
                return Err("Native Canvas clip projection exceeds its finite layout range".into());
            }
            result = Some(if let Some(before) = result {
                let x = before.x.max(next.x);
                let y = before.y.max(next.y);
                crate::Rect {
                    x,
                    y,
                    w: ((before.x + before.w).min(next.x + next.w) - x).max(0.),
                    h: ((before.y + before.h).min(next.y + next.h) - y).max(0.),
                }
            } else {
                next
            });
        }
        Ok(result)
    }
    /// Resolve a live, previously published anchor against `scene`; grants no native event authority.
    pub fn placement<'a>(&self, scene: &'a CanvasScene) -> Option<CanvasPlacement<'a>> {
        if self.identity != scene.identity
            || self.scene_revision == 0
            || self.scene_revision > scene.revision
            || !valid_rect(self.rect)
        {
            return None;
        }
        let target = scene
            .targets
            .iter()
            .find(|target| target.id == self.target.id && target.epoch == self.target.epoch)?;
        let mut placement = scene.group_placement(self.space.as_deref())?;
        // Coordinate space chooses placement only; the owning target's hidden ancestry still retires UI.
        placement.visible &= scene.group_placement(target.space.as_deref())?.visible;
        Some(placement)
    }
}

impl Canvas {
    /// Validate final native layout magnification and both content/projected boxes before font allocation.
    pub(in crate::ui) fn validate_overlay_projection(
        &self,
        viewport: Option<&super::super::VisualViewport>,
    ) -> Result<(), String> {
        let Some(scene) = &self.scene else {
            return Ok(());
        };
        let zoom = viewport
            .and_then(|viewport| viewport.transform)
            .map_or(1., |transform| transform.scale);
        for overlay in &self.overlays {
            overlay.anchor.native_clip(scene)?;
            let placement = overlay
                .anchor
                .placement(scene)
                .ok_or("Stale Canvas native anchor")?;
            let scale = placement.transform.0[0];
            if !(1. / 64. ..=8.).contains(&(scale * zoom)) {
                return Err("Native Canvas control scale must remain within 1/64..8".into());
            }
            let (x, y) = placement.transform.point(
                overlay.anchor.rect.x + overlay.rect.x,
                overlay.anchor.rect.y + overlay.rect.y,
            );
            let rect = crate::Rect {
                x,
                y,
                w: overlay.rect.w * scale,
                h: overlay.rect.h * scale,
            };
            let bounded = |rect: crate::Rect| {
                valid_rect(rect)
                    && [rect.x + rect.w, rect.y + rect.h]
                        .into_iter()
                        .all(|value| value.is_finite() && value.abs() <= 1_000_000.)
            };
            if !bounded(rect) {
                return Err("Native Canvas control exceeds content coordinate quota".into());
            }
            if let Some(viewport) = viewport
                && let Some(content) = viewport.content
            {
                let projected = viewport.transform.unwrap_or_default().project(
                    rect,
                    content,
                    super::super::ContentSize {
                        width: 0.,
                        height: 0.,
                    },
                );
                if !bounded(projected) {
                    return Err("Native Canvas control exceeds projected coordinate quota".into());
                }
            }
        }
        Ok(())
    }

    /// Bound overlays and reject native rotations, shear, reflections and nonuniform scaling.
    pub(super) fn validate_overlays(&self) -> Result<(), String> {
        if self.overlays.is_empty() {
            return Ok(());
        }
        let scene = self
            .scene
            .as_ref()
            .ok_or("Native overlays require a scene")?;
        if self.overlays.len() > 64 {
            return Err("Canvas native overlay quota exceeded".into());
        }
        let mut ids = BTreeSet::new();
        for overlay in &self.overlays {
            if overlay.id.is_empty()
                || overlay.id.len() > 128
                || !ids.insert(&overlay.id)
                || !valid_rect(overlay.rect)
            {
                return Err("Invalid Canvas native overlay identity or rectangle".into());
            }
            let placement = overlay
                .anchor
                .placement(scene)
                .ok_or("Stale Canvas native anchor")?;
            if overlay.focus_return.as_ref().is_some_and(|focus| {
                !scene.targets.iter().any(|target| {
                    target.id == focus.id && target.epoch == focus.epoch && target.focusable
                })
            }) {
                return Err("Native overlay focus return requires a live focusable target".into());
            }
            let [a, b, c, d, _, _] = placement.transform.0;
            if b != 0. || c != 0. || a <= 0. || a != d {
                return Err(
                    "Native Canvas controls require positive uniform axis-aligned scale".into(),
                );
            }
        }
        Ok(())
    }
}

/// Reject nonfinite, negative-size or unbounded coordinates before native layout allocation.
fn valid_rect(rect: crate::Rect) -> bool {
    [rect.x, rect.y, rect.w, rect.h]
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 1_000_000.)
        && rect.w >= 0.
        && rect.h >= 0.
}
