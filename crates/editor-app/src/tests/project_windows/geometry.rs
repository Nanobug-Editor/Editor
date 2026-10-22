//! Native project activation restores client size and centers the complete window on its monitor.

use super::*;
use gpui_kit::{Bounds, QuitMode, WindowBounds, point};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{cell::RefCell, rc::Rc, time::Duration};
use windows_sys::Win32::{
    Foundation::RECT,
    Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow},
    UI::WindowsAndMessaging::{GetClientRect, GetWindowRect},
};

/// A separate bounded process owns the real GPUI platform and keeps native TLS isolated.
#[test]
#[ignore = "requires a real Windows desktop/D3D11; opens an isolated project window"]
fn project_selection_centers_native_window() {
    const CHILD_ENV: &str = "NANOBUG_PROJECT_GEOMETRY_CHILD";
    if std::env::var_os(CHILD_ENV).is_none() {
        let output = tempfile::tempfile().unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "app::projects::tests::geometry::project_selection_centers_native_window",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD_ENV, "1")
            .env("RUST_MIN_STACK", "8388608")
            .stdout(output.try_clone().unwrap())
            .stderr(output.try_clone().unwrap())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("native project geometry test exceeded 30 seconds");
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        use std::io::{Read as _, Seek as _};
        let mut output = output;
        output.rewind().unwrap();
        let mut log = String::new();
        output.read_to_string(&mut log).unwrap();
        assert!(status.success(), "native project geometry: {status}\n{log}");
        assert!(log.contains("native project geometry verified"), "{log}");
        return;
    }

    let profile = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let project_path = project.path().canonicalize().unwrap();
    let mut session = crate::SessionState::load_in_profile(&project_path, profile.path());
    session.workspace_trusted = false;
    session.window_width = 900.;
    session.window_height = 600.;
    session.try_save().unwrap();
    let profile_path = profile.path().to_owned();
    let result = Rc::new(RefCell::new(None));
    let observed = result.clone();
    gpui_kit::application()
        .with_assets(crate::AppAssets)
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            typography::init(cx);
            apply_theme(builtin_theme(false), cx);
            cx.set_reduce_motion(true);
            crate::app::shortcuts::bootstrap::configure(cx, profile_path.join("shortcuts.json"))
                .unwrap();
            let mut options = editor_ui::window::TitleBar::window_options();
            let display = cx.primary_display().unwrap().visible_bounds();
            options.window_bounds = Some(WindowBounds::Windowed(Bounds::new(
                display.origin + point(px(40.), px(40.)),
                size(px(720.), px(480.)),
            )));
            let (handle, host) = gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| ProjectWindow::new(profile_path, window, cx))
            })
            .unwrap();
            handle
                .update(cx, |_, window, _| {
                    window.on_next_frame(move |window, cx| {
                        host.update(cx, |host, cx| {
                            host.open_path(project_path, window, cx);
                            if host.editor.is_none() {
                                *observed.borrow_mut() = Some(Err(format!(
                                    "project activation failed: {:?}",
                                    host.error
                                )));
                            }
                        });
                        window.on_next_frame(move |window, cx| {
                            // Next-frame callbacks run before drawing; paint this restored layout
                            // before comparing its viewport and reading the actual D3D11 target.
                            window.draw(cx).clear(cx);
                            if observed.borrow().is_none() {
                                *observed.borrow_mut() = Some(check_native_geometry(window));
                            }
                            host.update(cx, |host, cx| {
                                host.request_close(
                                    crate::app::projects::CloseIntent::Window,
                                    window,
                                    cx,
                                );
                            });
                            cx.spawn(async |cx| {
                                cx.background_executor()
                                    .timer(Duration::from_millis(300))
                                    .await;
                                cx.update(|cx| cx.quit());
                            })
                            .detach();
                        });
                        window.refresh();
                    });
                    window.refresh();
                })
                .unwrap();
        });
    result
        .borrow_mut()
        .take()
        .expect("native frame was not observed")
        .unwrap();
    eprintln!("native project geometry verified");
}

/// Compare the native client rectangle with GPUI's viewport as well as the outer window center.
/// A correct HWND with stale GPUI dimensions leaves unpainted bands on the right and bottom.
fn check_native_geometry(window: &gpui_kit::Window) -> Result<(), String> {
    let RawWindowHandle::Win32(handle) = HasWindowHandle::window_handle(window).unwrap().as_raw()
    else {
        return Err("expected native Win32 window".into());
    };
    let hwnd = handle.hwnd.get() as _;
    let mut outer = RECT::default();
    let mut client = RECT::default();
    let mut monitor = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // The borrowed native handle remains valid for this synchronous geometry observation.
    unsafe {
        if GetWindowRect(hwnd, &mut outer) == 0
            || GetClientRect(hwnd, &mut client) == 0
            || GetMonitorInfoW(
                MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
                &mut monitor,
            ) == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }
    let work = monitor.rcWork;
    let dx = (outer.left + outer.right - work.left - work.right).abs();
    let dy = (outer.top + outer.bottom - work.top - work.bottom).abs();
    let scale = window.scale_factor();
    let width = client.right - client.left;
    let height = client.bottom - client.top;
    let viewport = window.viewport_size();
    if (viewport.width / px(1.) * scale - width as f32).abs() > 1.
        || (viewport.height / px(1.) * scale - height as f32).abs() > 1.
    {
        return Err(format!(
            "GPUI viewport does not fill native client: viewport={viewport:?}, client={width}x{height}, scale={scale}"
        ));
    }
    if dx > 2
        || dy > 2
        || (width as f32 - 900. * scale).abs() > 1.
        || (height as f32 - 600. * scale).abs() > 1.
    {
        return Err(format!(
            "window not centered with restored size: doubled offset=({dx},{dy}), client={width}x{height}, scale={scale}"
        ));
    }
    let image = window
        .render_to_image()
        .map_err(|error| error.to_string())?;
    if image.dimensions() != (width as u32, height as u32) {
        return Err(format!(
            "D3D11 target {:?} does not fill native client {width}x{height}",
            image.dimensions()
        ));
    }
    if let Some(path) = std::env::var_os("NANOBUG_PROJECT_GEOMETRY_CAPTURE") {
        image.save(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}
