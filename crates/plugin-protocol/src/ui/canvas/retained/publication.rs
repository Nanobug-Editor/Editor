//! One SDK publication decision composes local artwork changes with existing ordinary UI reuse.
use super::*;
use std::collections::BTreeMap;

/// An immutable wire baseline and its publication; this owns no domain editing state or undo stack.
pub struct RetainedPublication {
    /// Actual next baseline, or the previous baseline when nothing meaningful changed.
    pub document: crate::ui::Document,
    /// Full initialization/resynchronization or one atomic UI/scene delta transaction.
    pub output: crate::api::Output,
}

/// Publish ordinary UI and structured artwork through one public, reusable decision.
/// Requires ui.canvas 1.10 and ui.incremental 1 when both kinds of delta are emitted.
/// `previous` is the immutable accepted wire baseline, not another editable model. Clear it for an
/// explicit resynchronization or after publication rejection. Source/identity replacement and
/// oversized structural deltas use a full View. Unchanged artwork is never serialized into a delta.
/// This constructs wire data; the host validates the complete expansion and all resource authority.
/// Errors cover bounded diff construction/encoding, not a promise that the host accepted a View.
pub fn retained_publication(
    panel: &str,
    previous: Option<&crate::ui::Document>,
    next: crate::ui::Document,
) -> Result<RetainedPublication, Failure> {
    let Some(previous) = previous else {
        return Ok(full(panel, next));
    };
    if previous.source != next.source
        || previous.file != next.file
        || next.revision <= previous.revision
    {
        return Ok(full(panel, next));
    }
    let old = canvases(previous);
    let new = canvases(&next);
    if old.keys().ne(new.keys())
        || new
            .iter()
            .any(|(id, canvas)| match (&old[id].scene, &canvas.scene) {
                (Some(a), Some(b)) => a.identity != b.identity || b.revision < a.revision,
                (None, None) => false,
                _ => true,
            })
    {
        return Ok(full(panel, next));
    }
    // Do not turn a guest's revision-only clock tick into an idle native redraw.
    let mut meaningful = next.clone();
    meaningful.revision = previous.revision;
    visit_mut(&mut meaningful, &mut |node| {
        if let crate::ui::Kind::Canvas(canvas) = &mut node.kind
            && let Some(before) = old.get(&node.id)
            && let (Some(scene), Some(before)) = (&mut canvas.scene, &before.scene)
        {
            scene.revision = before.revision;
        }
        Ok(())
    })?;
    if meaningful == *previous {
        return Ok(RetainedPublication {
            document: previous.clone(),
            output: Default::default(),
        });
    }
    let mut patches = Vec::new();
    // An outer delta restores its native child owners before a child's independent delta runs.
    let mut order = Vec::new();
    let mut ordered = next.clone();
    visit_mut(&mut ordered, &mut |node| {
        if matches!(node.kind, crate::ui::Kind::Canvas(_)) {
            order.push(node.id.clone());
        }
        Ok(())
    })?;
    let matched = new.keys().cloned().collect::<BTreeSet<_>>();
    for id in &order {
        let canvas = &new[id];
        let before = &old[id];
        let (Some(a), Some(b)) = (&before.scene, &canvas.scene) else {
            continue;
        };
        if b.revision <= a.revision {
            if before != canvas {
                return Ok(full(panel, next));
            }
            continue;
        }
        let mut before_local = before.clone();
        let mut next_local = canvas.clone();
        neutralize_children(&mut before_local, &matched)?;
        neutralize_children(&mut next_local, &matched)?;
        let operations = match operations(&before_local, &next_local) {
            Ok(operations) => operations,
            Err(_) => return Ok(full(panel, next)),
        };
        let patch = CanvasScenePatch {
            panel: panel.into(),
            canvas: id.clone(),
            base_ui_revision: previous.revision,
            next_ui_revision: next.revision,
            identity: a.identity,
            base_revision: a.revision,
            next_revision: b.revision,
            operations,
            damage: None,
        };
        if encoded_len(&patch)? > MAX_SCENE_PATCH_BYTES {
            return Ok(full(panel, next));
        }
        patches.push(patch);
    }
    if patches.len() > 64 || encoded_len(&patches)? > MAX_SCENE_PATCH_BYTES {
        return Ok(full(panel, next));
    }
    let ids = patches
        .iter()
        .map(|patch| patch.canvas.clone())
        .collect::<BTreeSet<_>>();
    let mut shell = next.clone();
    neutralize(&mut shell, &ids)?;
    let mut old_shell = previous.clone();
    old_shell.revision = next.revision;
    neutralize(&mut old_shell, &ids)?;
    let mut output = crate::api::Output {
        scene_patches: patches,
        ..Default::default()
    };
    if shell != old_shell {
        let reused = crate::ui::incremental::compact(previous, &mut shell);
        output.view_patches.push(crate::api::ViewPatch {
            panel: panel.into(),
            base_revision: previous.revision,
            document: shell,
            reused,
        });
    }
    // A Canvas-free publication still benefits from the existing UI subtree contract.
    if output.scene_patches.is_empty() && output.view_patches.is_empty() {
        return Ok(full(panel, next));
    }
    Ok(RetainedPublication {
        document: next,
        output,
    })
}

