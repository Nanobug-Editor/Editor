//! Restore editor geometry without racing an asynchronous resize against window placement.

use std::io;
use windows::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow},
    UI::WindowsAndMessaging::{
        GetClientRect, GetWindowRect, GetWindowThreadProcessId, IsIconic, IsZoomed, SWP_NOACTIVATE,
        SWP_NOZORDER, SetWindowPos,
    },
};

/// Restore `owner_hwnd`'s physical client size and center its outer rectangle on its current monitor.
/// `client_width` and `client_height` are positive device pixels; oversize windows fit the work area.
/// Maximized/minimized windows retain their state. Invalid or foreign handles and Win32 failures
/// return an I/O error; the caller must keep its own window alive throughout this synchronous call.
pub fn restore_centered_window(
    owner_hwnd: isize,
    client_width: i32,
    client_height: i32,
) -> io::Result<()> {
    if client_width <= 0 || client_height <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid window size",
        ));
    }
    let hwnd = HWND(owner_hwnd as _);
    let mut process_id = 0;
    // Win32 validates the native handle; restrict placement to this application's own windows.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
    if process_id != std::process::id() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid owner window",
        ));
    }
    unsafe {
        if IsZoomed(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return Ok(());
        }
        let mut outer = RECT::default();
        let mut client = RECT::default();
        GetWindowRect(hwnd, &mut outer).map_err(io::Error::other)?;
        GetClientRect(hwnd, &mut client).map_err(io::Error::other)?;
        let mut monitor = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        GetMonitorInfoW(
            MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
            &mut monitor,
        )
        .ok()
        .map_err(io::Error::other)?;
        let work = monitor.rcWork;
        // Preserve the native nonclient frame, including invisible resize borders, when restoring
        // the client size. One SetWindowPos prevents a later queued resize from undoing centering.
        let frame_width = outer.right - outer.left - (client.right - client.left);
        let frame_height = outer.bottom - outer.top - (client.bottom - client.top);
        let width = client_width
            .saturating_add(frame_width)
            .min(work.right - work.left);
        let height = client_height
            .saturating_add(frame_height)
            .min(work.bottom - work.top);
        SetWindowPos(
            hwnd,
            None,
            work.left + (work.right - work.left - width) / 2,
            work.top + (work.bottom - work.top - height) / 2,
            width,
            height,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )
        .map_err(io::Error::other)
    }
}
