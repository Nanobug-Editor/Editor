//! Bounded native mouse producer; it never drives paint callbacks or reports its timer as FPS.

use super::*;
use std::io::{Read as _, Seek as _};
use windows::Win32::{
    Foundation::{HWND, LPARAM, POINT, RECT, WPARAM},
    Graphics::Gdi::ClientToScreen,
    UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEINPUT,
        SendInput,
    },
    UI::WindowsAndMessaging::{
        GetCursorPos, GetWindowRect, PostMessageW, SetCursorPos, WM_LBUTTONUP,
    },
};

/// Bound a single native test process; forward diagnostics and fail if it cannot cleanly exit.
pub(super) fn child(marker: &str) {
    let mut log = tempfile::tempfile().unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "window::tests::native_window_drag_tracks_display_refresh",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(marker, "1")
        .env("RUST_MIN_STACK", "8388608")
        .stdout(log.try_clone().unwrap())
        .stderr(log.try_clone().unwrap())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(25);
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            timed_out = true;
            break child.wait().unwrap();
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    log.rewind().unwrap();
    let mut output = String::new();
    log.read_to_string(&mut output).unwrap();
    eprintln!("{output}");
    assert!(!timed_out, "native drag acceptance exceeded 25 seconds");
    assert!(status.success(), "native drag child: {status}");
}

/// Save and restore the cursor even when a test assertion unwinds the producer thread.
struct Cursor(POINT, bool);
impl Drop for Cursor {
    fn drop(&mut self) {
        unsafe {
            if self.1 {
                button(false);
            }
            let _ = SetCursorPos(self.0.x, self.0.y);
        }
    }
}

/// Drive the real OS modal loop or the production control's ordinary mouse handler.
/// `hwnd` must remain alive until the returned thread ends; `bounds` uses logical client pixels.
/// `period` is this window's display interval, so input does not impose a fixed high-refresh limit.
pub(super) fn gesture(
    hwnd: isize,
    scale: f32,
    bounds: Bounds<Pixels>,
    period: Duration,
    kind: String,
) -> std::thread::JoinHandle<(Instant, Instant)> {
    std::thread::spawn(move || unsafe {
        let hwnd = HWND(hwnd as _);
        let mut old_cursor = POINT::default();
        GetCursorPos(&mut old_cursor).unwrap();
        let mut cursor = Cursor(old_cursor, false);
        let native = matches!(
            kind.as_str(),
            "main" | "dialog" | "resize" | "dialog-resize"
        );
        let resize = kind.ends_with("resize") && native;
        let mut outer = RECT::default();
        GetWindowRect(hwnd, &mut outer).unwrap();
        let mut origin = POINT::default();
        ClientToScreen(hwnd, &mut origin).ok().unwrap();
        let logical = match kind.as_str() {
            "dock" => point(bounds.center().x, bounds.top() - px(14.)),
            "dock-resize" | "split" => point(bounds.right() - px(1.), bounds.center().y),
            "split-vertical" => point(bounds.center().x, bounds.bottom() - px(1.)),
            _ => point(px(100.), px(16.)),
        };
        let client_x = (logical.x / px(1.) * scale).round() as i32;
        let client_y = (logical.y / px(1.) * scale).round() as i32;
        let (screen_x, screen_y) = if native {
            if resize {
                (outer.right - 2, outer.bottom - 2)
            } else {
                (outer.left + 100, outer.top + 16)
            }
        } else {
            (origin.x + client_x, origin.y + client_y)
        };
        SetCursorPos(screen_x, screen_y).unwrap();
        // Keep physical button state consistent with pointer movement for internal handles too.
        // Posted moves with a physically released button can be interrupted by native hover input.
        button(true);
        cursor.1 = true;
        let start = Instant::now();
        let deadline = start + Duration::from_secs(2);
        let input_period = period / 2;
        let mut last = (client_x, client_y);
        let mut step = 0;
        while Instant::now() < deadline {
            std::thread::sleep(input_period);
            step += 1;
            // Every input changes at least one physical pixel. A bounded triangular path avoids
            // pane minimum/maximum constraints turning a valid high-rate gesture into a plateau.
            let travel = 120 - (step % 240 - 120_i32).abs();
            let dx = if kind == "split-vertical" { 0 } else { travel };
            let dy = if native || kind == "split-vertical" {
                travel
            } else {
                0
            };
            last = (client_x + dx, client_y + dy);
            SetCursorPos(screen_x + dx, screen_y + dy).unwrap();
        }
        let end = Instant::now();
        button(false);
        cursor.1 = false;
        PostMessageW(
            Some(hwnd),
            WM_LBUTTONUP,
            WPARAM(0),
            coordinates(last.0, last.1),
        )
        .unwrap();
        (start, end)
    })
}

/// Win32 mouse messages pack signed physical client coordinates into two 16-bit words.
fn coordinates(x: i32, y: i32) -> LPARAM {
    LPARAM(((x as u16 as u32) | ((y as u16 as u32) << 16)) as isize)
}

/// Supply the physical left-button state required by the real Win32 modal drag loop.
fn button(down: bool) {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dwFlags: if down {
                    MOUSEEVENTF_LEFTDOWN
                } else {
                    MOUSEEVENTF_LEFTUP
                },
                ..Default::default()
            },
        },
    };
    assert_eq!(
        unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) },
        1
    );
}
