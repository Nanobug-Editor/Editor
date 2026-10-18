//! Capture real D3D output and independently compare its dimensions with the owned Windows client area.
//! Owned-thread identities distinguish GPUI bookkeeping from actual native-window retirement.
use anyhow::{Context as _, ensure};
use gpui_kit::{App, AsyncApp, Window, px};
use raw_window_handle::RawWindowHandle;
use std::{
    path::Path,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{GetLastError, HWND, RECT},
    UI::WindowsAndMessaging::{
        EnumThreadWindows, GW_OWNER, GetClassNameW, GetClientRect, GetForegroundWindow, GetWindow,
        GetWindowThreadProcessId, IsWindow, IsWindowVisible,
    },
};

/// Retain only a live GPUI window's own HWND for read-only observations after logical removal.
#[derive(Clone, Copy, Debug)]
pub(super) struct OwnedWindow {
    /// Borrowed from the current process's Window, never discovered by title or enumeration.
    hwnd: isize,
}

impl OwnedWindow {
    /// Capture `window`'s native identity; reject non-Windows or unavailable platform handles.
    pub fn for_window(window: &Window) -> anyhow::Result<Self> {
        let RawWindowHandle::Win32(raw) = raw_window_handle::HasWindowHandle::window_handle(window)
            .map_err(|error| anyhow::anyhow!("native handle unavailable: {error:?}"))?
            .as_raw()
        else {
            anyhow::bail!("native retirement requires this process's Win32 window");
        };
        Ok(Self {
            hwnd: raw.hwnd.get(),
        })
    }

    /// Query whether the original owned HWND still exists without sending messages or closing it.
    pub fn is_alive(self) -> bool {
        // SAFETY: IsWindow only queries the identity borrowed from this test's live Window.
        unsafe { IsWindow(self.hwnd as *mut _) != 0 }
    }

    /// Observe foreground ownership without inspecting another process's window identity or content.
    /// Only an HWND owned by this private child may appear in the receipt; other foregrounds are opaque.
    pub fn foreground_snapshot(self) -> serde_json::Value {
        // SAFETY: both calls query opaque native identities without dereferencing handles or sending input.
        let foreground = unsafe { GetForegroundWindow() };
        let mut process_id = 0;
        let available = !foreground.is_null()
            && unsafe { GetWindowThreadProcessId(foreground, &mut process_id) } != 0;
        let owned = available && process_id == std::process::id();
        serde_json::json!({
            "available": available, "owned_by_this_process": owned,
            "is_this_window": owned && foreground as isize == self.hwnd,
            "owned_foreground_hwnd": owned.then_some(foreground as isize),
        })
    }
}

/// Identify only this acceptance child's UI thread through a borrowed, live GPUI window.
#[derive(Clone, Copy)]
pub(super) struct OwnedUiThread {
    /// Native creator thread captured before the original window can be destroyed.
    thread_id: u32,
    /// Every enumerated HWND must still belong to the same private child process.
    process_id: u32,
}

impl OwnedUiThread {
    /// Capture `window`'s owning thread and reject a failed query or another process's HWND.
    pub fn for_window(window: &Window) -> anyhow::Result<Self> {
        let window = OwnedWindow::for_window(window)?;
        let mut process_id = 0;
        // SAFETY: the handle was borrowed from this child's live GPUI Window above.
        let thread_id = unsafe { GetWindowThreadProcessId(window.hwnd as *mut _, &mut process_id) };
        ensure!(
            thread_id != 0 && process_id == std::process::id(),
            "native UI thread query did not identify this acceptance child"
        );
        Ok(Self {
            thread_id,
            process_id,
        })
    }

    /// Compare all current GPUI windows with native nonchild windows belonging to this exact thread.
    /// Only read-only identity queries run; no window message, activation or close is issued.
    pub fn snapshot(self, cx: &mut App) -> serde_json::Value {
        let mut gpui_windows = Vec::new();
        for handle in cx.windows() {
            let id = format!("{:?}", handle.window_id());
            let native = handle
                .update(cx, |_, window, _| OwnedWindow::for_window(window))
                .and_then(|window| window)
                .and_then(|window| {
                    self.identity(window.hwnd).ok_or_else(|| {
                        anyhow::anyhow!("GPUI HWND no longer belongs to the captured UI thread")
                    })
                });
            gpui_windows.push(match native {
                Ok(native) => serde_json::json!({"window_id": id, "native": native}),
                Err(error) => serde_json::json!({"window_id": id, "error": format!("{error:#}")}),
            });
        }
        let mut collector = ThreadWindowCollector {
            owner: self,
            windows: Vec::new(),
            rejected: 0,
        };
        // SAFETY: enumeration is synchronous, the stack collector stays live, and the callback
        // validates both creator thread and PID before inspecting any returned window identity.
        let enumerated = unsafe {
            EnumThreadWindows(
                self.thread_id,
                Some(collect_owned_thread_window),
                &mut collector as *mut ThreadWindowCollector as isize,
            )
        };
        let enumeration_error = (enumerated == 0).then(|| unsafe { GetLastError() });
        serde_json::json!({
            "process_id": self.process_id,
            "thread_id": self.thread_id,
            "gpui_windows": gpui_windows,
            "native_nonchild_windows": collector.windows,
            "rejected_windows": collector.rejected,
            "enumeration_error": enumeration_error,
        })
    }

