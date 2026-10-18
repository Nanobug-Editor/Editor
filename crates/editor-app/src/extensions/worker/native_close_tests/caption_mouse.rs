//! Actual Win32 mouse input exercises caption hit testing before observing native window state.
use super::*;
use anyhow::{Context as _, ensure};
use gpui_kit::{AnyWindowHandle, AsyncApp};
use windows_sys::Win32::{
    Foundation::{POINT, RECT},
    Graphics::Gdi::ClientToScreen,
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN,
            MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE, MOUSEINPUT, SendInput, VK_LBUTTON,
        },
        WindowsAndMessaging::{
            GetCursorPos, GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId, HTCAPTION,
            HTCLOSE, HTMAXBUTTON, HTMINBUTTON, HWND_TOPMOST, IsIconic, IsZoomed, SMTO_ABORTIFHUNG,
            SW_RESTORE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SendMessageTimeoutW, SetCursorPos,
            SetForegroundWindow, SetWindowPos, ShowWindowAsync, WM_NCHITTEST, WindowFromPoint,
        },
    },
};

/// One actual caption gesture; Restore is a second click on the same maximize/restore area.
#[derive(Clone, Copy, Debug)]
pub(in crate::extensions::worker) enum CaptionButton {
    Minimize,
    Maximize,
    Restore,
    Close,
}

/// Native observations never depend on GPUI's retained bookkeeping after a caption operation.
#[derive(Clone, Copy, Debug)]
pub(in crate::extensions::worker) struct CaptionState {
    pub alive: bool,
    pub iconic: bool,
    pub zoomed: bool,
}

/// The outer process bounds a stalled UI callback and restores the original cursor even after killing its own child.
#[test]
#[ignore = "requires Windows desktop/D3D11; actual SendInput targets only private child caption HWNDs"]
fn native_caption_mouse_controls_remain_responsive() {
    let output = std::env::var_os("NANOBUG_CAPTION_MOUSE_OUTPUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "target/git-caption-mouse-red-2026-10-18".into());
    std::fs::create_dir_all(&output).unwrap();
    let mut failed = Vec::new();
    for action in ["minimize", "maximize", "close"] {
        let _cursor = CursorRestore::capture().unwrap();
        let path = output.join(format!("{action}.log"));
        let log = std::fs::File::create(&path).unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                CHILD,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("NANOBUG_CAPTION_MOUSE", action)
            .env_remove("NANOBUG_CLOSE_GIT_PACKAGE")
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .stdin(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(12);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break Some(status);
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                break None;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        eprintln!(
            "actual caption {action}: {status:?}; log={}",
            path.display()
        );
        if !status.is_some_and(|status| status.success()) {
            failed.push(action);
        }
    }
    assert!(
        failed.is_empty(),
        "actual caption mouse failed: {failed:?}; logs={}",
        output.display()
    );
}

/// Exercise the requested button without using WM_CLOSE as its operation receipt.
pub(super) async fn exercise(
    action: &str,
    target: AnyWindowHandle,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    drag_title(target, cx).await?;
    match action {
        "minimize" => {
            let observed = click(target, CaptionButton::Minimize, cx).await?;
            ensure!(observed.iconic);
            restore_minimized(target, cx).await?;
        }
        "maximize" => {
            let observed = click(target, CaptionButton::Maximize, cx).await?;
            ensure!(observed.zoomed);
            let restored = click(target, CaptionButton::Restore, cx).await?;
            ensure!(!restored.zoomed);
        }
        "close" => {
            ensure!(!click(target, CaptionButton::Close, cx).await?.alive);
        }
        _ => anyhow::bail!("unknown fixed caption action"),
    }
    Ok(())
}

/// Inject one down/up pair only after a real OS move and hit-test identify this live child HWND's caption.
pub(in crate::extensions::worker) async fn click(
    target: AnyWindowHandle,
    button: CaptionButton,
    cx: &mut AsyncApp,
) -> anyhow::Result<CaptionState> {
    let _cursor = CursorRestore::capture()?;
    let (handle, logical, screen) = target.update(cx, |_, window, cx| {
        window.draw(cx).clear(cx);
        let controls = window.window_controls();
        ensure!(!matches!(button, CaptionButton::Minimize) || controls.minimize);
        ensure!(
            !matches!(button, CaptionButton::Maximize | CaptionButton::Restore)
                || controls.maximize
        );
        // These centers follow the existing local 34px caption cells and 28px WorkWindow header.
        // Native WM_NCHITTEST below is the authority that proves the computed point is actually correct.
        let offset = match button {
            CaptionButton::Minimize => 85.,
            CaptionButton::Maximize | CaptionButton::Restore => 51.,
            CaptionButton::Close => 17.,
        };
        let logical = point(
            window.viewport_size().width - px(offset),
            px(crate::PANEL_HEADER_HEIGHT / 2.),
        );
        let scale = window.scale_factor();
        Ok::<_, anyhow::Error>((
            hwnd(window),
            logical,
            POINT {
                x: (f32::from(logical.x) * scale).round() as i32,
                y: (f32::from(logical.y) * scale).round() as i32,
            },
        ))
    })??;
    let expected = match button {
        CaptionButton::Minimize => HTMINBUTTON,
        CaptionButton::Maximize | CaptionButton::Restore => HTMAXBUTTON,
        CaptionButton::Close => HTCLOSE,
    };
    let screen = position_on_caption(target, handle, logical, screen, expected, cx).await?;
    eprintln!(
        "actual caption: {button:?} HWND={handle} screen=({}, {}) hit={expected} expected={expected}",
        screen.x, screen.y
    );
    ensure!(
        unsafe { WindowFromPoint(screen) } as isize == handle,
        "caption coordinate left the own HWND before input"
    );
    let inputs = [
        mouse_input(MOUSEEVENTF_LEFTDOWN),
        mouse_input(MOUSEEVENTF_LEFTUP),
    ];
    ensure!(
        unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            )
        } == inputs.len() as u32,
        "actual caption SendInput did not enqueue both inputs"
    );
    eprintln!("actual caption: {button:?} SendInput down/up enqueued");
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        cx.background_executor()
            .timer(Duration::from_millis(5))
            .await;
        let state = native_state(handle);
        let completed = match button {
            CaptionButton::Minimize => state.alive && state.iconic,
            CaptionButton::Maximize => state.alive && state.zoomed,
            CaptionButton::Restore => state.alive && !state.zoomed && !state.iconic,
            CaptionButton::Close => !state.alive,
        };
        if completed {
            eprintln!("actual caption: {button:?} observed {state:?}");
            return Ok(state);
        }
        ensure!(
            Instant::now() < deadline,
            "actual caption {button:?} did not change native state: {state:?}"
        );
    }
}

