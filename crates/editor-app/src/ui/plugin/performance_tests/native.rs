//! Windows acceptance reuses the native input driver and observes real long-plugin paints.
use super::*;
use gpui_kit::{
    AnyWindowHandle, Bounds, InteractiveElement, IntoElement, ParentElement, Pixels, Point, Styled,
    WindowBounds, canvas, div, point,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::cell::Cell;

// Share the bounded OS gesture implementation without a production test API.
#[path = "../../../../../editor-ui/src/window/tests/driver.rs"]
mod driver;

/// Foreground paint time and native position, independent of the input producer's timer.
struct Sample {
    at: Instant,
    position: Point<Pixels>,
    period: Duration,
    active: bool,
}

/// Actual PluginView content and a paint observation canvas under the production title bar.
struct Probe {
    view: Entity<PluginView>,
    samples: Rc<RefCell<Vec<Sample>>>,
    animate: Rc<Cell<bool>>,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let samples = self.samples.clone();
        let animate = self.animate.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .relative()
            .child(
                editor_ui::window::editor_title_bar(cx)
                    .window_control_area(gpui_kit::WindowControlArea::Drag),
            )
            .child(div().flex_1().min_h_0().child(self.view.clone()))
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, cx| {
                        samples.borrow_mut().push(Sample {
                            at: Instant::now(),
                            position: window.bounds().origin,
                            period: window.refresh_interval(cx).unwrap(),
                            active: window.is_window_active(),
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

/// Exercise the OS caption modal loop with a real portable UI tree and foreground frame observations.
/// `NANOBUG_DRAG_DARK` selects dark appearance; `NANOBUG_DRAG_CAPTURE` saves the final native image.
#[test]
#[ignore = "opens an isolated Windows/D3D window and moves the pointer; run with --test-threads=1"]
fn long_plugin_window_drag_tracks_display_refresh() {
    const CHILD: &str = "NANOBUG_PLUGIN_DRAG_CHILD";
    if std::env::var_os(CHILD).is_none() {
        driver::child(
            CHILD,
            "ui::plugin::performance_tests::native::long_plugin_window_drag_tracks_display_refresh",
        );
        return;
    }
    let samples = Rc::new(RefCell::new(Vec::new()));
    let output = samples.clone();
    let span = Rc::new(Cell::new(None));
    let result = span.clone();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .with_quit_mode(gpui_kit::QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            editor_ui::typography::init(cx);
            crate::ui::theme::apply_theme(
                crate::ui::theme::builtin_theme(std::env::var_os("NANOBUG_DRAG_DARK").is_some()),
                cx,
            );
            editor_ui::init(cx);
            let mut options = editor_ui::window::TitleBar::window_options();
            options.window_bounds = Some(WindowBounds::Windowed(Bounds::new(
                cx.primary_display().unwrap().visible_bounds().origin + point(px(100.), px(100.)),
                size(px(1000.), px(800.)),
            )));
            let animate = Rc::new(Cell::new(true));
            let animation = animate.clone();
            let window: AnyWindowHandle = cx
                .open_window(options, |window, cx| {
                    window.activate_window();
                    let view = cx.new(|cx| {
                        PluginView::new(
                            "ordinary-provider".into(),
                            long_document(),
                            Environment::default(),
                            |_, _| {},
                            window,
                            cx,
                        )
                    });
                    let probe = cx.new(|_| Probe {
                        view,
                        samples,
                        animate,
                    });
                    cx.new(|cx| Root::new(probe, window, cx))
                })
                .unwrap()
                .into();
            cx.spawn(async move |cx| {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let (hwnd, scale, bounds, period) = window
                    .update(cx, |_, window, cx| {
                        window.activate_window();
                        let RawWindowHandle::Win32(handle) =
                            HasWindowHandle::window_handle(window).unwrap().as_raw()
                        else {
                            panic!("native Windows handle required")
                        };
                        (
                            handle.hwnd.get(),
                            window.scale_factor(),
                            window.bounds(),
                            window.refresh_interval(cx).unwrap(),
                        )
                    })
                    .unwrap();
                let producer = driver::gesture(hwnd, scale, bounds, period, "main".into());
                while !producer.is_finished() {
                    cx.background_executor()
                        .timer(Duration::from_millis(20))
                        .await;
                }
                let (start, end) = producer.join().unwrap();
                span.set(Some((
                    start + Duration::from_millis(150),
                    end - Duration::from_millis(150),
                )));
                animation.set(false);
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                window
                    .update(cx, |_, window, cx| {
                        if let Some(path) = std::env::var_os("NANOBUG_DRAG_CAPTURE") {
                            window.render_to_image().unwrap().save(path).unwrap();
                        }
                        window.blur(cx);
                        window.refresh();
                    })
                    .unwrap();
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                window
                    .update(cx, |_, window, _| window.remove_window())
                    .unwrap();
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
    let (start, end) = result.get().expect("bounded native gesture completed");
    let samples = output.borrow();
    eprintln!(
        "native plugin observations: total={}, active={}, changing={}",
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
                && pair[1].position != pair[0].position
        })
        .map(|pair| &pair[1])
        .collect();
    assert!(
        moving.len() > 1,
        "the actual plugin window must move in foreground paints"
    );
    let seconds = moving
        .last()
        .unwrap()
        .at
        .duration_since(moving[0].at)
        .as_secs_f64();
    let fps = (moving.len() - 1) as f64 / seconds;
    let hz = moving
        .iter()
        .map(|sample| 1. / sample.period.as_secs_f64())
        .sum::<f64>()
        / moving.len() as f64;
    eprintln!(
        "native long-plugin drag: fps={fps:.2}, display_hz={hz:.2}, moving_paints={}",
        moving.len()
    );
    assert!(
        fps >= hz * 0.90,
        "long plugin drag {fps:.2} FPS must track {hz:.2} Hz"
    );
}
