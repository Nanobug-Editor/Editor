//! Bounded DPI changes and D3D readback inspect only this acceptance process's own live HWND.

use anyhow::{Context as _, ensure};
use gpui_kit::Window;
use raw_window_handle::RawWindowHandle;
use std::path::Path;
use windows_sys::Win32::{
    Foundation::RECT,
    UI::WindowsAndMessaging::{GetClientRect, GetWindowRect, SendMessageW, WM_DPICHANGED},
};

/// A prepared DPI notification must be sent after releasing the GPUI Window borrow.
pub(super) struct DpiRequest {
    /// Borrowed from this test-owned live GPUI window.
    hwnd: isize,
    /// Windows uses 96 DPI at 100 percent scaling.
    dpi: u32,
    /// Native suggested bounds keep the current logical viewport extent.
    bounds: RECT,
}

impl DpiRequest {
    /// Prepare `percent` scaling without changing global display settings or other applications.
    pub fn new(window: &Window, percent: u32) -> anyhow::Result<Self> {
        let hwnd = hwnd(window)?;
        let mut bounds = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        // SAFETY: hwnd was borrowed from this application's live Window and RECT is writable.
        ensure!(
            unsafe { GetWindowRect(hwnd as *mut _, &mut bounds) } != 0,
            "GetWindowRect failed"
        );
        let ratio = percent as f32 / 100. / window.scale_factor();
        bounds.right = bounds.left + ((bounds.right - bounds.left) as f32 * ratio).round() as i32;
        bounds.bottom = bounds.top + ((bounds.bottom - bounds.top) as f32 * ratio).round() as i32;
        Ok(Self {
            hwnd,
            dpi: 96 * percent / 100,
            bounds,
        })
    }

    /// Apply only after the caller releases its App/Window update borrow to permit WM_SIZE reentry.
    pub fn apply(self) {
        // SAFETY: caller retains the owned window and this stack RECT for the synchronous message.
        unsafe {
            SendMessageW(
                self.hwnd as *mut _,
                WM_DPICHANGED,
                ((self.dpi as usize) << 16) | self.dpi as usize,
                &self.bounds as *const RECT as isize,
            );
        }
    }
}

/// Require an actual Win32 window identity, which a TestPlatform surface cannot supply.
fn hwnd(window: &Window) -> anyhow::Result<isize> {
    match raw_window_handle::HasWindowHandle::window_handle(window)
        .map_err(|error| anyhow::anyhow!("native handle: {error:?}"))?
        .as_raw()
    {
        RawWindowHandle::Win32(raw) => Ok(raw.hwnd.get()),
        _ => anyhow::bail!("native acceptance requires a real Windows HWND"),
    }
}

/// Save `window`'s real render target and reject stale renderer dimensions against the native client.
pub(super) fn capture(window: &Window, path: &Path) -> anyhow::Result<()> {
    let hwnd = hwnd(window)?;
    let mut client = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: hwnd belongs to the borrowed live window; the call only reads native geometry.
    ensure!(
        unsafe { GetClientRect(hwnd as *mut _, &mut client) } != 0,
        "GetClientRect failed"
    );
    let image = window.render_to_image().context("D3D11 readback failed")?;
    let dimensions = image.dimensions();
    ensure!(
        dimensions.0 > 1 && dimensions.1 > 1,
        "initial renderer target is not a screenshot"
    );
    ensure!(
        dimensions
            == (
                (client.right - client.left) as u32,
                (client.bottom - client.top) as u32
            ),
        "D3D target does not match native client bounds"
    );
    let viewport = window.viewport_size();
    let logical = (
        viewport.width / gpui_kit::px(1.),
        viewport.height / gpui_kit::px(1.),
    );
    let scale = window.scale_factor();
    ensure!(
        (logical.0 * scale - dimensions.0 as f32).abs() <= 1.
            && (logical.1 * scale - dimensions.1 as f32).abs() <= 1.,
        "logical viewport and actual DPI do not match the D3D target"
    );
    image.save(path)?;
    std::fs::write(
        path.with_extension("json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "renderer": "Windows D3D11 render target readback", "scale_factor": scale,
            "viewport_logical": [logical.0, logical.1], "client_physical": dimensions,
            "image": path.file_name().and_then(|name| name.to_str()),
        }))?,
    )?;
    Ok(())
}
