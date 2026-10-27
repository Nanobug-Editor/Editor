//! Real native paint observes retained paragraph work, placement and typography invalidation.
use super::*;

/// The real production renderer persists through this sequence exactly as its worker does.
#[derive(Default)]
pub(super) struct Evidence {
    renderer: super::super::super::images::VectorRenderer,
}

/// Paint twice, change a sibling, translate, then change size/face, native DPI and the active theme.
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
    if phase == 6 {
        theme_changes(view, evidence, window, cx)?;
    }
    Ok(())
}

/// Switch the real theme on `view` through its ordinary host environment and `evidence` renderer.
/// `window` supplies actual glyph pixels; `cx` applies the palette without changing the public scene.
fn theme_changes(
    view: &Entity<PluginView>,
    evidence: &mut Evidence,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<()> {
    let (document, original_environment) = {
        let view = view.read(cx);
        (view.document.clone(), view.environment.clone())
    };
    let documents = BTreeMap::from([("ordinary/diagram".into(), Arc::new(document.clone()))]);
    for (dark, expected_layouts) in [(false, 5), (true, 6), (true, 6), (false, 7)] {
        editor_ui::theme::apply_theme(editor_ui::theme::builtin_theme(dark), cx);
        // Use the same host projection as installed plugins; applying only global native colors
        // would leave the explicit worker/Canvas environment unchanged and miss this dependency.
        let environment = crate::extensions::environment(std::path::Path::new(""), cx);
        let foreground = environment.foreground;
        anyhow::ensure!(
            environment.dark == dark,
            "host theme projection did not switch"
        );
        evidence.renderer.set_environment(environment.clone());
        let images = evidence.renderer.prepare(&documents);
        view.update(cx, |view, cx| {
            view.update_document(document.clone(), environment, window, cx);
            view.update_images("ordinary/diagram", &images, window, cx);
        });
        window.draw(cx).clear(cx);
        theme_glyph_pixels(view, foreground, window, cx)?;
        let current = view.read(cx);
        anyhow::ensure!(
            current.document == document,
            "a theme change must preserve the scene identity, revision and original text"
        );
        let actual = current.canvases["drawing"]
            .read(cx)
            .scene_graphics
            .as_ref()
            .unwrap()
            .statistics()
            .paragraph_layouts;
        anyhow::ensure!(
            actual == expected_layouts,
            "theme {dark}: expected {expected_layouts} native layouts, observed {actual}"
        );
        eprintln!(
            "retained theme: dark={dark}, unchanged public scene, glyph color=#{foreground:06x}, layouts={actual}"
        );
    }
    // Keep the original fixture's final black-pixel and presentation-revision assertions intact.
    editor_ui::theme::apply_theme(editor_ui::theme::builtin_theme(false), cx);
    evidence
        .renderer
        .set_environment(original_environment.clone());
    let images = evidence.renderer.prepare(&documents);
    view.update(cx, |view, cx| {
        view.update_document(document, original_environment, window, cx);
        view.update_images("ordinary/diagram", &images, window, cx);
    });
    window.draw(cx).clear(cx);
    Ok(())
}

/// Read only the known paragraph's interior in `window`; unrelated theme surfaces cannot satisfy it.
/// `view` supplies its actual origin and `foreground` is the ordinary host's resolved RGB glyph color.
fn theme_glyph_pixels(
    view: &Entity<PluginView>,
    foreground: u32,
    window: &mut Window,
    cx: &App,
) -> anyhow::Result<()> {
    let snapshot = view
        .read(cx)
        .native_canvas_input_snapshot("drawing", window, cx)
        .expect("native paragraph Canvas geometry");
    let scale = f64::from(window.scale_factor());
    // Earlier phases translate the parent by 24 pixels; the paragraph itself starts at (20,20).
    // This rectangle excludes the colored sibling at y=130 and contains no controls or borders.
    let x = ((snapshot["origin_x"].as_f64().unwrap() + 44.) * scale) as u32;
    let y = ((snapshot["origin_y"].as_f64().unwrap() + 20.) * scale) as u32;
    let image = window.render_to_image()?;
    let expected = [
        (foreground >> 16) as u8,
        (foreground >> 8) as u8,
        foreground as u8,
    ];
    let mut ink = 0;
    for row in y..(y + (80. * scale) as u32).min(image.height()) {
        for column in x..(x + (180. * scale) as u32).min(image.width()) {
            let actual = image.get_pixel(column, row).0;
            ink += usize::from(
                actual[..3]
                    .iter()
                    .zip(expected)
                    .all(|(actual, expected)| actual.abs_diff(expected) <= 3),
            );
        }
    }
    anyhow::ensure!(
        ink >= 24,
        "retained paragraph did not repaint theme glyph color #{foreground:06x}: {ink} interior pixels"
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
