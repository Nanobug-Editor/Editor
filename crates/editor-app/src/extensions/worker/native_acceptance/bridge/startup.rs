//! First visible Git content is observed before the shared native harness can request a refresh.
use super::*;
use plugin_runtime::plugin_protocol::{Paint, ui::Kind};

impl Harness {
    /// Permit cold WASM preparation separately, but never allow the thirty-second observer to
    /// rescue an already mounted empty list. The scene, native input owner and D3D frame agree.
    pub(super) async fn wait_history_content(
        &self,
        initial: bool,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        let started = Instant::now();
        let mut first_scene = None;
        loop {
            self.frame(self.main, cx).await?;
            let (revision, text_count, content, painted_bottom) = {
                let state = self.worker.state.lock().unwrap();
                let view = state.views.get("git/history");
                let canvas = view
                    .and_then(|view| view.active_node("log"))
                    .and_then(|node| match &node.kind {
                        Kind::Canvas(canvas) => Some(canvas),
                        _ => None,
                    });
                (
                    view.map(|view| view.revision),
                    canvas.map_or(0, |canvas| {
                        canvas.paint.iter().filter(|paint| {
                            matches!(paint, Paint::Text { text, .. } | Paint::TextBox { text, .. }
                                if !text.is_empty())
                        }).count()
                    }),
                    canvas
                        .and_then(|canvas| canvas.scroll.as_ref())
                        .map_or(0., |range| range.content),
                    canvas.map_or(0., |canvas| {
                        canvas
                            .paint
                            .iter()
                            .filter_map(|paint| match paint {
                                Paint::TextBox { rect, text, .. } if !text.is_empty() => {
                                    Some(rect.y + rect.h)
                                }
                                _ => None,
                            })
                            .fold(0_f32, f32::max)
                    }),
                )
            };
            if revision.is_some() && first_scene.is_none() {
                first_scene = Some(Instant::now());
            }
            let (current_native, native_height) = self.main.update(cx, |_, window, cx| {
                let owner = self.app.read(cx);
                owner
                    .plugin_panels
                    .get("git/history")
                    .map_or((false, 0.), |panel| {
                        let panel = panel.read(cx);
                        let revision_current = panel
                            .current_document()
                            .is_some_and(|view| Some(view.revision) == revision);
                        panel.native_ui.as_ref().map_or((false, 0.), |view| {
                            view.read(cx)
                                .native_canvas_input_snapshot("log", window, cx)
                                .map_or((false, 0.), |input| {
                                    (
                                        revision_current
                                            && input["enabled"] == true
                                            && input["scene_current"] == true
                                            && input["node_active"] == true,
                                        input["height"].as_f64().unwrap_or(0.) as f32,
                                    )
                                })
                        })
                    })
            })?;
            // A lost initial Resize can paint only the fallback's first three rows while the
            // graph fills the real dock. First content is ready only when text covers the viewport.
            let fills_viewport =
                native_height > 0. && painted_bottom + 0.5 >= content.min(native_height);
            if text_count > 0 && content > 0. && current_native && fills_viewport {
                let evidence = serde_json::json!({
                    "before_explicit_refresh": initial,
                    "first_scene_to_content_ms": first_scene.unwrap().elapsed().as_millis(),
                    "activation_observation_ms": started.elapsed().as_millis(),
                    "painted_text_count": text_count,
                    "scroll_content": content,
                    "native_scene_current": true,
                    "painted_bottom": painted_bottom,
                    "native_viewport_height": native_height,
                    "visible_rows_covered": fills_viewport,
                    "revision": revision,
                });
                eprintln!("native initial history: {evidence}");
                self.record(
                    if initial {
                        "initial_history_content"
                    } else {
                        "refreshed_history_content"
                    },
                    evidence,
                );
                self.capture(
                    self.main,
                    if initial {
                        "initial-history.png"
                    } else {
                        "refreshed-history.png"
                    },
                    cx,
                )?;
                return Ok(());
            }
            let in_budget = started.elapsed() < Duration::from_secs(90)
                && first_scene
                    .is_none_or(|first: Instant| first.elapsed() < Duration::from_secs(15));
            if !in_budget {
                self.dump_diagnostics("timeout-initial-history", cx)?;
            }
            ensure!(
                in_budget,
                "history viewport did not fill (initial={initial}, painted_bottom={painted_bottom}, native_height={native_height}): {}",
                self.diagnostic(),
            );
        }
    }
}