    /// Read an identity only while `hwnd` still belongs to the captured child process and UI thread.
    fn identity(self, hwnd: isize) -> Option<serde_json::Value> {
        let hwnd = hwnd as HWND;
        let mut process_id = 0;
        // SAFETY: these APIs only query the handle; no raw HWND memory is dereferenced.
        let thread_id = unsafe { GetWindowThreadProcessId(hwnd, &mut process_id) };
        if thread_id != self.thread_id || process_id != self.process_id {
            return None;
        }
        let mut class = [0u16; 256];
        // SAFETY: ownership was checked above and the fixed buffer covers the advertised capacity.
        let class_len = unsafe { GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32) };
        if class_len <= 0 {
            return None;
        }
        let class = String::from_utf16_lossy(&class[..class_len as usize]);
        Some(serde_json::json!({
            "hwnd": hwnd as isize,
            "class": class,
            "visible": unsafe { IsWindowVisible(hwnd) != 0 },
            "owner_hwnd": unsafe { GetWindow(hwnd, GW_OWNER) } as isize,
        }))
    }
}

/// This stack-owned callback state cannot extend a native window or GPUI entity's lifetime.
struct ThreadWindowCollector {
    /// Only this previously captured creator thread and private child PID may be inspected.
    owner: OwnedUiThread,
    /// Native identities are owned JSON values, never strong window or application handles.
    windows: Vec<serde_json::Value>,
    /// Stale, mismatched or unreadable HWNDs cannot silently become a proven empty native set.
    rejected: usize,
}

/// Collect `hwnd` only when it belongs to the stack collector's exact UI thread and child process.
unsafe extern "system" fn collect_owned_thread_window(hwnd: HWND, context: isize) -> i32 {
    // SAFETY: EnumThreadWindows invokes this callback synchronously with the live stack collector.
    let collector = unsafe { &mut *(context as *mut ThreadWindowCollector) };
    if let Some(window) = collector.owner.identity(hwnd as isize) {
        collector.windows.push(window);
    } else {
        collector.rejected += 1;
    }
    1
}

/// Require a real owned-thread `snapshot` to contain no GPUI handles or native GPUI content windows.
/// Capture/enumeration failures cannot count as empty sets; unrelated IME helper windows stay diagnostic.
pub(super) fn verify_retirement(snapshot: &serde_json::Value) -> anyhow::Result<()> {
    ensure!(
        snapshot["capture_error"].is_null(),
        "native window identity capture failed: {}",
        snapshot["capture_error"],
    );
    ensure!(
        snapshot["process_id"].as_u64() == Some(u64::from(std::process::id()))
            && snapshot["thread_id"].as_u64().is_some_and(|id| id != 0),
        "native window retirement snapshot does not identify this child's UI thread"
    );
    ensure!(
        snapshot["enumeration_error"].is_null(),
        "native UI-thread enumeration failed: {}",
        snapshot["enumeration_error"],
    );
    ensure!(
        snapshot["rejected_windows"].as_u64() == Some(0),
        "native UI-thread enumeration contains unverified window identities"
    );
    let gpui = snapshot["gpui_windows"]
        .as_array()
        .context("native retirement snapshot has no GPUI window observation")?;
    // GPUI's registered content-window class owns AccessKit; OS IME helpers use other classes.
    let native = snapshot["native_nonchild_windows"]
        .as_array()
        .context("native retirement snapshot has no native window observation")?
        .iter()
        .filter(|window| window["class"] == "Zed::Window")
        .collect::<Vec<_>>();
    ensure!(
        gpui.is_empty() && native.is_empty(),
        "native retirement retained {} GPUI handles and {} native Zed::Window identities: gpui={gpui:?}, native={native:?}",
        gpui.len(),
        native.len(),
    );
    Ok(())
}

