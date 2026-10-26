//! A real WASM source button drags through native input into a second owned OS receiver window.
use super::super::wasm::InstalledFlow;
use super::*;
use futures::StreamExt as _;
use gpui_kit::{App, Entity, Role, Window, px};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Mutex, atomic::AtomicBool},
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::{HWND, POINT},
    Graphics::Gdi::ClientToScreen,
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

/// One fixture's native source behavior, independent of pointer timing and OS receiver ownership.
#[derive(Clone, Copy)]
pub(in crate::ui::plugin::canvas::scene_native_tests) struct Case {
    pub cancel_before_ready: bool,
    pub canvas_source: bool,
    pub bad_image: bool,
    pub table_source: bool,
}
pub(in crate::ui::plugin::canvas::scene_native_tests) struct Probe {
    source: usize,
    destination: usize,
    start: (i32, i32),
    done: Arc<AtomicBool>,
    entered: Arc<AtomicBool>,
    dropped: Arc<Mutex<Option<Arc<drag::Offer>>>>,
    case: Case,
}
/// Use the local Button's standard native observer, not a host test-only geometry API.
pub(in crate::ui::plugin::canvas::scene_native_tests) fn prepare(
    view: &Entity<crate::ui::plugin::PluginView>,
    owner: Rc<RefCell<InstalledFlow>>,
    events: Rc<RefCell<Vec<plugin_runtime::plugin_protocol::ui::UiEvent>>>,
    case: Case,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<(Probe, gpui_kit::AnyWindowHandle, drag::Registration)> {
    let source = super::super::uia::owned_handle(window)?;
    let point = if case.canvas_source {
        let snapshot = view
            .read(cx)
            .native_canvas_input_snapshot("drawing", window, cx)
            .ok_or_else(|| anyhow::anyhow!("actual Canvas source missing"))?;
        gpui_kit::point(
            px(snapshot["origin_x"].as_f64().unwrap() as f32
                + if case.table_source { 332. } else { 30. }),
            px(snapshot["origin_y"].as_f64().unwrap() as f32
                + if case.table_source { 20. } else { 30. }),
        )
    } else {
        let button = gpui_base::test_support::snapshots(window)
            .into_iter()
            .find(|node| {
                node.visible()
                    && node.role() == Some(Role::Button)
                    && node.label() == Some("拖出节点 / Drag node out")
            })
            .ok_or_else(|| anyhow::anyhow!("actual source button missing"))?;
        button.bounds().center()
    };
    let start = (
        (point.x / px(1.) * window.scale_factor()) as i32,
        (point.y / px(1.) * window.scale_factor()) as i32,
    );
    let done = Arc::new(AtomicBool::new(false));
    let complete = done.clone();
    // This is the native actor port, with controlled transport delay and the real public Manager.
    let (sender, mut receiver) = futures::channel::mpsc::unbounded();
    cx.spawn(async move |cx| {
        let mut delivered = 0;
        while let Some(event) = receiver.next().await {
            if case.cancel_before_ready
                && matches!(
                    &event,
                    crate::ui::plugin::transfer::Ingress::StartDrag { .. }
                )
            {
                cx.background_executor()
                    .timer(Duration::from_millis(700))
                    .await;
            }
            let finish = matches!(
                &event,
                crate::ui::plugin::transfer::Ingress::FinishDrag { .. }
            );
            let fresh = {
                let events = events.borrow();
                let fresh = events[delivered..].to_vec();
                delivered = events.len();
                fresh
            };
            if let Err(error) = owner.borrow_mut().deliver(&fresh) {
                eprintln!("original native source inputs: {error:#}");
            }
            let result = owner.borrow_mut().native_source(event);
            if let Err(error) = &result {
                eprintln!("native source admission: {error:#}");
            }
            if finish && result.is_ok() {
                complete.store(true, Ordering::Release);
            }
        }
    })
    .detach();
    view.update(cx, |view, cx| {
        view.set_native_transfers(
            Rc::new(move |event| sender.unbounded_send(event).is_ok()),
            window,
            cx,
        )
    })?;
    let peer = super::super::input::deactivation::open(cx)?;
    let destination = peer.update(cx, |_, window, _| super::super::uia::owned_handle(window))??;
    // Position only these two owned HWNDs, so all physical input stays off other applications.
    unsafe {
        SetWindowPos(
            HWND(source as *mut _),
            Some(HWND_TOPMOST),
            60,
            60,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE,
        )?;
        SetWindowPos(
            HWND(destination as *mut _),
            Some(HWND_TOPMOST),
            900,
            60,
            320,
            200,
            SWP_NOACTIVATE,
        )?;
    }
    let dropped = Arc::new(Mutex::new(None));
    let capture = dropped.clone();
    let entered = Arc::new(AtomicBool::new(false));
    let receiver_entered = entered.clone();
    let registration = drag::Registration::new(destination as isize, move |event| {
        if matches!(&event, drag::Event::Enter { .. }) {
            receiver_entered.store(true, Ordering::Release);
        }
        if let drag::Event::Drop { offer, .. } = event {
            *capture.lock().unwrap() = Some(offer);
        }
        true
    })?;
    Ok((
        Probe {
            source,
            destination,
            start,
            done,
            entered,
            dropped,
            case,
        },
        peer,
        registration,
    ))
}
/// Verify ownership and actual native hit before injecting each mouse observation.
fn pointer(hwnd: usize, x: i32, y: i32, flags: MOUSE_EVENT_FLAGS) -> anyhow::Result<()> {
    let hwnd = HWND(hwnd as *mut _);
    let mut point = POINT { x, y };
    let mut process = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut process));
        anyhow::ensure!(process == std::process::id());
        ClientToScreen(hwnd, &mut point).ok()?;
        SetCursorPos(point.x, point.y)?;
        anyhow::ensure!(
            WindowFromPoint(point) == hwnd,
            "native outgoing pointer is occluded"
        );
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dwFlags: flags,
                    ..Default::default()
                },
            },
        };
        anyhow::ensure!(SendInput(&[input], std::mem::size_of::<INPUT>() as i32) == 1);
    }
    Ok(())
}
/// OS completion precedes selected receiver IO, proving both real gesture admission and delayed bytes.
pub(in crate::ui::plugin::canvas::scene_native_tests) fn run(probe: Probe) -> anyhow::Result<()> {
    pointer(
        probe.source,
        probe.start.0,
        probe.start.1,
        MOUSEEVENTF_LEFTDOWN,
    )?;
    pointer(
        probe.source,
        probe.start.0,
        probe.start.1,
        MOUSEEVENTF_LEFTUP,
    )?;
    std::thread::sleep(Duration::from_millis(60));
    pointer(
        probe.source,
        probe.start.0,
        probe.start.1,
        MOUSEEVENTF_LEFTDOWN,
    )?;
    pointer(
        probe.source,
        probe.start.0 + 8,
        probe.start.1,
        MOUSEEVENTF_MOVE,
    )?;
    std::thread::sleep(Duration::from_millis(250));
    pointer(probe.destination, 45, 35, MOUSEEVENTF_MOVE)?;
    if !probe.case.cancel_before_ready {
        // Hold this genuine gesture until OLE observes the receiver; actor scheduling has no fixed deadline.
        let entered_deadline = Instant::now() + Duration::from_secs(3);
        while !probe.entered.load(Ordering::Acquire) && Instant::now() < entered_deadline {
            std::thread::sleep(Duration::from_millis(20));
            pointer(probe.destination, 45, 35, MOUSEEVENTF_MOVE)?;
        }
    }
    std::thread::sleep(Duration::from_millis(180));
    pointer(probe.destination, 45, 35, MOUSEEVENTF_LEFTUP)?;
    if probe.case.cancel_before_ready {
        // A different, later physical gesture must never revive the old source request.
        std::thread::sleep(Duration::from_millis(70));
        pointer(probe.destination, 45, 35, MOUSEEVENTF_LEFTDOWN)?;
        std::thread::sleep(Duration::from_millis(600));
        pointer(probe.destination, 45, 35, MOUSEEVENTF_LEFTUP)?;
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while !probe.done.load(Ordering::Acquire) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    anyhow::ensure!(
        probe.done.load(Ordering::Acquire),
        "native source gesture did not complete through the actor port"
    );
    if probe.case.cancel_before_ready {
        anyhow::ensure!(
            probe.dropped.lock().unwrap().is_none(),
            "a retired source gesture exported into a later mouse gesture"
        );
        return Ok(());
    }
    let offer = probe
        .dropped
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| anyhow::anyhow!("real OS receiver missed outgoing data"))?;
    let drag::Data::Bytes(bytes) = offer.read("text/html", 256 * 1024)? else {
        anyhow::bail!("inert HTML expected")
    };
    let expected = if probe.case.table_source {
        "<table><tr><td>姓名 / Name</td><td>e\u{301} 👩‍💻</td></tr></table>".as_bytes()
    } else {
        b"<table><tr><td>Flow</td></tr></table>"
    };
    anyhow::ensure!(bytes == expected);
    if !probe.case.canvas_source {
        let image = offer.read("image/png", 256 * 1024);
        if probe.case.bad_image {
            anyhow::ensure!(
                image.is_err(),
                "native export must reject an oversized decoded PNG before returning OS data"
            );
        } else {
            let drag::Data::Bytes(bytes) = image? else {
                anyhow::bail!("PNG bytes expected")
            };
            anyhow::ensure!(
                crate::ui::plugin::bitmap::input_format(&bytes)?
                    == plugin_runtime::plugin_protocol::api::ImageFormat::Png
            );
        }
    }
    Ok(())
}
