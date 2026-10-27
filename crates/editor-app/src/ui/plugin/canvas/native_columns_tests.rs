//! Real Windows glyphs and pixels verify the independently committed Canvas 1.4–1.6 baseline.
use super::*;
use std::{cell::RefCell, time::Duration};

/// Open one owned native window, move a divider through production input, and inspect its pixels.
/// A child process isolates GPUI's process-global platform while the HWND retires before shutdown.
#[test]
#[ignore = "opens a Windows/D3D window; run explicitly with --test-threads=1"]
fn native_canvas_text_and_columns_reach_pixels() {
    const CHILD: &str = "NANOBUG_CANVAS_COLUMNS_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let mut log = tempfile::tempfile().unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "native_canvas_text_and_columns_reach_pixels",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD, "1")
            .env("RUST_MIN_STACK", "8388608")
            .stdout(log.try_clone().unwrap())
            .stderr(log.try_clone().unwrap())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(45);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if std::time::Instant::now() > deadline {
                child.kill().unwrap();
                panic!("native column validation timed out");
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        use std::io::{Read as _, Seek as _};
        log.rewind().unwrap();
        let mut output = String::new();
        log.read_to_string(&mut output).unwrap();
        eprintln!("{output}");
        assert!(status.success(), "native column child: {status}");
        return;
    }
    let result = Rc::new(RefCell::new(None));
    let output = result.clone();
    gpui_kit::application().with_assets(gpui_kit::assets::Assets)
        .with_quit_mode(gpui_kit::QuitMode::Explicit).run(move |cx| {
            gpui_kit::init(cx); editor_ui::typography::init(cx);
            editor_ui::theme::apply_theme(editor_ui::theme::builtin_theme(false), cx);
            let events = Rc::new(RefCell::new(Vec::new())); let recorded = events.clone();
            let window = cx.open_window(gpui_kit::WindowOptions {
                window_bounds: Some(gpui_kit::WindowBounds::Windowed(Bounds::new(
                    point(px(100.), px(100.)), size(px(400.), px(160.))))),
                ..Default::default()
            }, move |window, cx| {
                let view = cx.new(|cx| CanvasView::new(drawing(), move |event, _, _| recorded.borrow_mut().push(event), window, cx));
                cx.new(|cx| gpui_base::Root::new(view, window, cx))
            }).unwrap();
            // Native drawing updates Root itself, so do not hold a typed Root update during readback.
            let window: gpui_kit::AnyWindowHandle = window.into();
            cx.spawn(async move |cx| {
                cx.background_executor().timer(Duration::from_millis(800)).await;
                let mut observed = window.update(cx, |_, window, cx| -> anyhow::Result<()> {
                    let widths = events.borrow().iter().find_map(|event| match event {
                        CanvasEvent::TextMeasured { widths } => Some(widths.clone()), _ => None
                    }).ok_or_else(|| anyhow::anyhow!("native measurement was not delivered"))?;
                    anyhow::ensure!(widths[1].width > widths[0].width * 2., "native proportional font widths: {widths:?}");
                    let before = window.render_to_image()?;
                    before.save(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/canvas-columns-before.png"))?;
                    let scale = window.scale_factor();
                    anyhow::ensure!(red_pixels(&before, scale) == 0, "first text must initially clip at its column edge");
                    let position = point(px(200.), px(20.));
                    window.dispatch_event(gpui_kit::PlatformInput::MouseDown(MouseDownEvent {
                        position, button: MouseButton::Left, modifiers: Default::default(), click_count: 1, first_mouse: false,
                    }), cx);
                    for x in [212., 224., 240.] {
                        window.dispatch_event(gpui_kit::PlatformInput::MouseMove(MouseMoveEvent {
                            position: point(px(x), px(20.)), pressed_button: Some(MouseButton::Left), modifiers: Default::default(),
                        }), cx);
                        window.draw(cx).clear(cx);
                    }
                    anyhow::ensure!(!events.borrow().iter().any(|event| matches!(event, CanvasEvent::ColumnsResized { .. })), "movement must not call the guest");
                    let after = window.render_to_image()?;
                    after.save(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/canvas-columns-after.png"))?;
                    window.dispatch_event(gpui_kit::PlatformInput::MouseUp(MouseUpEvent {
                        position: point(px(240.), px(20.)), button: MouseButton::Left, modifiers: Default::default(), click_count: 1,
                    }), cx);
                    let releases: Vec<_> = events.borrow().iter().filter_map(|event| match event {
                        CanvasEvent::ColumnsResized { epoch, widths } => Some((*epoch, widths.clone())), _ => None,
                    }).collect();
                    anyhow::ensure!(releases == vec![(1, vec![240., 160.])], "exactly one native release: {releases:?}");
                    anyhow::ensure!(red_pixels(&after, scale) > 0, "native glyphs must follow the locally enlarged clip before guest acknowledgement; before={}, after={}, scale={scale}", red_pixels(&before, scale), red_pixels(&after, scale));
                    Ok(())
                }).and_then(|result| result);
                if let Err(error) = window.update(cx, |_, window, _| window.remove_window()) {
                    if observed.is_ok() { observed = Err(error); }
                }
                cx.background_executor().timer(Duration::from_millis(150)).await;
                *output.borrow_mut() = Some(observed);
                let _ = cx.update(|cx| cx.quit());
            }).detach();
        });
    result
        .borrow_mut()
        .take()
        .expect("native readback must complete")
        .unwrap();
}

/// Text is deliberately longer than both columns; only their native clips constrain its paint.
fn drawing() -> Canvas {
    Canvas {
        column_resize: Some(protocol::ui::ColumnResize {
            viewport: None,
            epoch: 1,
            x: 0.,
            widths: vec![200., 200.],
            minimum: vec![80., 80.],
            bindings: vec![
                protocol::ui::ColumnBinding {
                    paint: 1,
                    column: 0,
                },
                protocol::ui::ColumnBinding {
                    paint: 2,
                    column: 1,
                },
            ],
        }),
        measure_text: ["iii", "WWW"]
            .into_iter()
            .enumerate()
            .map(|(id, text)| protocol::ui::TextMeasure {
                id: id.to_string(),
                texts: vec![text.into()],
                size: 16.,
                bold: false,
                font: Some("Segoe UI".into()),
            })
            .collect(),
        paint: vec![
            protocol::Paint::Fill {
                rect: protocol::Rect {
                    x: 0.,
                    y: 0.,
                    w: 400.,
                    h: 160.,
                },
                color: 0xffffff,
                extend_to_bottom: false,
            },
            text_box(4., 0xff0000),
            text_box(204., 0x0000ff),
        ],
        ..Default::default()
    }
}

/// Each label inherits the same Windows font and retains its original four-pixel inner padding.
fn text_box(x: f32, color: u32) -> protocol::Paint {
    protocol::Paint::TextBox {
        rect: protocol::Rect {
            x,
            y: 8.,
            w: 192.,
            h: 32.,
        },
        text: "W".repeat(50),
        color,
        size: 16.,
        bold: false,
        font: Some("Segoe UI".into()),
        align: protocol::ui::Alignment::Start,
    }
}

/// Count visibly red glyph coverage in a strip exposed only by the first column's resize.
fn red_pixels(image: &image::RgbaImage, scale: f32) -> usize {
    // Inspect every device pixel: integer logical samples can miss thin glyphs at fractional DPI.
    ((200. * scale).ceil() as u32..(236. * scale).floor() as u32)
        .flat_map(|x| {
            ((8. * scale).ceil() as u32..(38. * scale).floor() as u32).map(move |y| (x, y))
        })
        .filter(|(x, y)| {
            let color = image.get_pixel(*x, *y).0;
            color[0] > 180 && color[1] < 80 && color[2] < 80
        })
        .count()
}