/// Await actual destruction of `windows` while `cx` keeps pumping the native event loop.
/// A five-second deadline reports surviving owned HWNDs; no fixed delay counts as retirement.
pub(super) async fn wait_for_retirement(
    windows: &[OwnedWindow],
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let remaining = windows
            .iter()
            .copied()
            .filter(|window| window.is_alive())
            .collect::<Vec<_>>();
        if remaining.is_empty() {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "native HWND retirement timed out: original_count={} remaining_count={} remaining={remaining:?}",
            windows.len(),
            remaining.len(),
        );
        // Yield the UI thread so WindowsWindow's ordinary deferred DestroyWindow task can run.
        cx.background_executor()
            .timer(Duration::from_millis(10))
            .await;
    }
}

/// A bounded native DPI request is prepared under a Window borrow and sent only after that borrow ends.
pub(super) struct DpiRequest {
    hwnd: isize,
    dpi: u32,
    bounds: RECT,
}
impl DpiRequest {
    pub fn for_window(window: &Window, percent: u32) -> anyhow::Result<Self> {
        let RawWindowHandle::Win32(raw) = raw_window_handle::HasWindowHandle::window_handle(window)
            .map_err(|error| anyhow::anyhow!("native handle unavailable: {error:?}"))?
            .as_raw()
        else {
            anyhow::bail!("DPI acceptance requires this process's Win32 window");
        };
        let mut bounds = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        // SAFETY: raw is borrowed from this live Window, never found by title or other-process enumeration.
        ensure!(
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(
                    raw.hwnd.get() as *mut _,
                    &mut bounds,
                )
            } != 0,
            "GetWindowRect failed"
        );
        let ratio = percent as f32 / 100. / window.scale_factor();
        bounds.right = bounds.left + ((bounds.right - bounds.left) as f32 * ratio).round() as i32;
        bounds.bottom = bounds.top + ((bounds.bottom - bounds.top) as f32 * ratio).round() as i32;
        Ok(Self {
            hwnd: raw.hwnd.get(),
            dpi: 96 * percent / 100,
            bounds,
        })
    }

    pub fn apply(self) {
        // SAFETY: caller keeps the owned window alive and has released App/Window update borrows.
        // The stack RECT remains valid for the complete synchronous message, including WM_SIZE reentry.
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW(
                self.hwnd as *mut _,
                windows_sys::Win32::UI::WindowsAndMessaging::WM_DPICHANGED,
                ((self.dpi as usize) << 16) | self.dpi as usize,
                &self.bounds as *const RECT as isize,
            );
        }
    }
}

/// Read only this GPUI window's HWND; a test platform or another raw-handle kind cannot pass.
pub(super) fn capture(window: &Window, path: &Path) -> anyhow::Result<serde_json::Value> {
    let RawWindowHandle::Win32(raw) = raw_window_handle::HasWindowHandle::window_handle(window)
        .map_err(|error| anyhow::anyhow!("native window handle unavailable: {error:?}"))?
        .as_raw()
    else {
        anyhow::bail!("native Windows acceptance requires a Win32 HWND");
    };
    let hwnd = raw.hwnd.get() as *mut std::ffi::c_void;
    let mut client = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: this HWND belongs to the live window borrowed above; RECT remains writable for the call.
    ensure!(
        unsafe { GetClientRect(hwnd, &mut client) } != 0,
        "GetClientRect failed"
    );
    let client_size = (client.right - client.left, client.bottom - client.top);
    let image = window
        .render_to_image()
        .context("real D3D11 readback failed")?;
    let physical = image.dimensions();
    ensure!(
        physical.0 > 1 && physical.1 > 1,
        "renderer still has its initial 1×1 target"
    );
    ensure!(
        physical == (client_size.0 as u32, client_size.1 as u32),
        "D3D image {physical:?} differs from native client {client_size:?}"
    );
    let viewport = window.viewport_size();
    let scale = window.scale_factor();
    let logical = (viewport.width / px(1.), viewport.height / px(1.));
    ensure!(
        (logical.0 * scale - physical.0 as f32).abs() <= 1.
            && (logical.1 * scale - physical.1 as f32).abs() <= 1.,
        "logical viewport {logical:?} at {scale} differs from D3D size {physical:?}"
    );
    image.save(path)?;
    Ok(serde_json::json!({
        "renderer": "Windows D3D11 render target readback",
        "scale_factor": scale,
        "viewport_logical": [logical.0, logical.1],
        "client_physical": [client_size.0, client_size.1],
        "image_physical": [physical.0, physical.1],
        "screenshot": path.file_name().and_then(|name| name.to_str()),
        "os_ime_candidates_verified": false
    }))
}
