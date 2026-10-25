//! Real Windows drag cadence, measured from paints rather than the input producer's timer.

use gpui_kit::{
    AnyWindowHandle, AppContext as _, Bounds, Context, InteractiveElement, IntoElement,
    ParentElement, Pixels, Point, Render, Styled, Window, WindowBounds, WindowKind, canvas, div,
    point, px, size,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

mod driver;
mod surfaces;

/// GPUI paint callback, current monitor period and geometry of the moving surface.
struct Sample {
    /// Clock during the observation canvas paint, before GPU presentation.
    at: Instant,
    /// Read from this window's current display, never a machine-wide FPS constant.
    period: Duration,
    /// Native geometry, split allocation or floating title coordinate.
    position: Point<Pixels>,
    /// Background throttling cannot certify foreground drag performance.
    active: bool,
    /// Dock measurements require a real floating drag payload, rather than ordinary hover paints.
    dragging: bool,
}

/// One production surface plus an uncached observation canvas; internal drags animate themselves.
struct Probe {
    /// The fixture reuses the local dock/split controls and Base's original interaction state.
    surface: surfaces::Surface,
    /// Paint observations shared with the bounded native test.
    samples: Rc<RefCell<Vec<Sample>>>,
    /// Continuous content invalidation is used only to measure the native modal move/resize loop.
    animate: Rc<Cell<bool>>,
}

impl Render for Probe {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let samples = self.samples.clone();
        let animate = self.animate.clone();
        let geometry = self.surface.geometry.clone();
        let kind = self.surface.kind.clone();
        let content = self.surface.content(window, cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .relative()
            .bg(gpui_kit::rgb(0x203040))
            .child(
                super::editor_title_bar(cx).window_control_area(gpui_kit::WindowControlArea::Drag),
            )
            .child(div().flex_1().min_h_0().child(content))
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, cx| {
                        let position = match kind.as_str() {
                            "main" | "dialog" => window.bounds().origin,
                            "resize" | "dialog-resize" => {
                                let extent = window.viewport_size();
                                point(extent.width, extent.height)
                            }
                            "dock" => window.mouse_position(),
                            _ => {
                                let extent = geometry.get().size;
                                point(extent.width, extent.height)
                            }
                        };
                        samples.borrow_mut().push(Sample {
                            at: Instant::now(),
                            period: window
                                .refresh_interval(cx)
                                .expect("native display reports its mode"),
                            position,
                            active: window.is_window_active(),
                            dragging: cx.has_active_drag(),
                        });
                        if animate.get() {
                            window.on_next_frame(|window, _| window.refresh());
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
    }
}

/// Isolate each scenario's native platform lifetime from other tests in a bounded process.
/// `NANOBUG_DRAG_SURFACE` selects main/dialog/resize/dialog-resize/dock/dock-resize/split/split-vertical.
#[test]
#[ignore = "opens an isolated Windows/D3D window and moves the pointer; run with --test-threads=1"]
fn native_window_drag_tracks_display_refresh() {
    const CHILD: &str = "NANOBUG_DRAG_ACCEPTANCE_CHILD";
    if std::env::var_os(CHILD).is_none() {
        driver::child(CHILD);
        return;
    }
    let kind = std::env::var("NANOBUG_DRAG_SURFACE").unwrap_or_else(|_| "main".into());
    assert!(
        matches!(
            kind.as_str(),
            "main"
                | "dialog"
                | "resize"
                | "dialog-resize"
                | "dock"
                | "dock-resize"
                | "split"
                | "split-vertical"
        ),
        "unknown drag surface: {kind}"
    );
    let native = matches!(
        kind.as_str(),
        "main" | "dialog" | "resize" | "dialog-resize"
    );
    let samples = Rc::new(RefCell::new(Vec::new()));
    let output = samples.clone();
    let span = Rc::new(Cell::new(None));
    let result = span.clone();
    let fixture_kind = kind.clone();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .with_quit_mode(gpui_kit::QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            crate::typography::init(cx);
            crate::theme::apply_theme(
                crate::theme::builtin_theme(std::env::var_os("NANOBUG_DRAG_LIGHT").is_none()),
                cx,
            );
            crate::init(cx);
            let mut options = super::TitleBar::window_options();
            options.window_bounds = Some(WindowBounds::Windowed(Bounds::new(
                cx.primary_display().unwrap().visible_bounds().origin + point(px(100.), px(100.)),
                size(px(800.), px(500.)),
            )));
            let parent: Option<AnyWindowHandle> = if fixture_kind.starts_with("dialog") {
                // This is the same owned modal kind selected by the application's settings dialogs.
                let parent = cx
                    .open_window(super::TitleBar::window_options(), |_, cx| {
                        cx.new(|_| surfaces::Parent)
                    })
                    .unwrap();
                options.kind = WindowKind::Dialog;
                Some(parent.into())
            } else {
                None
            };
            let animate = Rc::new(Cell::new(native));
            let animation = animate.clone();
            let geometry = Rc::new(Cell::new(Bounds::default()));
            let measured = geometry.clone();
            let window: AnyWindowHandle = cx
                .open_window(options, |window, cx| {
                    window.activate_window();
                    let surface =
                        surfaces::Surface::new(fixture_kind.clone(), geometry, window, cx);
                    let probe = cx.new(|_| Probe {
                        surface,
                        samples,
                        animate,
                    });
                    cx.new(|cx| gpui_base::Root::new(probe, window, cx))
                })
                .unwrap()
                .into();
            cx.spawn(async move |cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(1000))
                    .await;
                let (hwnd, scale, bounds, period) = window
                    .update(cx, |_, window, cx| {
                        window.activate_window();
                        let RawWindowHandle::Win32(handle) =
                            HasWindowHandle::window_handle(window).unwrap().as_raw()
                        else {
                            panic!("native Windows handle required");
                        };
                        (
                            handle.hwnd.get(),
                            window.scale_factor(),
                            measured.get(),
                            window.refresh_interval(cx).expect("native display mode"),
                        )
                    })
                    .unwrap();
                let producer = driver::gesture(hwnd, scale, bounds, period, fixture_kind.clone());
                while !producer.is_finished() {
                    cx.background_executor()
                        .timer(Duration::from_millis(20))
                        .await;
                }
                let (start, end) = producer.join().unwrap();
                // Let the posted release pass through normal native dispatch before inspecting state.
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                span.set(Some((
                    start + Duration::from_millis(150),
                    end - Duration::from_millis(150),
                )));
                animation.set(false);
                window
                    .update(cx, |_, window, cx| {
                        assert!(
                            !cx.has_active_drag(),
                            "release must retire the real drag payload"
                        );
                        // Screenshot export is optional and does not contribute any sampled frames.
                        if let Some(path) = std::env::var_os("NANOBUG_DRAG_CAPTURE") {
                            window.render_to_image().unwrap().save(path).unwrap();
                        }
                    })
                    .unwrap();
                // Release native focus and owned windows before App drops its platform/TLS state.
                // This follows the existing native scroll acceptance cleanup, including dialogs.
                for handle in [Some(window), parent].into_iter().flatten() {
                    let _ = handle.update(cx, |_, window, cx| {
                        window.blur(cx);
                        window.refresh();
                    });
                }
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                for handle in [Some(window), parent].into_iter().flatten() {
                    let _ = handle.update(cx, |_, window, _| window.remove_window());
                }
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
    let (start, end) = result.get().expect("bounded native gesture completed");
    verify(&kind, &output.borrow(), start, end);
}

/// Assert visible progress and foreground paint cadence against the period reported in each frame.
fn verify(kind: &str, samples: &[Sample], start: Instant, end: Instant) {
    eprintln!(
        "native drag observations {kind}: total={}, active={}, changing={}",
        samples.len(),
        samples.iter().filter(|sample| sample.active).count(),
        samples
            .windows(2)
            .filter(|pair| pair[0].position != pair[1].position)
            .count()
    );
    let moving: Vec<_> = samples
        .windows(2)
        .filter(|pair| {
            pair[1].at >= start
                && pair[1].at <= end
                && pair[1].active
                && (kind != "dock" || pair[1].dragging)
                && pair[1].position != pair[0].position
        })
        .map(|pair| &pair[1])
        .collect();
    assert!(
        moving.len() > 10,
        "{kind}: the actual surface must move in foreground paints"
    );
    let seconds = moving
        .last()
        .unwrap()
        .at
        .duration_since(moving[0].at)
        .as_secs_f64();
    let fps = (moving.len() - 1) as f64 / seconds;
    let display_hz = moving
        .iter()
        .map(|sample| 1. / sample.period.as_secs_f64())
        .sum::<f64>()
        / moving.len() as f64;
    let mut intervals: Vec<_> = moving
        .windows(2)
        .map(|pair| pair[1].at.duration_since(pair[0].at).as_secs_f64() * 1000.)
        .collect();
    intervals.sort_by(f64::total_cmp);
    eprintln!(
        "native drag {kind}: moving_paints={}, fps={fps:.2}, display_hz={display_hz:.2}, p95_ms={:.3}, max_ms={:.3}",
        moving.len(),
        intervals[(intervals.len() - 1) * 95 / 100],
        intervals.last().unwrap()
    );
    assert!(
        fps >= display_hz * 0.90,
        "{kind}: actual motion {fps:.2} FPS must track the current {display_hz:.2} Hz display"
    );
}