/// The title must remain a real OS drag target after separating it from the caption buttons.
pub(in crate::extensions::worker) async fn drag_title(
    target: AnyWindowHandle,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let _cursor = CursorRestore::capture()?;
    let (handle, logical, screen) = target.update(cx, |_, window, cx| {
        window.draw(cx).clear(cx);
        let controls = window.window_controls();
        let caption_width =
            34. * (1 + u32::from(controls.minimize) + u32::from(controls.maximize)) as f32;
        let logical = point(
            (window.viewport_size().width - px(caption_width)) / 2.,
            px(crate::PANEL_HEADER_HEIGHT / 2.),
        );
        let scale = window.scale_factor();
        (
            hwnd(window),
            logical,
            POINT {
                x: (f32::from(logical.x) * scale).round() as i32,
                y: (f32::from(logical.y) * scale).round() as i32,
            },
        )
    })?;
    let screen = position_on_caption(target, handle, logical, screen, HTCAPTION, cx).await?;
    let before = window_rect(handle)?;
    let state = native_state(handle);
    ensure!(
        state.alive && !state.iconic && !state.zoomed,
        "title drag requires a normal own window"
    );
    ensure!(
        unsafe { WindowFromPoint(screen) } as isize == handle,
        "title coordinate left the own HWND before input"
    );
    let mut movement = mouse_input(MOUSEEVENTF_MOVE);
    // Relative OS movement is intentional: no direct window-position API can satisfy this receipt.
    unsafe {
        movement.Anonymous.mi.dx = 24;
        movement.Anonymous.mi.dy = 16;
    }
    let inputs = [
        mouse_input(MOUSEEVENTF_LEFTDOWN),
        movement,
        mouse_input(MOUSEEVENTF_LEFTUP),
    ];
    ensure!(
        unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            )
        } == inputs.len() as u32,
        "title drag SendInput did not enqueue the full down/move/up batch"
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        cx.background_executor()
            .timer(Duration::from_millis(5))
            .await;
        let after = window_rect(handle)?;
        let state = native_state(handle);
        ensure!(
            state.alive && !state.iconic && !state.zoomed,
            "title drag changed native window state: {state:?}"
        );
        ensure!(
            after.right - after.left == before.right - before.left
                && after.bottom - after.top == before.bottom - before.top,
            "title drag resized the own window"
        );
        if (after.left, after.top) != (before.left, before.top) {
            eprintln!(
                "actual caption: Drag HWND={handle} hit={HTCAPTION} SendInput down/move/up observed rect=({},{},{},{}) -> ({},{},{},{})",
                before.left,
                before.top,
                before.right,
                before.bottom,
                after.left,
                after.top,
                after.right,
                after.bottom
            );
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "actual title drag did not move the own HWND"
        );
    }
}

