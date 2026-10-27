//! Real installed Chinese IME receives OS keyboard input only while this acceptance HWND owns focus.
use super::*;
use windows::Win32::{
    Foundation::HWND,
    UI::{
        Input::{Ime::*, KeyboardAndMouse::*},
        WindowsAndMessaging::{
            GetForegroundWindow, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
            SetCursorPos, SetForegroundWindow, SetWindowPos, WindowFromPoint,
        },
    },
};

/// Actual native composition is observed through public text/IMM APIs, never inferred from key events.
#[derive(Default)]
pub(super) struct Evidence {
    handle: usize,
    preedit: Option<std::ops::Range<usize>>,
    candidate: Option<(i32, i32)>,
}
/// Borrow only the live window's own context and always release it on the owning UI thread.
fn context<R>(handle: usize, read: impl FnOnce(HIMC) -> anyhow::Result<R>) -> anyhow::Result<R> {
    // SAFETY: the caller captures a live owned HWND; this executes on its native UI thread.
    unsafe {
        let hwnd = HWND(handle as *mut std::ffi::c_void);
        let context = ImmGetContext(hwnd);
        anyhow::ensure!(!context.0.is_null(), "own HWND has no native IME context");
        let result = read(context);
        let _ = ImmReleaseContext(hwnd, context);
        result
    }
}
/// Inject ordinary virtual-key press/releases only after foreground and native focus both match.
fn keys(handle: usize, keys: &[u16]) -> anyhow::Result<()> {
    let hwnd = HWND(handle as *mut std::ffi::c_void);
    // SAFETY: reject an occluded/foreign focus before SendInput; every input contains initialized data.
    unsafe {
        anyhow::ensure!(
            GetForegroundWindow() == hwnd && GetFocus() == hwnd,
            "OS keyboard input lost the owned HWND: expected={hwnd:?}, foreground={:?}, focus={:?}",
            GetForegroundWindow(),
            GetFocus()
        );
        let input = keys
            .iter()
            .flat_map(|key| {
                [KEYBD_EVENT_FLAGS(0), KEYEVENTF_KEYUP].map(|flags| INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VIRTUAL_KEY(*key),
                            dwFlags: flags,
                            ..Default::default()
                        },
                    },
                })
            })
            .collect::<Vec<_>>();
        anyhow::ensure!(
            SendInput(&input, std::mem::size_of::<INPUT>() as i32) == input.len() as u32,
            "OS input injection failed"
        );
    }
    Ok(())
}
/// A real, ownership-checked mouse click activates this acceptance window under Windows foreground rules.
fn activate(handle: usize) -> anyhow::Result<()> {
    use windows::Win32::{Foundation::POINT, Graphics::Gdi::ClientToScreen};
    let hwnd = HWND(handle as *mut std::ffi::c_void);
    let mut point = POINT { x: 45, y: 45 };
    // SAFETY: only the live captured own HWND is raised; verify the final native hit before clicking.
    unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )?;
        anyhow::ensure!(
            ClientToScreen(hwnd, &mut point).as_bool(),
            "own client coordinate unavailable"
        );
        SetCursorPos(point.x, point.y)?;
        anyhow::ensure!(
            WindowFromPoint(point) == hwnd,
            "own test window is occluded at activation point"
        );
        let input = [MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP].map(|flags| INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dwFlags: flags,
                    ..Default::default()
                },
            },
        });
        anyhow::ensure!(
            SendInput(&input, std::mem::size_of::<INPUT>() as i32) == 2,
            "own activation click failed"
        );
    }
    Ok(())
}
/// Prepare only this acceptance UI thread's already installed Chinese input method and own HWND.
pub(super) fn prepare_owned(handle: usize) -> anyhow::Result<()> {
    // SAFETY: input layout is thread local; no installation, registry or global language change.
    unsafe {
        let mut layouts = vec![HKL::default(); GetKeyboardLayoutList(None).max(0) as usize];
        GetKeyboardLayoutList(Some(&mut layouts));
        let layout = layouts
            .iter()
            .find(|layout| (layout.0 as usize & 0xffff) == 0x0804)
            .copied()
            .ok_or_else(|| {
                anyhow::anyhow!("No installed Simplified Chinese keyboard layout: {layouts:?}")
            })?;
        ActivateKeyboardLayout(layout, ACTIVATE_KEYBOARD_LAYOUT_FLAGS(0))?;
        let _ = SetForegroundWindow(HWND(handle as *mut std::ffi::c_void));
        let mut description = [0u16; 256];
        let length = ImmGetDescriptionW(layout, Some(&mut description));
        eprintln!(
            "actual installed Chinese IME layout={layout:?}, description={}",
            String::from_utf16_lossy(&description[..length as usize])
        );
    }
    activate(handle)
}
/// Enter actual Chinese preedit only after the owned foreground/focus checks in the keyboard adapter.
pub(super) fn start_owned(handle: usize) -> anyhow::Result<()> {
    context(handle, |context| {
        // SAFETY: this borrowed context belongs to the live acceptance HWND.
        unsafe {
            let _ = ImmSetOpenStatus(context, true);
            let _ = ImmSetConversionStatus(context, IME_CMODE_NATIVE, IME_SENTENCE_MODE(0));
        }
        Ok(())
    })?;
    keys(handle, &"ZHONGWEN".encode_utf16().collect::<Vec<_>>())
}
/// Commit the installed input method's current candidate through an ordinary OS Space key.
pub(super) fn commit_owned(handle: usize) -> anyhow::Result<()> {
    keys(handle, &[0x20])
}
/// Start actual Chinese preedit, compare the OS candidate position, then commit through the installed IME.
pub(super) fn phase(
    view: &Entity<PluginView>,
    evidence: &mut Evidence,
    phase: u8,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<()> {
    match phase {
        0 => {
            super::super::targets::native_text_tests::phase(
                view,
                &[],
                &mut Default::default(),
                0,
                window,
                cx,
            )?;
            evidence.handle = super::uia::owned_handle(window)?;
        }
        1 => {
            prepare_owned(evidence.handle)?;
        }
        2 => {
            start_owned(evidence.handle)?;
        }
        3 => {
            evidence.candidate = Some(candidate(view, evidence, window, cx)?);
            super::super::targets::native_text_tests::redraw(
                view,
                |document| {
                    document.root.viewport = Some(protocol::ui::VisualViewport {
                        content: Some(protocol::ui::ContentSize {
                            width: 320.,
                            height: 240.,
                        }),
                        transform: Some(protocol::ui::ContentTransform {
                            scale: 1.25,
                            x: 20.,
                            y: 10.,
                            anchor_x: 0.,
                            anchor_y: 0.,
                        }),
                    });
                },
                window,
                cx,
            )?;
        }
        4 => {
            let moved = candidate(view, evidence, window, cx)?;
            anyhow::ensure!(
                Some(moved) != evidence.candidate,
                "OS candidate did not move after viewport zoom/pan"
            );
            keys(evidence.handle, &[0x20])?;
        }
        5 => {
            let input = super::super::targets::native_text_tests::endpoint(view, cx);
            input.update(cx, |input, cx| {
                anyhow::ensure!(
                    input.marked_text_range(window, cx).is_none(),
                    "OS commit must end preedit"
                );
                Ok(())
            })?;
        }
        _ => unreachable!("bounded OS IME acceptance phases"),
    }
    Ok(())
}
/// GPUI's single-line IME policy anchors the current marked line's start, rather than the caret end.
fn candidate(
    view: &Entity<PluginView>,
    evidence: &mut Evidence,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<(i32, i32)> {
    let input = super::super::targets::native_text_tests::endpoint(view, cx);
    let bounds = input
        .update(cx, |input, cx| {
            let marked = input.marked_text_range(window, cx)?;
            if let Some(before) = &evidence.preedit {
                assert_eq!(*before, marked, "redraw must preserve real OS marked range");
            }
            evidence.preedit = Some(marked.clone());
            input.bounds_for_range(marked.start..marked.start, Bounds::default(), window, cx)
        })
        .ok_or_else(|| anyhow::anyhow!("OS Chinese keys did not create a native marked line"))?;
    context(evidence.handle, |context| {
        let mut candidate = CANDIDATEFORM::default();
        // SAFETY: initialized output is scoped to the HWND's borrowed native context.
        unsafe {
            anyhow::ensure!(
                ImmGetCandidateWindow(context, 0, &mut candidate).as_bool(),
                "OS candidate placement unavailable"
            );
        }
        let expected = point(bounds.origin.x, bounds.origin.y + bounds.size.height / 2.);
        let scale = window.scale_factor();
        anyhow::ensure!(
            (candidate.ptCurrentPos.x as f32 - expected.x / px(1.) * scale).abs() <= 2.
                && (candidate.ptCurrentPos.y as f32 - expected.y / px(1.) * scale).abs() <= 2.,
            "OS candidate must follow the actual marked line and DPI once: actual=({},{}), logical={expected:?}, scale={scale}",
            candidate.ptCurrentPos.x,
            candidate.ptCurrentPos.y
        );
        eprintln!(
            "actual OS Chinese marked {:?}, candidate=({},{}), DPI={scale}",
            evidence.preedit, candidate.ptCurrentPos.x, candidate.ptCurrentPos.y
        );
        Ok((candidate.ptCurrentPos.x, candidate.ptCurrentPos.y))
    })
}
/// One independent Chinese commit, no legacy duplicate, uses the original emoji byte range.
pub(super) fn verify(events: &[ui::UiEvent]) -> anyhow::Result<()> {
    let edits = events
        .iter()
        .filter_map(|event| match &event.action {
            Action::Canvas(CanvasEvent::Input { input }) => {
                if let protocol::ui::CanvasInputKind::Text(edit) = &input.kind {
                    Some(edit)
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    anyhow::ensure!(
        edits.len() == 1
            && edits[0].range == protocol::ui::CanvasTextRange { start: 4, end: 15 }
            && edits[0]
                .text
                .chars()
                .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)),
        "real OS Chinese commit missing/duplicated: {edits:?}"
    );
    eprintln!("actual OS Chinese committed text: {:?}", edits[0].text);
    Ok(())
}
