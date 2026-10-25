//! Reconcile a missed backend activation using dispatch-time OS truth and one bounded native callback.
use std::{io, num::NonZeroU32, sync::OnceLock};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::Threading::GetCurrentThreadId,
        UI::{
            Input::KeyboardAndMouse::GetFocus,
            Shell::{DefSubclassProc, GetWindowSubclass, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{
                GetForegroundWindow, GetWindowThreadProcessId, IsIconic, PostMessageW,
                RegisterWindowMessageW, WA_ACTIVE, WM_ACTIVATE, WM_NCDESTROY,
            },
        },
    },
    core::w,
};

/// Registration is process-wide; no instance, target, input text or heap state is retained by the callback.
static MESSAGE: OnceLock<NonZeroU32> = OnceLock::new();

/// Reconcile activation only when `owner_hwnd` really owns foreground and keyboard focus.
/// The handle must belong to this process and calling UI thread. Returns false without mutation
/// when OS focus is elsewhere; invalid/foreign handles or native queue failures return an I/O error.
/// One queued callback per window rechecks real focus at dispatch and removes itself on completion
/// or native destruction. It never activates a window or synthesizes keyboard/mouse input.
pub fn reconcile_active_window(owner_hwnd: isize) -> io::Result<bool> {
    let hwnd = HWND(owner_hwnd as _);
    let mut process = 0;
    // SAFETY: Win32 validates the opaque handle before any callback or message is installed.
    let thread = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process)) };
    if process != std::process::id() || thread == 0 || thread != unsafe { GetCurrentThreadId() } {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "window focus requires the owning UI thread",
        ));
    }
    if !really_focused(hwnd) {
        return Ok(false);
    }
    let message = message()?;
    let id = observation as *const () as usize;
    // SAFETY: this callback belongs only to the current UI thread's verified window. Posting avoids
    // reentering the borrowed GPUI window; an existing callback coalesces repeated reader requests.
    unsafe {
        if GetWindowSubclass(hwnd, Some(observation), id, None).as_bool() {
            return Ok(true);
        }
        if !SetWindowSubclass(hwnd, Some(observation), id, 0).as_bool() {
            return Err(io::Error::other(
                "native focus observer registration failed",
            ));
        }
        if let Err(error) = PostMessageW(Some(hwnd), message, WPARAM(0), LPARAM(0)) {
            let _ = RemoveWindowSubclass(hwnd, Some(observation), id);
            return Err(io::Error::other(error));
        }
    }
    Ok(true)
}
/// Register an isolated message without reserving a WM_APP value used by GPUI or another component.
fn message() -> io::Result<u32> {
    if let Some(message) = MESSAGE.get() {
        return Ok(message.get());
    }
    // SAFETY: the static terminated name carries no pointer or user-controlled payload.
    let value = unsafe { RegisterWindowMessageW(w!("Nanobug.NativeWindowFocusObservation.v1")) };
    let value = NonZeroU32::new(value).ok_or_else(io::Error::last_os_error)?;
    let _ = MESSAGE.set(value);
    Ok(value.get())
}
/// Independently verify current OS focus both at admission and immediately before backend delivery.
fn really_focused(hwnd: HWND) -> bool {
    // SAFETY: these queries do not change focus; a destroyed or replaced owner cannot pass.
    unsafe { GetForegroundWindow() == hwnd && GetFocus() == hwnd && !IsIconic(hwnd).as_bool() }
}
/// A one-shot observer reports only current truth; a focus change while queued cannot replay activation.
unsafe extern "system" fn observation(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    _: usize,
) -> LRESULT {
    let ours = MESSAGE
        .get()
        .is_some_and(|registered| registered.get() == message);
    if ours || message == WM_NCDESTROY {
        // SAFETY: comctl32 owns callback chaining during removal, including native window teardown.
        unsafe {
            let _ = RemoveWindowSubclass(hwnd, Some(observation), id);
        }
    }
    if ours {
        if really_focused(hwnd) {
            // SAFETY: forward an observed active state only now, outside the original GPUI borrow.
            // The locked backend may have missed WM_ACTIVATE while its accessibility state was
            // borrowed. Its existing handler updates AccessKit and ordinary activation observers.
            return unsafe {
                DefSubclassProc(hwnd, WM_ACTIVATE, WPARAM(WA_ACTIVE as usize), LPARAM(0))
            };
        }
        return LRESULT(0);
    }
    // SAFETY: unrelated messages and destruction retain the original native window procedure.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
