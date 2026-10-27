//! Atomic retained artwork changes; the host retains ownership and validates the complete expansion.
use super::*;
use crate::api::{ErrorCode, Failure};
use std::collections::BTreeSet;
mod bounds;
mod mutation;
mod publication;
pub use publication::{RetainedPublication, retained_publication};

/// Canvas 1.10 independently bounds encoded patches and their expanded retained artwork.
pub const MAX_SCENE_PATCH_BYTES: usize = 2 * 1024 * 1024;

/// A Canvas 1.10 delta addresses one existing document and one exact accepted scene base.
/// UI revisions remain explicit, so a later full View cannot borrow a host-invented revision.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasScenePatch {
    /// Existing declared panel, owned by the calling instance.
    pub panel: String,
    /// Existing Canvas node in that panel, including its active native ancestor lineage.
    pub canvas: String,
    /// Exact accepted outer UI revision; checked by the host before any publication.
    pub base_ui_revision: u64,
    /// Strictly newer outer UI revision shared by this atomic publication.
    pub next_ui_revision: u64,
    /// Nonzero current scene incarnation; never changes through a patch.
    pub identity: u64,
    /// Exact accepted scene version, independently of the last native presentation.
    pub base_revision: u64,
    /// Strictly newer scene version; duplicate and out-of-order deltas fail without mutation.
    pub next_revision: u64,
    /// Ordered operations, limited to 1024 and the independent encoded/expanded byte budgets.
    pub operations: Vec<CanvasSceneOperation>,
    /// Optional conservative hint; the host always unions actual old/new affected bounds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub damage: Option<crate::Rect>,
}

/// Scene-local operations grant no native-object, filesystem or cross-instance authority.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum CanvasSceneOperation {
    /// Advance an unchanged scene base as part of an atomic UI/projection publication.
    Advance,
    /// Insert one bounded subtree at an explicit sibling position.
    Insert {
        /// Existing parent group; None addresses the scene roots.
        parent: Option<String>,
        /// Insertion index in that parent's ordered children.
        index: usize,
        /// New subtree; duplicate or dangling IDs reject the final transaction.
        node: SceneNode,
    },
    /// Move an existing subtree without serializing its drawing data again.
    Move {
        /// Existing subtree identity.
        id: String,
        /// Existing destination group; None addresses roots.
        parent: Option<String>,
        /// Destination index after removing the original subtree.
        index: usize,
    },
    /// Change group attributes while retaining all descendants by identity.
    Group {
        /// Existing group identity.
        id: String,
        /// Complete replacement affine placement.
        transform: SceneTransform,
        /// Complete replacement clip, including explicit removal.
        clip: Option<SceneGeometry>,
        /// Source-over group opacity.
        opacity: f32,
    },
    /// Insert/replace/reorder one input declaration; native epoch rules still apply.
    UpsertTarget {
        /// Position after removing an existing declaration of this ID.
        index: usize,
        /// Complete current input declaration, checked by ordinary lifetime admission.
        target: HitTarget,
    },
    /// Remove a target and dependent native overlays/semantics atomically.
    RemoveTarget {
        /// Existing scene-local input identity.
        id: String,
    },
    /// Insert/replace/reorder one visible semantic object.
    UpsertSemantic {
        /// Position after removing an existing declaration of this ID.
        index: usize,
        /// Complete semantic declaration, including its parent/reference relationships.
        node: CanvasSemanticNode,
    },
    /// Remove one semantic parent and its dependent descendants.
    RemoveSemantic {
        /// Existing semantic parent or leaf identity.
        id: String,
    },
    /// Insert/replace/reorder one ordinary native foreground subtree.
    UpsertOverlay {
        /// Foreground position after removing an existing declaration of this ID.
        index: usize,
        /// Complete native subtree and its scene-owned anchor.
        overlay: CanvasOverlay,
    },
    /// Retire one ordinary native foreground subtree.
    RemoveOverlay {
        /// Existing foreground subtree identity.
        id: String,
    },
    /// Replace bounded non-artwork Canvas properties, including scroll/query metadata.
    Properties {
        /// Complete non-artwork projection/query declarations.
        value: CanvasSceneProperties,
    },
    /// Remove an existing subtree and retire input/semantics whose coordinate owner disappears.
    Remove {
        /// Existing stable drawing ID; unknown IDs reject the whole delta.
        id: String,
    },
    /// Replace an existing drawing subtree at the same stable ID and sibling position.
    Replace {
        /// Complete replacement subtree; IDs and all expanded references are validated atomically.
        node: SceneNode,
    },
}