/// Initialization and explicit recovery never claim to share an unconfirmed old scene base.
fn full(panel: &str, document: crate::ui::Document) -> RetainedPublication {
    let output = crate::api::Output {
        views: vec![crate::api::View {
            panel: panel.into(),
            document: document.clone(),
        }],
        ..Default::default()
    };
    RetainedPublication { document, output }
}
/// Every ordinary owner branch participates; inactive native declarations remain instance owned.
fn visit_mut(
    document: &mut crate::ui::Document,
    visitor: &mut impl FnMut(&mut crate::ui::Node) -> Result<(), String>,
) -> Result<(), Failure> {
    document.root.try_visit_mut(visitor).map_err(invalid)?;
    if let Some(toolbar) = &mut document.editor_toolbar {
        toolbar.try_visit_mut(visitor).map_err(invalid)?;
    }
    if let Some(dialog) = &mut document.dialog {
        dialog.content.try_visit_mut(visitor).map_err(invalid)?;
    }
    Ok(())
}
/// Index immutable wire owners, including inactive controls and overlay children.
fn canvases(document: &crate::ui::Document) -> BTreeMap<String, Canvas> {
    let mut values = BTreeMap::new();
    let mut visitor = |node: &crate::ui::Node| {
        if let crate::ui::Kind::Canvas(canvas) = &node.kind {
            values.insert(node.id.clone(), canvas.clone());
        }
    };
    document.root.visit(&mut visitor);
    if let Some(toolbar) = &document.editor_toolbar {
        toolbar.visit(&mut visitor);
    }
    if let Some(dialog) = &document.dialog {
        dialog.content.visit(&mut visitor);
    }
    values
}
/// Keep ordinary node properties while replacing separately patched artwork with a strict placeholder.
fn neutralize(document: &mut crate::ui::Document, ids: &BTreeSet<String>) -> Result<(), Failure> {
    visit_mut(document, &mut |node| {
        if ids.contains(&node.id)
            && let crate::ui::Kind::Canvas(canvas) = &mut node.kind
        {
            *canvas = Canvas::default();
        }
        Ok(())
    })
}
/// Artwork inside a native overlay is published by its own ordered delta, never copied by its owner.
fn neutralize_children(canvas: &mut Canvas, ids: &BTreeSet<String>) -> Result<(), Failure> {
    for overlay in &mut canvas.overlays {
        overlay
            .child
            .try_visit_mut(&mut |node| {
                if ids.contains(&node.id)
                    && let crate::ui::Kind::Canvas(canvas) = &mut node.kind
                {
                    *canvas = Canvas::default();
                }
                Ok(())
            })
            .map_err(invalid)?;
    }
    Ok(())
}
/// Preserve a typed construction failure without granting publication authority.
fn invalid(message: String) -> Failure {
    Failure::new(ErrorCode::InvalidRequest, message)
}

