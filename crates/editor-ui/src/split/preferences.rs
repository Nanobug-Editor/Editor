//! Project plugin-owned proportions onto the original Base state without retaining child content.
use super::*;
use gpui_kit::{AppContext as _, Entity, Window, canvas};

/// Final user gesture only; programmatic restoration and viewport resize never call this sink.
pub(super) type ResizeSink = Rc<dyn Fn(Vec<f32>, &mut Window, &mut App)>;

/// Native state and the latest declared/user preference survive ordinary owner repainting.
struct SplitState {
    /// Base owns all measured sizes and active pointer capture.
    native: Entity<gpui_base::ResizableState>,
    /// Last guest declaration distinguishes an acknowledgement from a new restore request.
    received: Vec<f32>,
    /// Latest user/guest intent survives native minimum-size compression.
    proportions: Vec<f32>,
    /// Last projected axis extent prevents repeated frame work and stale viewport callbacks.
    extent: f32,
}

/// Render `children` on the chosen axis, restoring normalized `proportions` after native measurement.
/// `id` identifies state within the owning native view; `vertical` chooses heights instead of widths.
/// `children` pair optional initial logical-pixel extents with the elements; `window`/`cx` own state.
/// Empty proportions keep initial extents. `on_resize` receives normalized final weights on release;
/// callers own persistence. Native minimum sizes apply, and no document or text state is retained.
pub fn split_container_with_layout(
    id: SharedString,
    vertical: bool,
    children: Vec<(Option<f32>, AnyElement)>,
    proportions: &[f32],
    on_resize: impl Fn(Vec<f32>, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let count = children.len();
    let memory = window.use_keyed_state(
        SharedString::from(format!("split-preferences-{id}")),
        cx,
        |_, cx| {
            let native = cx.new(|_| gpui_base::ResizableState::default());
            SplitState {
                native,
                received: Vec::new(),
                proportions: Vec::new(),
                extent: 0.,
            }
        },
    );
    memory.update(cx, |memory, _| {
        if memory.received != proportions {
            memory.received = proportions.to_vec();
            if !proportions.is_empty() && memory.proportions != proportions {
                memory.proportions = proportions.to_vec();
                memory.extent = 0.;
            }
        }
    });
    let native = memory.read(cx).native.clone();
    let captured = memory.clone();
    let sink: ResizeSink = Rc::new(move |weights, window, cx| {
        captured.update(cx, |state, _| state.proportions = weights.clone());
        on_resize(weights, window, cx);
    });
    let surface = render_split(id, vertical, children, Some((native, sink)));
    div()
        .relative()
        .size_full()
        .child(surface)
        .child(
            canvas(
                move |bounds, window, cx| {
                    let extent = f32::from(if vertical {
                        bounds.size.height
                    } else {
                        bounds.size.width
                    });
                    let state = memory.read(cx);
                    if count < 2
                        || !extent.is_finite()
                        || extent < count as f32 * 100.
                        || state.proportions.len() != count
                        || state.extent == extent
                    {
                        return;
                    }
                    let captured = memory.clone();
                    // Base's authoritative child bounds must exist before calling its public resize API.
                    window.on_next_frame(move |window, cx| {
                        let state = captured.read(cx);
                        if state.proportions.len() != count
                            || state.native.read(cx).sizes().len() != count
                            || state.extent == extent
                            || (f32::from(state.native.read(cx).container_size()) - extent).abs()
                                > 0.5
                        {
                            return;
                        }
                        let native = state.native.clone();
                        let weights = state.proportions.clone();
                        let targets = fit(&weights, extent);
                        captured.update(cx, |state, _| state.extent = extent);
                        native.update(cx, |state, cx| {
                            for (index, size) in targets.into_iter().enumerate().take(count - 1) {
                                // Base synchronizes old painted bounds even for a no-op resize.
                                // Keep earlier projections authoritative until the next frame.
                                if (f32::from(state.sizes()[index]) - size).abs() <= 0.01 {
                                    continue;
                                }
                                state.resize_panel(index, px(size), window, cx);
                            }
                        });
                        // The adapter is not a Render entity; invalidate its owner's cached frame
                        // after programmatic projection, exactly once for this measured extent.
                        window.refresh();
                    });
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        )
        .into_any_element()
}

/// Respect Base's minimum while preserving relative spare space in constrained viewports.
fn fit(weights: &[f32], extent: f32) -> Vec<f32> {
    let mut sizes = weights
        .iter()
        .map(|weight| (weight * extent).max(100.))
        .collect::<Vec<_>>();
    let excess = (sizes.iter().sum::<f32>() - extent).max(0.);
    let spare = sizes.iter().map(|v| v - 100.).sum::<f32>();
    if spare > 0. {
        for size in &mut sizes {
            *size -= (*size - 100.) * (excess / spare).min(1.);
        }
    }
    sizes
}
