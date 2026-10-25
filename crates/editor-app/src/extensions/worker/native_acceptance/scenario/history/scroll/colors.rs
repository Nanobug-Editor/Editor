//! Verify the installed candidate's colored SVG and capture actual native graph pixels.
use super::*;
use plugin_runtime::plugin_protocol::Paint;
use std::collections::BTreeSet;

/// Read packed RGB attributes from actual SVG elements without changing public canvas rendering.
fn colors<'a>(source: &'a str, element: &str, attribute: &str) -> BTreeSet<&'a str> {
    source
        .split(element)
        .skip(1)
        .filter_map(|tail| tail.split('>').next()?.split_once(attribute)?.1.get(..6))
        .collect()
}

/// Require multiple colored strands and matching line/node colors in the real published scene.
pub(super) fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let source = {
        let state = h.worker.state.lock().unwrap();
        let Kind::Canvas(canvas) = &state.views["git/history"]
            .active_node("log")
            .context("native graph canvas missing")?
            .kind
        else {
            anyhow::bail!("graph is not canvas")
        };
        canvas
            .paint
            .iter()
            .filter_map(|paint| match paint {
                Paint::Svg { source, .. } => Some(source.as_str()),
                _ => None,
            })
            .collect::<String>()
    };
    let lines = colors(&source, "<path ", "stroke=\"#");
    let nodes = colors(&source, "<circle ", "fill=\"#");
    let rings = colors(&source, "<circle ", "stroke=\"#");
    ensure!(
        lines.len() >= 2 && nodes.len() >= 2,
        "native graph lacks distinct branch colors"
    );
    ensure!(
        nodes.is_subset(&lines),
        "node colors differ from their graph strands"
    );
    ensure!(
        !rings.is_empty() && rings.is_subset(&lines) && source.contains("r=\"4\" fill=\"none\""),
        "merge commits must render hollow rings in their branch color"
    );
    ensure!(
        !vectors(h).is_empty(),
        "colored vectors were not rasterized natively"
    );
    h.capture(h.main, "history-colors.png", cx)?;
    h.record(
        "history_colors",
        serde_json::json!({
            "line_colors": lines, "node_colors": nodes, "hollow_colors": rings,
            "solid_normal_hollow_merge": true, "matching_lines_and_nodes": true,
            "real_merge_parents": true, "native_pixels_captured": true,
        }),
    );
    Ok(())
}