/// Non-artwork declarations participating in the same retained transaction.
/// Ordinary UI ownership, capability and source checks still run on the complete resulting document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasSceneProperties {
    /// Native typography inherited by the artwork.
    pub font: crate::FontStyle,
    /// Existing vertical range, mutually exclusive with the new two-axis range.
    pub scroll: Option<ScrollRange>,
    /// Existing vertical content incarnation.
    pub scroll_epoch: Option<u64>,
    /// Existing vertical painted coverage.
    pub scroll_buffer: Option<ScrollBuffer>,
    /// Two-axis content, acknowledged position and painted coverage.
    pub scroll_2d: Option<SceneScroll2D>,
    /// Bounded original-text point/range queries.
    pub text_queries: Vec<CanvasTextQuery>,
    /// Existing native sample measurements.
    pub measure_text: Vec<TextMeasure>,
    /// Legacy Canvas-level keyboard focus opt-in.
    pub focusable: bool,
    /// Legacy grid declaration; invalid structured combinations are still rejected.
    pub grid: bool,
    /// Legacy caret declaration, independent of target-owned native IME.
    pub caret: Option<crate::Rect>,
    /// Existing column declaration; structured-scene exclusion remains enforced.
    pub column_resize: Option<ColumnResize>,
}
impl From<&Canvas> for CanvasSceneProperties {
    fn from(value: &Canvas) -> Self {
        Self {
            font: value.font.clone(),
            scroll: value.scroll.clone(),
            scroll_epoch: value.scroll_epoch,
            scroll_buffer: value.scroll_buffer.clone(),
            scroll_2d: value.scroll_2d.clone(),
            text_queries: value.text_queries.clone(),
            measure_text: value.measure_text.clone(),
            focusable: value.focusable,
            grid: value.grid,
            caret: value.caret,
            column_resize: value.column_resize.clone(),
        }
    }
}

/// A validated candidate and conservative logical damage, neither published nor rendered yet.
pub struct CanvasPatchResult {
    /// The complete candidate; callers must still validate its enclosing UI resource references.
    pub canvas: Canvas,
    /// Union of removed and added pixels, independent of a smaller guest-provided hint.
    pub damage: Option<crate::Rect>,
}

/// A complete staged UI document, with authority and image references validated together.
pub struct CanvasDocumentPatchResult {
    /// Publish only after capability, instance and panel admission succeeds.
    pub document: crate::ui::Document,
    /// Actual affected logical pixels; a renderer may repaint a larger region.
    pub damage: Option<crate::Rect>,
}