/// Move onto the own HWND, then let native hit testing validate geometry without requiring foreground ownership.
async fn position_on_caption(
    target: AnyWindowHandle,
    handle: isize,
    logical: gpui_kit::Point<gpui_kit::Pixels>,
    mut screen: POINT,
    expected: u32,
    cx: &mut AsyncApp,
) -> anyhow::Result<POINT> {
    let mut pid = 0;
    ensure!(
        unsafe { GetWindowThreadProcessId(handle as _, &mut pid) } != 0
            && pid == std::process::id(),
        "caption HWND is not owned by this child"
    );
    // All synchronous Win32 callbacks run after Window.update has released its App/Window borrows.
    // The foreground lock may reject activation of a later private child. Raise only this owned
    // case window; NOMOVE/NOSIZE cannot satisfy the separate real drag/resize observations.
    // Its normal destruction at case cleanup also removes this temporary topmost placement.
    let raised = unsafe {
        SetWindowPos(
            handle as _,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
    } != 0;
    eprintln!(
        "actual caption setup: own HWND={handle} topmost_without_move_resize_activate={raised}"
    );
    ensure!(
        unsafe { ClientToScreen(handle as _, &mut screen) } != 0,
        "ClientToScreen failed"
    );
    let before = foreground_state(handle);
    let activated = unsafe { SetForegroundWindow(handle as _) } != 0;
    ensure!(
        unsafe { SetCursorPos(screen.x, screen.y) } != 0,
        "SetCursorPos failed"
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        cx.background_executor()
            .timer(Duration::from_millis(5))
            .await;
        let positioned = target.update(cx, |_, window, cx| {
            window.draw(cx).clear(cx);
            let actual = window.mouse_position();
            (actual.x - logical.x).abs() <= px(1.) && (actual.y - logical.y).abs() <= px(1.)
        })?;
        if positioned {
            break;
        }
        ensure!(
            Instant::now() < deadline,
            "actual caption mouse move was not delivered"
        );
    }
    let after = foreground_state(handle);
    eprintln!(
        "actual caption foreground: SetForegroundWindow={activated} before_target={} before_own_pid={} after_target={} after_own_pid={}",
        before.0, before.1, after.0, after.1
    );
    ensure!(
        unsafe { WindowFromPoint(screen) } as isize == handle,
        "caption coordinate is occluded outside the own HWND"
    );
    let packed = ((screen.x as u16 as u32) | ((screen.y as u16 as u32) << 16)) as isize;
    let mut hit = 0;
    ensure!(
        unsafe {
            SendMessageTimeoutW(
                handle as _,
                WM_NCHITTEST,
                0,
                packed,
                SMTO_ABORTIFHUNG,
                500,
                &mut hit,
            )
        } != 0,
        "native caption hit-test failed"
    );
    eprintln!(
        "actual caption hit-test: HWND={handle} screen=({}, {}) hit={hit} expected={expected}",
        screen.x, screen.y
    );
    ensure!(
        hit == expected as usize,
        "native caption hit-test disagrees with the painted control"
    );
    Ok(screen)
}

/// Foreground observations expose only whether the opaque window belongs to this process.
fn foreground_state(handle: isize) -> (bool, bool) {
    let foreground = unsafe { GetForegroundWindow() };
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(foreground, &mut pid);
    }
    (foreground as isize == handle, pid == std::process::id())
}

/// Read this already validated own HWND's native rectangle, never another application's bounds.
fn window_rect(handle: isize) -> anyhow::Result<RECT> {
    let mut rect = RECT::default();
    ensure!(
        unsafe { GetWindowRect(handle as _, &mut rect) } != 0,
        "own native window rectangle unavailable"
    );
    Ok(rect)
}

/// A minimized window has no clickable caption; restore only its own native HWND for subsequent independent checks.
pub(in crate::extensions::worker) async fn restore_minimized(
    target: AnyWindowHandle,
    cx: &mut AsyncApp,
) -> anyhow::Result<CaptionState> {
    let handle = target.update(cx, |_, window, _| hwnd(window))?;
    unsafe {
        ShowWindowAsync(handle as _, SW_RESTORE);
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        cx.background_executor()
            .timer(Duration::from_millis(5))
            .await;
        let state = native_state(handle);
        if state.alive && !state.iconic {
            return Ok(state);
        }
        ensure!(
            Instant::now() < deadline,
            "own minimized window did not restore"
        );
    }
}

/// Construct only ordinary left-button mouse input; the real cursor and OS select the delivery path.
fn mouse_input(flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dwFlags: flags,
                ..Default::default()
            },
        },
    }
}

/// IsWindow/IsIconic/IsZoomed query only an identity borrowed from this child's own Window.
fn native_state(handle: isize) -> CaptionState {
    CaptionState {
        alive: alive(handle),
        iconic: unsafe { IsIconic(handle as _) != 0 },
        zoomed: unsafe { IsZoomed(handle as _) != 0 },
    }
}

/// Parent and child both restore cursor position on return; the parent's guard also covers its timeout kill.
pub(in crate::extensions::worker) struct CursorRestore(POINT);
impl CursorRestore {
    pub(in crate::extensions::worker) fn capture() -> anyhow::Result<Self> {
        let mut point = POINT::default();
        ensure!(
            unsafe { GetCursorPos(&mut point) } != 0,
            "original cursor position unavailable"
        );
        Ok(Self(point))
    }
}
impl Drop for CursorRestore {
    fn drop(&mut self) {
        unsafe {
            // The parent also owns this guard, so killing a stalled drag child cannot strand a held button.
            if GetAsyncKeyState(VK_LBUTTON as i32) < 0 {
                let release = mouse_input(MOUSEEVENTF_LEFTUP);
                SendInput(1, &release, std::mem::size_of::<INPUT>() as i32);
            }
            SetCursorPos(self.0.x, self.0.y);
        }
    }
}
