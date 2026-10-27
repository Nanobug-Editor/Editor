//! Native input and process-memory observations are restricted to this acceptance process.
use windows::Win32::{
    Foundation::{HWND, POINT},
    Graphics::Gdi::ClientToScreen,
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

/// Verify ownership for every operation; an unrelated foreground window never receives fixture input.
fn owned(handle: usize) -> anyhow::Result<HWND> {
    let hwnd = HWND(handle as *mut _);
    let mut process = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut process));
    }
    anyhow::ensure!(
        process == std::process::id(),
        "timeline native window ownership changed"
    );
    Ok(hwnd)
}

/// Keep an already restored owned window above unrelated apps; never change its visibility state.
pub(super) fn foreground(handle: usize) -> anyhow::Result<()> {
    let hwnd = owned(handle)?;
    unsafe {
        anyhow::ensure!(
            IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool(),
            "GPUI activation did not restore the actual native window"
        );
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )?;
        let _ = SetForegroundWindow(hwnd);
        if std::env::var_os("NANOBUG_CANVAS_FRAME_TRACE").is_some() {
            eprintln!(
                "[canvas-native-window] restored OS_visible={} minimized={}",
                IsWindowVisible(hwnd).as_bool(),
                IsIconic(hwnd).as_bool()
            );
        }
    }
    Ok(())
}

/// Send real vertical/horizontal wheel messages only after ownership, foreground and hit-test checks.
pub(super) fn wheel(handle: usize, horizontal: bool, delta: i32) -> anyhow::Result<()> {
    let hwnd = owned(handle)?;
    let mut point = POINT { x: 300, y: 140 };
    unsafe {
        anyhow::ensure!(
            GetForegroundWindow() == hwnd,
            "timeline input window lost foreground"
        );
        ClientToScreen(hwnd, &mut point).ok()?;
        SetCursorPos(point.x, point.y)?;
        anyhow::ensure!(
            WindowFromPoint(point) == hwnd,
            "timeline input window is occluded"
        );
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    mouseData: delta as u32,
                    dwFlags: if horizontal {
                        MOUSEEVENTF_HWHEEL
                    } else {
                        MOUSEEVENTF_WHEEL
                    },
                    ..Default::default()
                },
            },
        };
        anyhow::ensure!(
            SendInput(&[input], std::mem::size_of::<INPUT>() as i32) == 1,
            "timeline OS wheel was not admitted"
        );
    }
    Ok(())
}

/// Official PROCESS_MEMORY_COUNTERS_EX layout; querying the current pseudo-handle needs no grants.
#[repr(C)]
#[derive(Default)]
struct ProcessMemory {
    cb: u32,
    faults: u32,
    peak_working_set: usize,
    working_set: usize,
    quota_peak_paged: usize,
    quota_paged: usize,
    quota_peak_nonpaged: usize,
    quota_nonpaged: usize,
    pagefile: usize,
    peak_pagefile: usize,
    private: usize,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn K32GetProcessMemoryInfo(process: isize, counters: *mut ProcessMemory, bytes: u32) -> i32;
}

/// Return actual process RSS and private commit, including host/WASM/GPU-driver CPU allocations.
pub(super) fn memory() -> anyhow::Result<(u64, u64)> {
    let mut counters = ProcessMemory {
        cb: std::mem::size_of::<ProcessMemory>() as u32,
        ..Default::default()
    };
    anyhow::ensure!(
        unsafe { K32GetProcessMemoryInfo(-1, &mut counters, counters.cb) } != 0,
        "GetProcessMemoryInfo failed"
    );
    Ok((counters.working_set as u64, counters.private as u64))
}