impl CanvasScenePatch {
    /// Restore against exact UI and scene bases without changing either accepted input.
    /// Validates the complete resulting document; StaleRevision reports the current UI version.
    /// The host still checks the negotiated capability and ownership of `panel`.
    pub fn apply_document(
        &self,
        original: &crate::ui::Document,
    ) -> Result<CanvasDocumentPatchResult, Failure> {
        if original.revision != self.base_ui_revision {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                format!(
                    "Scene patch UI base is stale; current revision={}",
                    original.revision
                ),
            ));
        }
        self.apply_staged_document(original, original)
    }

    /// Combine this accepted scene base with a paired ordinary UI subtree patch.
    /// The staged Canvas is either the unchanged accepted value or Canvas::default() as an explicit
    /// neutral placeholder. New full artwork cannot pretend to be the old base. Sources must match.
    /// The caller checks the shared base/next UI revisions and rejects duplicate Canvas deltas.
    pub fn apply_staged_document(
        &self,
        original: &crate::ui::Document,
        staged: &crate::ui::Document,
    ) -> Result<CanvasDocumentPatchResult, Failure> {
        if original.revision != self.base_ui_revision
            || ![self.base_ui_revision, self.next_ui_revision].contains(&staged.revision)
        {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Scene/UI patch bases do not match the accepted document",
            ));
        }
        if staged.source != original.source || staged.file != original.file {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Scene/UI patch source was replaced",
            ));
        }
        let mut base = original.clone();
        let mut old = None;
        let mut locate = |node: &mut crate::ui::Node| {
            if node.id == self.canvas
                && let crate::ui::Kind::Canvas(canvas) = &node.kind
            {
                old = Some(canvas.clone());
            }
            Ok(())
        };
        base.root
            .try_visit_mut(&mut locate)
            .map_err(|message| Failure::new(ErrorCode::InvalidRequest, message))?;
        if let Some(toolbar) = &mut base.editor_toolbar {
            toolbar
                .try_visit_mut(&mut locate)
                .map_err(|message| Failure::new(ErrorCode::InvalidRequest, message))?;
        }
        if let Some(dialog) = &mut base.dialog {
            dialog
                .content
                .try_visit_mut(&mut locate)
                .map_err(|message| Failure::new(ErrorCode::InvalidRequest, message))?;
        }
        let old = old.ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidRequest,
                "Unknown accepted Canvas patch target",
            )
        })?;
        let mut document = staged.clone();
        let mut found = false;
        let mut damage = None;
        let mut failure = None;
        let mut visitor = |node: &mut crate::ui::Node| -> Result<(), String> {
            if node.id != self.canvas {
                return Ok(());
            }
            if found {
                return Err("Duplicate Canvas node".into());
            }
            found = true;
            let crate::ui::Kind::Canvas(canvas) = &mut node.kind else {
                return Err("Scene patch target is not a Canvas".into());
            };
            if *canvas != old && *canvas != Canvas::default() {
                return Err("UI patch cannot replace a scene delta's accepted artwork base".into());
            }
            match self.apply(&old) {
                Ok(candidate) => {
                    *canvas = candidate.canvas;
                    damage = candidate.damage;
                    Ok(())
                }
                Err(error) => {
                    failure = Some(error);
                    Err("Scene patch rejected".into())
                }
            }
        };
        let walked = (|| {
            document.root.try_visit_mut(&mut visitor)?;
            if let Some(toolbar) = &mut document.editor_toolbar {
                toolbar.try_visit_mut(&mut visitor)?;
            }
            if let Some(dialog) = &mut document.dialog {
                dialog.content.try_visit_mut(&mut visitor)?;
            }
            Ok::<(), String>(())
        })();
        walked.map_err(|message| {
            failure.unwrap_or_else(|| Failure::new(ErrorCode::InvalidRequest, message))
        })?;
        if !found {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Unknown Canvas patch target",
            ));
        }
        document.revision = self.next_ui_revision;
        document
            .validate()
            .map_err(|message| Failure::new(ErrorCode::InvalidRequest, message))?;
        Ok(CanvasDocumentPatchResult { document, damage })
    }
    /// Apply to an immutable accepted base. Failure preserves the input and returns the current
    /// identity/revision on StaleRevision; the caller can explicitly resynchronize with a full View.
    /// Permission, capability, document revision and instance ownership remain host obligations.
    pub fn apply(&self, original: &Canvas) -> Result<CanvasPatchResult, Failure> {
        let before = original.scene.as_ref().ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidRequest,
                "Scene patch requires structured artwork",
            )
        })?;
        if self.identity != before.identity || self.base_revision != before.revision {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                format!(
                    "Scene patch base is stale; current identity={} revision={}",
                    before.identity, before.revision
                ),
            ));
        }
        if self.panel.is_empty()
            || self.panel.len() > 256
            || self.canvas.is_empty()
            || self.canvas.len() > 256
            || self.base_ui_revision == 0
            || self.next_ui_revision <= self.base_ui_revision
            || self.next_revision <= self.base_revision
            || self.operations.is_empty()
        {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Invalid scene patch addressing or revision",
            ));
        }
        if self.operations.len() > 1024 || encoded_len(self)? > MAX_SCENE_PATCH_BYTES {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Scene patch encoded budget exceeded",
            ));
        }
        if self.damage.is_some_and(|rect| {
            [
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                rect.x + rect.w,
                rect.y + rect.h,
            ]
            .into_iter()
            .any(|value| !value.is_finite() || value.abs() > 1_000_000.)
                || rect.w < 0.
                || rect.h < 0.
        }) {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Invalid scene damage hint",
            ));
        }
        let mut canvas = original.clone();
        for operation in &self.operations {
            mutation::apply(&mut canvas, operation)?;
        }
        canvas.scene.as_mut().unwrap().revision = self.next_revision;
        retire_removed(&mut canvas, before);
        if encoded_len(&canvas)? > MAX_SCENE_PATCH_BYTES {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Retained scene expanded budget exceeded",
            ));
        }
        canvas
            .validate()
            .map_err(|message| Failure::new(ErrorCode::InvalidRequest, message))?;
        let changed = self
            .operations
            .iter()
            .filter_map(mutation::drawing_id)
            .collect::<BTreeSet<_>>();
        let damage = if self
            .operations
            .iter()
            .any(|op| matches!(op, CanvasSceneOperation::Properties { .. }))
        {
            bounds::union(bounds::canvas(original), bounds::canvas(&canvas))
        } else {
            bounds::union(
                bounds::union(
                    bounds::affected(before, &changed),
                    bounds::affected(canvas.scene.as_ref().unwrap(), &changed),
                ),
                bounds::metadata_changed(original, &canvas),
            )
        };
        Ok(CanvasPatchResult {
            canvas,
            damage: bounds::union(damage, self.damage),
        })
    }
}