/// Unchanged leaves use borrowed identity/placement indexes; only actual changes clone their data.
fn operations(before: &Canvas, next: &Canvas) -> Result<Vec<CanvasSceneOperation>, Failure> {
    let source = before.scene.as_ref().unwrap();
    let target = next.scene.as_ref().unwrap();
    let mut old = BTreeMap::new();
    index(&source.roots, None, &mut old);
    let mut wanted = BTreeSet::new();
    for root in &target.roots {
        root.visit(&mut |node| {
            wanted.insert(node.id().to_owned());
        });
    }
    let mut candidate = before.clone();
    let mut operations = Vec::new();
    for (position, node) in target.roots.iter().enumerate() {
        sync_node(&mut candidate, &mut operations, &old, None, position, node)?;
    }
    prune(&mut candidate, &mut operations, &wanted, None)?;
    sync_targets(&mut candidate, &mut operations, &target.targets)?;
    sync_semantics(&mut candidate, &mut operations, &target.semantics)?;
    sync_overlays(&mut candidate, &mut operations, &next.overlays)?;
    let properties = CanvasSceneProperties::from(next);
    if CanvasSceneProperties::from(&candidate) != properties {
        emit(
            &mut candidate,
            &mut operations,
            CanvasSceneOperation::Properties { value: properties },
        )?;
    }
    candidate.scene.as_mut().unwrap().revision = target.revision;
    retire_removed(&mut candidate, source);
    if candidate != *next {
        return Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Scene diff did not reproduce the requested document",
        ));
    }
    if operations.is_empty() {
        operations.push(CanvasSceneOperation::Advance);
    }
    Ok(operations)
}
type Old<'a> = BTreeMap<&'a str, (Option<&'a str>, usize, &'a SceneNode)>;
/// Borrow stable IDs and original sibling positions instead of copying unchanged leaves.
fn index<'a>(nodes: &'a [SceneNode], parent: Option<&'a str>, out: &mut Old<'a>) {
    for (position, node) in nodes.iter().enumerate() {
        out.insert(node.id(), (parent, position, node));
        if let SceneNode::Group { children, .. } = node {
            index(children, Some(node.id()), out);
        }
    }
}
/// Locate current candidate placement after every preceding move or insertion.
fn placement(
    nodes: &[SceneNode],
    id: &str,
    parent: Option<&str>,
) -> Option<(Option<String>, usize)> {
    for (position, node) in nodes.iter().enumerate() {
        if node.id() == id {
            return Some((parent.map(str::to_owned), position));
        }
        if let SceneNode::Group { children, .. } = node
            && let Some(found) = placement(children, id, Some(node.id()))
        {
            return Some(found);
        }
    }
    None
}
/// Restore requested drawing order and attributes, retaining existing descendants whenever possible.
fn sync_node(
    candidate: &mut Canvas,
    operations: &mut Vec<CanvasSceneOperation>,
    old: &Old<'_>,
    parent: Option<&str>,
    position: usize,
    next: &SceneNode,
) -> Result<(), Failure> {
    let current = placement(&candidate.scene.as_ref().unwrap().roots, next.id(), None);
    // Earlier insertions/moves may have displaced an otherwise unchanged sibling.
    if current
        .as_ref()
        .is_some_and(|(p, i)| p.as_deref() == parent && *i == position)
        && old
            .get(next.id())
            .is_some_and(|(_, _, node)| **node == *next)
    {
        return Ok(());
    }
    if let Some((p, i)) = current {
        if p.as_deref() != parent || i != position {
            emit(
                candidate,
                operations,
                CanvasSceneOperation::Move {
                    id: next.id().into(),
                    parent: parent.map(str::to_owned),
                    index: position,
                },
            )?;
        }
    } else {
        let mut inserted = next.clone();
        if let SceneNode::Group { children, .. } = &mut inserted {
            children.clear();
        }
        emit(
            candidate,
            operations,
            CanvasSceneOperation::Insert {
                parent: parent.map(str::to_owned),
                index: position,
                node: inserted,
            },
        )?;
    }
    let current = mutation::find_mut(&mut candidate.scene.as_mut().unwrap().roots, next.id())
        .unwrap()
        .clone();
    match (current, next) {
        (
            SceneNode::Group {
                transform: a,
                clip: b,
                opacity: c,
                ..
            },
            SceneNode::Group {
                transform,
                clip,
                opacity,
                children,
                ..
            },
        ) => {
            if a != *transform || b != *clip || c != *opacity {
                emit(
                    candidate,
                    operations,
                    CanvasSceneOperation::Group {
                        id: next.id().into(),
                        transform: *transform,
                        clip: clip.clone(),
                        opacity: *opacity,
                    },
                )?;
            }
            for (index, node) in children.iter().enumerate() {
                sync_node(candidate, operations, old, Some(next.id()), index, node)?;
            }
        }
        (_, SceneNode::Group { children, .. }) => {
            let mut value = next.clone();
            if let SceneNode::Group { children, .. } = &mut value {
                children.clear();
            }
            emit(
                candidate,
                operations,
                CanvasSceneOperation::Replace { node: value },
            )?;
            for (index, node) in children.iter().enumerate() {
                sync_node(candidate, operations, old, Some(next.id()), index, node)?;
            }
        }
        (current, _) => {
            if current != *next {
                emit(
                    candidate,
                    operations,
                    CanvasSceneOperation::Replace { node: next.clone() },
                )?;
            }
        }
    }
    Ok(())
}
/// Retire unwanted nodes after all moves, so cross-parent moves cannot lose their source subtree.
fn prune(
    candidate: &mut Canvas,
    operations: &mut Vec<CanvasSceneOperation>,
    wanted: &BTreeSet<String>,
    parent: Option<&str>,
) -> Result<(), Failure> {
    let nodes = if let Some(parent) = parent {
        let Some(SceneNode::Group { children, .. }) =
            mutation::find_mut(&mut candidate.scene.as_mut().unwrap().roots, parent)
        else {
            return Ok(());
        };
        children.clone()
    } else {
        candidate.scene.as_ref().unwrap().roots.clone()
    };
    for node in nodes {
        if !wanted.contains(node.id()) {
            emit(
                candidate,
                operations,
                CanvasSceneOperation::Remove {
                    id: node.id().into(),
                },
            )?;
        } else if matches!(node, SceneNode::Group { .. }) {
            prune(candidate, operations, wanted, Some(node.id()))?;
        }
    }
    Ok(())
}
/// Apply only to the private candidate and bound the independently encoded operation count.
fn emit(
    candidate: &mut Canvas,
    operations: &mut Vec<CanvasSceneOperation>,
    operation: CanvasSceneOperation,
) -> Result<(), Failure> {
    if operations.len() >= 1024 {
        return Err(Failure::new(
            ErrorCode::LimitExceeded,
            "Scene diff operation budget exceeded",
        ));
    }
    mutation::apply(candidate, &operation)?;
    operations.push(operation);
    Ok(())
}
/// Diff ordered native input declarations independently of drawing nodes.
fn sync_targets(
    candidate: &mut Canvas,
    operations: &mut Vec<CanvasSceneOperation>,
    next: &[HitTarget],
) -> Result<(), Failure> {
    let removed = candidate
        .scene
        .as_ref()
        .unwrap()
        .targets
        .iter()
        .filter(|old| !next.iter().any(|value| value.id == old.id))
        .map(|old| old.id.clone())
        .collect::<Vec<_>>();
    for id in removed {
        emit(
            candidate,
            operations,
            CanvasSceneOperation::RemoveTarget { id },
        )?;
    }
    for (index, target) in next.iter().enumerate() {
        if candidate.scene.as_ref().unwrap().targets.get(index) != Some(target) {
            emit(
                candidate,
                operations,
                CanvasSceneOperation::UpsertTarget {
                    index,
                    target: target.clone(),
                },
            )?;
        }
    }
    Ok(())
}
/// Diff the semantic forest; complete parent/target relationships are checked by host admission.
fn sync_semantics(
    candidate: &mut Canvas,
    operations: &mut Vec<CanvasSceneOperation>,
    next: &[CanvasSemanticNode],
) -> Result<(), Failure> {
    let removed = candidate
        .scene
        .as_ref()
        .unwrap()
        .semantics
        .iter()
        .filter(|old| !next.iter().any(|value| value.id == old.id))
        .map(|old| old.id.clone())
        .collect::<Vec<_>>();
    for id in removed {
        emit(
            candidate,
            operations,
            CanvasSceneOperation::RemoveSemantic { id },
        )?;
    }
    for (index, node) in next.iter().enumerate() {
        if candidate.scene.as_ref().unwrap().semantics.get(index) != Some(node) {
            emit(
                candidate,
                operations,
                CanvasSceneOperation::UpsertSemantic {
                    index,
                    node: node.clone(),
                },
            )?;
        }
    }
    Ok(())
}
/// Diff ordinary foreground controls while nested Canvas artwork uses its separate owner delta.
fn sync_overlays(
    candidate: &mut Canvas,
    operations: &mut Vec<CanvasSceneOperation>,
    next: &[CanvasOverlay],
) -> Result<(), Failure> {
    let removed = candidate
        .overlays
        .iter()
        .filter(|old| !next.iter().any(|value| value.id == old.id))
        .map(|old| old.id.clone())
        .collect::<Vec<_>>();
    for id in removed {
        emit(
            candidate,
            operations,
            CanvasSceneOperation::RemoveOverlay { id },
        )?;
    }
    for (index, overlay) in next.iter().enumerate() {
        if candidate.overlays.get(index) != Some(overlay) {
            emit(
                candidate,
                operations,
                CanvasSceneOperation::UpsertOverlay {
                    index,
                    overlay: overlay.clone(),
                },
            )?;
        }
    }
    Ok(())
}
