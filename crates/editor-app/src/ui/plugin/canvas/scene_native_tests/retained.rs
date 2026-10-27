//! Real native paint observes retained paragraph work, placement and typography invalidation.
use super::*;

/// The real production renderer persists through this sequence exactly as its worker does.
#[derive(Default)]
pub(super) struct Evidence {
    renderer: super::super::super::images::VectorRenderer,
}

/// Paint twice, change a sibling, translate, then change size/face and the actual native DPI.
pub(super) fn phase(
    view: &Entity<PluginView>,
    evidence: &mut Evidence,
    phase: u8,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<()> {
    if phase != 1 {
        let (mut document, environment) = {
            let view = view.read(cx);
            (view.document.clone(), view.environment.clone())
        };
        if phase > 0 {
            document.revision += 1;
            let Kind::Canvas(canvas) = &mut document.root.kind else {
                unreachable!()
            };
            let scene = canvas.scene.as_mut().unwrap();
            scene.revision += 1;
            let ui::SceneNode::Group {
                transform,
                children,
                ..
            } = &mut scene.roots[0]
            else {
                unreachable!()
            };
            if phase == 2 {
                let ui::SceneNode::Shape {
                    geometry: ui::SceneGeometry::Rect { rect, .. },
                    ..
                } = &mut children[1]
                else {
                    unreachable!()
                };
                rect.w = 27.;
            }
            if phase == 3 {
                transform.0[4] = 24.;
            }
            if phase == 4 || phase == 5 {
                let ui::SceneNode::Paragraph { style, .. } = &mut children[0] else {
                    unreachable!()
                };
                if phase == 4 {
                    style.font.size_px = Some(21.);
                } else {
                    style.font.family = Some("Consolas".into());
                }
            }
        }
        document.validate().map_err(anyhow::Error::msg)?;
        evidence.renderer.set_environment(environment.clone());
        let images = evidence.renderer.prepare(&BTreeMap::from([(
            "ordinary/diagram".into(),
            Arc::new(document.clone()),
        )]));
        view.update(cx, |view, cx| {
            view.update_document(document, environment, window, cx);
            view.update_images("ordinary/diagram", &images, window, cx);
        });
    } else {
        window.refresh();
    }
    window.draw(cx).clear(cx);
    let count = view.read(cx).canvases["drawing"]
        .read(cx)
        .scene_graphics
        .as_ref()
        .unwrap()
        .statistics()
        .paragraph_layouts;
    if phase == 3 {
        translated_geometry(window)?;
    }
    let expected = match phase {
        0..=3 => 1,
        4 => 2,
        5 => 3,
        _ => 4,
    };
    anyhow::ensure!(
        count == expected,
        "retained native paragraph shapes: phase {phase}, expected {expected}, observed {count}"
    );
    Ok(())
}

/// The same public paragraph placement moves native point/caret/selection and visible line geometry.
fn translated_geometry(window: &Window) -> anyhow::Result<()> {
    let mut paragraph = editor_ui::text::paragraph::CanvasParagraph::new(
        "abc".into(),
        font("Segoe UI"),
        px(18.),
        rgb(0).into(),
        Bounds::new(point(px(5.), px(10.)), size(px(120.), px(50.))),
        &Default::default(),
        window,
    )
    .map_err(anyhow::Error::msg)?;
    let caret = paragraph.caret_bounds(0, true).unwrap();
    let selection = paragraph
        .selection_rects(0..1)
        .map_err(anyhow::Error::msg)?;
    let lines = paragraph.visible_lines().map_err(anyhow::Error::msg)?;
    let environment = paragraph.font_environment();
    paragraph.set_origin(point(px(25.), px(40.)));
    let moved = paragraph.caret_bounds(0, true).unwrap();
    anyhow::ensure!(
        moved.origin == caret.origin + point(px(20.), px(30.)),
        "caret must move with retained glyphs"
    );
    anyhow::ensure!(
        paragraph.byte_for_point(moved.origin) == 0,
        "native point mapping must use translated text origin"
    );
    let moved_selection = paragraph
        .selection_rects(0..1)
        .map_err(anyhow::Error::msg)?;
    anyhow::ensure!(
        moved_selection.len() == selection.len()
            && moved_selection
                .iter()
                .zip(&selection)
                .all(|(a, b)| a.origin == b.origin + point(px(20.), px(30.))),
        "selection fragments must move with retained glyphs"
    );
    let moved_lines = paragraph.visible_lines().map_err(anyhow::Error::msg)?;
    anyhow::ensure!(
        moved_lines[0].rect.x == lines[0].rect.x + 20.
            && moved_lines[0].rect.y == lines[0].rect.y + 30.,
        "visible line geometry must move with retained glyphs"
    );
    anyhow::ensure!(
        paragraph.font_environment() == environment,
        "translation cannot invent a new font environment"
    );
    Ok(())
}