/// Count the exact public encoding separately from the existing host-request budget.
fn encoded_len(value: &impl Serialize) -> Result<usize, Failure> {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .map_err(|error| Failure::new(ErrorCode::InvalidRequest, error.to_string()))
}

/// Replace in place to preserve explicit z order; only the cloned candidate is ever mutated.
fn replace(nodes: &mut [SceneNode], replacement: &SceneNode) -> bool {
    for node in nodes {
        if node.id() == replacement.id() {
            *node = replacement.clone();
            return true;
        }
        if let SceneNode::Group { children, .. } = node
            && replace(children, replacement)
        {
            return true;
        }
    }
    false
}

/// Removing a subtree preserves every surviving sibling's declared drawing order.
fn remove(nodes: &mut Vec<SceneNode>, id: &str) -> bool {
    if let Some(index) = nodes.iter().position(|node| node.id() == id) {
        nodes.remove(index);
        return true;
    }
    nodes.iter_mut().any(|node| {
        if let SceneNode::Group { children, .. } = node {
            remove(children, id)
        } else {
            false
        }
    })
}

/// Retire only references to previously existing owners; newly invented unknown references still fail.
fn retire_removed(canvas: &mut Canvas, before: &CanvasScene) {
    let ids = |scene: &CanvasScene| {
        let mut ids = BTreeSet::new();
        for root in &scene.roots {
            root.visit(&mut |node| {
                ids.insert(node.id().to_owned());
            });
        }
        ids
    };
    let scene = canvas.scene.as_mut().unwrap();
    let current = ids(scene);
    let removed = ids(before)
        .difference(&current)
        .cloned()
        .collect::<BTreeSet<_>>();
    scene.targets.retain(|target| {
        target.space.as_ref().is_none_or(|id| !removed.contains(id))
            && target
                .text_input
                .as_ref()
                .is_none_or(|input| !removed.contains(&input.paragraph))
    });
    canvas
        .text_queries
        .retain(|query| !removed.contains(&query.paragraph));
    let targets = scene
        .targets
        .iter()
        .map(|target| target.id.clone())
        .collect::<BTreeSet<_>>();
    let removed_targets = before
        .targets
        .iter()
        .filter(|target| !targets.contains(&target.id))
        .map(|target| target.id.clone())
        .collect::<BTreeSet<_>>();
    canvas.overlays.retain(|overlay| {
        !removed_targets.contains(&overlay.anchor.target.id)
            && overlay
                .anchor
                .space
                .as_ref()
                .is_none_or(|id| !removed.contains(id))
            && overlay
                .focus_return
                .as_ref()
                .is_none_or(|target| !removed_targets.contains(&target.id))
    });
    scene.semantics.retain(|node| {
        node.space.as_ref().is_none_or(|id| !removed.contains(id))
            && node
                .target
                .as_ref()
                .is_none_or(|id| !removed_targets.contains(id))
    });
    let mut missing = before
        .semantics
        .iter()
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();
    for node in &scene.semantics {
        missing.remove(&node.id);
    }
    // A validated semantic forest is at most 32 levels; each pass retires one descendant level.
    for _ in 0..32 {
        let mut retired = Vec::new();
        scene.semantics.retain(|node| {
            let keep = node.parent.as_ref().is_none_or(|id| !missing.contains(id));
            if !keep {
                retired.push(node.id.clone());
            }
            keep
        });
        if retired.is_empty() {
            break;
        }
        missing.extend(retired);
    }
}
