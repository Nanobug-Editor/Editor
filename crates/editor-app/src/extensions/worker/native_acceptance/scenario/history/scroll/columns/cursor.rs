//! Actual OS cursor observation is confined to the native case's own transient window.
use super::*;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::{
    Foundation::POINT,
    Graphics::Gdi::ClientToScreen,
    UI::WindowsAndMessaging::{
        GetCursor, GetWindowThreadProcessId, HWND_TOPMOST, IDC_SIZEWE, LoadCursorW, SWP_NOACTIVATE,
        SWP_NOMOVE, SWP_NOSIZE, SetCursorPos, SetForegroundWindow, SetWindowPos, WindowFromPoint,
    },
};

/// Move onto the given canvas-local divider using actual Windows input, then inspect its cursor.
/// `origin` and `y` locate the canvas in its own window; the parent restores the user's cursor.
pub(super) async fn exercise(
    h: &Harness,
    origin: gpui_kit::Pixels,
    y: gpui_kit::Pixels,
    boundary: f32,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let (handle, mut screen) = h.main.update(cx, |_, window, _| -> anyhow::Result<_> {
        let RawWindowHandle::Win32(raw) = window
            .window_handle()
            .map_err(|error| anyhow::anyhow!("native cursor window: {error:?}"))?
            .as_raw()
        else {
            anyhow::bail!("resize cursor probe requires its own Win32 window");
        };
        let scale = window.scale_factor();
        Ok((
            raw.hwnd.get(),
            POINT {
                x: ((origin / px(1.) + boundary) * scale).round() as i32,
                y: (y / px(1.) * scale).round() as i32,
            },
        ))
    })??;
    let mut pid = 0;
    ensure!(
        unsafe { GetWindowThreadProcessId(handle as _, &mut pid) } != 0
            && pid == std::process::id(),
        "cursor target is not owned by this native case"
    );
    // Foreground locks can reject activation. Raise only this owned window, without moving or
    // resizing it; normal case cleanup destroys it. Run synchronous callbacks outside Window.update.
    ensure!(
        unsafe {
            SetWindowPos(
                handle as _,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        } != 0,
        "owned cursor window could not be raised"
    );
    unsafe {
        SetForegroundWindow(handle as _);
    }
    ensure!(
        unsafe { ClientToScreen(handle as _, &mut screen) } != 0,
        "cursor conversion failed"
    );
    ensure!(
        unsafe { WindowFromPoint(screen) } as isize == handle,
        "cursor target is occluded"
    );
    ensure!(
        unsafe { SetCursorPos(screen.x, screen.y) } != 0,
        "native cursor move failed"
    );
    let mut shifted = false;
    h.until(
        h.main,
        "native horizontal resize cursor",
        |window, cx| {
            ensure!(
                unsafe { WindowFromPoint(screen) } as isize == handle,
                "cursor target became occluded"
            );
            let expected = unsafe { LoadCursorW(std::ptr::null_mut(), IDC_SIZEWE) };
            ensure!(!expected.is_null(), "system resize cursor unavailable");
            if unsafe { GetCursor() } == expected {
                let snapshot =
                    native_canvas(h, window, cx).context("native column canvas missing")?;
                ensure!(
                    snapshot["column_hover"] == true,
                    "OS cursor was not over the native divider"
                );
                h.record(
                    "column_cursor_probe",
                    serde_json::json!({"horizontal_resize_cursor":true,
                "own_hwnd_hit":true, "hover":true}),
                );
                return Ok(true);
            }
            // A later real movement emits WM_SETCURSOR after GPUI's cursor-style message. Alternate
            // once per native frame so Windows cannot coalesce an immediate out-and-back into no motion.
            shifted = !shifted;
            unsafe {
                SetCursorPos(screen.x + i32::from(shifted), screen.y);
            }
            Ok(false)
        },
        cx,
    )
    .await
}
