//! The production worker reports actual retained preparation work across scene revisions.
use super::*;
use plugin_runtime::plugin_protocol::ui::{Canvas, Kind, Node};

/// Two independent drawing items share a parent whose declaration changes with one child.
fn document(revision: u64, x: f32) -> BTreeMap<String, Arc<Document>> {
    let scene=serde_json::from_value(serde_json::json!({"identity":41,"revision":revision,"roots":[{
        "kind":"group","id":"events","children":[
        {"kind":"shape","id":"moving","geometry":{"kind":"rect","rect":{"x":x,"y":0,"w":20,"h":20}},"fill":{"kind":"solid","rgba":4294902015_u32}},
        {"kind":"shape","id":"stable","geometry":{"kind":"rect","rect":{"x":40,"y":0,"w":20,"h":20}},"fill":{"kind":"solid","rgba":16711935_u32}}]}]})).unwrap();
    BTreeMap::from([(
        "timeline/view".into(),
        Arc::new(
            Document::new(Node::new(
                "plot",
                Kind::Canvas(Canvas {
                    scene: Some(scene),
                    ..Default::default()
                }),
            ))
            .revision(revision),
        ),
    )])
}

/// Old published frames own pixels until they retire; diagnostics cannot hide them behind current-only totals.
#[test]
fn retained_canvas_texture_residency_observes_older_live_presentation_snapshots() {
    let scene=serde_json::from_value(serde_json::json!({"identity":71,"revision":1,"roots":[{
        "kind":"group","id":"layer","opacity":0.5,"children":[{"kind":"shape","id":"red",
        "geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":40,"h":40}},"fill":{"kind":"solid","rgba":4278190335_u32}}]}]})).unwrap();
    let documents = BTreeMap::from([(
        "timeline/view".into(),
        Arc::new(
            Document::new(Node::new(
                "plot",
                Kind::Canvas(Canvas {
                    scene: Some(scene),
                    ..Default::default()
                }),
            ))
            .revision(1),
        ),
    )]);
    let mut renderer = VectorRenderer::default();
    let old = renderer.prepare(&documents);
    let empty = renderer.prepare(&BTreeMap::new());
    assert_eq!(empty.statistics().resident_bytes, 0);
    assert!(
        empty.statistics().live_resident_bytes > 0,
        "an older published snapshot still owns its derived pixels"
    );
    assert_eq!(empty.statistics().live_textures, 1);
    drop(old);
    assert_eq!(
        empty.statistics().live_resident_bytes,
        0,
        "weak diagnostics cannot keep pixels alive after the final presentation retires"
    );
    let retired = renderer.prepare(&BTreeMap::new());
    assert_eq!(retired.statistics().live_textures, 0);
}

/// An isolated compositing layer is a retained resource even when an independent sibling changes.
#[test]
fn retained_canvas_isolated_layer_reuses_pixels_across_an_unrelated_item_change() {
    let documents = |revision, x| {
        let scene=serde_json::from_value(serde_json::json!({"identity":61,"revision":revision,"roots":[
            {"kind":"group","id":"layer","opacity":0.5,"children":[{"kind":"shape","id":"red",
            "geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":40,"h":40}},"fill":{"kind":"solid","rgba":4278190335_u32}}]},
            {"kind":"shape","id":"moving","geometry":{"kind":"rect","rect":{"x":x,"y":50,"w":20,"h":20}},"fill":{"kind":"solid","rgba":16711935_u32}}
        ]})).unwrap();
        BTreeMap::from([(
            "timeline/view".into(),
            Arc::new(
                Document::new(Node::new(
                    "plot",
                    Kind::Canvas(Canvas {
                        scene: Some(scene),
                        ..Default::default()
                    }),
                ))
                .revision(revision),
            ),
        )])
    };
    let mut renderer = VectorRenderer::default();
    let before = renderer.prepare(&documents(1, 0.));
    let after = renderer.prepare(&documents(2, 7.));
    assert_eq!(before.statistics.rasterizations, 1);
    assert_eq!(
        after.statistics.rasterizations, 1,
        "an unchanged isolated layer must keep its actual pixels"
    );
    let first = before.graphics["timeline/view/canvas/plot"]
        .drawing
        .as_ref()
        .unwrap()
        .images();
    let second = after.graphics["timeline/view/canvas/plot"]
        .drawing
        .as_ref()
        .unwrap()
        .images();
    assert!(Arc::ptr_eq(&first[0], &second[0]));
}

/// Pure artwork revisions do not flatten an unchanged native input shape again.
#[test]
fn retained_canvas_target_geometry_reuses_only_unchanged_hit_dependencies() {
    let with_target = |revision, x, target_x| {
        let mut documents = document(revision, x);
        let Kind::Canvas(canvas) = &mut Arc::make_mut(documents.get_mut("timeline/view").unwrap())
            .root
            .kind
        else {
            unreachable!()
        };
        canvas.scene.as_mut().unwrap().targets=vec![serde_json::from_value(serde_json::json!({
            "id":"select","epoch":1,"geometry":{"kind":"rect","rect":{"x":target_x,"y":0,"w":20,"h":20}}
        })).unwrap()];
        documents
    };
    let mut renderer = VectorRenderer::default();
    assert_eq!(
        renderer
            .prepare(&with_target(1, 0., 0.))
            .statistics
            .target_preparations,
        1
    );
    let moved_art = renderer.prepare(&with_target(2, 7., 0.));
    assert_eq!(moved_art.statistics.target_preparations, 1);
    assert_eq!(moved_art.statistics.target_reuses, 1);
    let moved_target = renderer.prepare(&with_target(3, 7., 9.));
    assert_eq!(
        moved_target.statistics.target_preparations, 2,
        "changed hit geometry must be prepared before display"
    );
}

/// A local delta prepares one changed item, keeps the other, and retires both with their owner.
#[test]
fn retained_canvas_local_change_reuses_unchanged_items_and_retires_owned_cache() {
    let mut renderer = VectorRenderer::default();
    let first = renderer.prepare(&document(1, 0.));
    assert_eq!(first.statistics.prepared_items, 2);
    let second = renderer.prepare(&document(2, 7.));
    assert_eq!(
        second.statistics.prepared_items, 3,
        "one replacement must not rebuild its unchanged sibling"
    );
    assert_eq!(second.statistics.reused_items, 1);
    assert_eq!(second.statistics.cached_items, 2);
    assert_eq!(second.statistics.rasterizations, 0);
    let idle = renderer.prepare(&document(2, 7.));
    assert_eq!(
        idle.statistics.prepared_items, 3,
        "idle worker polls perform no tessellation"
    );
    let retired = renderer.prepare(&BTreeMap::new());
    assert_eq!(retired.statistics.cached_items, 0);
    assert_eq!(retired.statistics.resident_bytes, 0);
}
