//! Exercise native DPI messages only on the isolated test process's own HWND.
use super::*;
use raw_window_handle::RawWindowHandle;
use windows_sys::Win32::{
    Foundation::RECT,
    UI::WindowsAndMessaging::{
        GetWindowRect, GetWindowThreadProcessId, IsWindow, SendMessageW, WM_DPICHANGED,
    },
};

/// Capture address and scale while the GPUI owner is live; no foreign-window lookup is performed.
pub(super) struct NativeDpi {
    hwnd: usize,
    bounds: RECT,
    scale: f32,
}
impl NativeDpi {
    /// Resolve `window`'s own Win32 handle and reject a stale or foreign process before testing it.
    pub fn capture(window: &Window) -> anyhow::Result<Self> {
        let RawWindowHandle::Win32(raw) = raw_window_handle::HasWindowHandle::window_handle(window)
            .map_err(|error| anyhow::anyhow!("native canvas handle unavailable: {error:?}"))?
            .as_raw()
        else {
            anyhow::bail!("native canvas acceptance requires Win32");
        };
        let hwnd = raw.hwnd.get() as *mut std::ffi::c_void;
        let mut process = 0;
        let mut bounds = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        // SAFETY: the borrowed window owns this HWND; both outputs are initialized writable locals.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut process);
            anyhow::ensure!(
                process == std::process::id() && GetWindowRect(hwnd, &mut bounds) != 0,
                "native canvas HWND ownership changed"
            );
        }
        Ok(Self {
            hwnd: hwnd as usize,
            bounds,
            scale: window.scale_factor(),
        })
    }
    /// Send a bounded WM_DPICHANGED from a producer thread; OS display settings remain unchanged.
    pub fn apply(self, percent: u32) -> anyhow::Result<()> {
        anyhow::ensure!(
            (75..=400).contains(&percent),
            "DPI fixture exceeds its allowed range"
        );
        let hwnd = self.hwnd as *mut std::ffi::c_void;
        let mut process = 0;
        let ratio = percent as f32 / 100. / self.scale;
        let suggested = RECT {
            right: self.bounds.left
                + ((self.bounds.right - self.bounds.left) as f32 * ratio).round() as i32,
            bottom: self.bounds.top
                + ((self.bounds.bottom - self.bounds.top) as f32 * ratio).round() as i32,
            ..self.bounds
        };
        let dpi = percent as usize * 96 / 100;
        // SAFETY: ownership is rechecked and the suggested rectangle outlives synchronous delivery.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut process);
            anyhow::ensure!(
                process == std::process::id() && IsWindow(hwnd) != 0,
                "native canvas HWND retired before DPI input"
            );
            SendMessageW(
                hwnd,
                WM_DPICHANGED,
                dpi | (dpi << 16),
                &suggested as *const RECT as isize,
            );
        }
        Ok(())
    }
}
