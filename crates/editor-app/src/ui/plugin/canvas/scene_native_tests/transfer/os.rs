//! Exercise RegisterDragDrop and DoDragDrop with real Windows pointer messages confined to our HWND.
use super::*;
use gpui_kit::Window;
use plugin_runtime::plugin_protocol as protocol;
use std::{
    sync::{Mutex, atomic::AtomicBool},
    time::Duration,
};
use windows::Win32::{
    Foundation::{HWND, POINT},
    Graphics::Gdi::ClientToScreen,
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

/// Sendable observations cross back from OLE callbacks without borrowing GPUI during OS dispatch.
pub(in crate::ui::plugin::canvas::scene_native_tests) struct Probe {
    hwnd: usize,
    events: Arc<Mutex<Vec<&'static str>>>,
    dropped: Arc<Mutex<Option<Arc<drag::Offer>>>>,
    count: Arc<AtomicUsize>,
    ingress: Option<std::sync::mpsc::Receiver<crate::ui::plugin::transfer::Ingress>>,
    canvas: bool,
    blocked: bool,
}

/// Register only the live acceptance window on its owning UI thread.
pub(in crate::ui::plugin::canvas::scene_native_tests) fn prepare(
    window: &Window,
) -> anyhow::Result<(drag::Registration, Probe)> {
    let hwnd = super::super::uia::owned_handle(window)?;
    let events = Arc::new(Mutex::new(Vec::new()));
    let dropped = Arc::new(Mutex::new(None));
    let observed = events.clone();
    let result = dropped.clone();
    let registration = drag::Registration::new(hwnd as isize, move |event| {
        let name = match event {
            drag::Event::Enter { .. } => "enter",
            drag::Event::Move { .. } => "move",
            drag::Event::Leave => "leave",
            drag::Event::Drop { offer, .. } => {
                *result.lock().unwrap() = Some(offer);
                "drop"
            }
        };
        observed.lock().unwrap().push(name);
        true
    })?;
    Ok((
        registration,
        Probe {
            hwnd,
            events,
            dropped,
            count: Arc::new(AtomicUsize::new(0)),
            ingress: None,
            canvas: false,
            blocked: false,
        },
    ))
}

/// Configure the production plugin-view port; real native layout must route this ordinary declared node.
pub(in crate::ui::plugin::canvas::scene_native_tests) fn prepare_node(
    view: &gpui_kit::Entity<crate::ui::plugin::PluginView>,
    canvas: bool,
    blocked: bool,
    window: &mut Window,
    cx: &mut gpui_kit::App,
) -> anyhow::Result<Probe> {
    let (sender, receiver) = std::sync::mpsc::channel();
    view.update(cx, |view, cx| {
        view.set_native_transfers(
            std::rc::Rc::new(move |event| sender.send(event).is_ok()),
            window,
            cx,
        )
    })?;
    Ok(Probe {
        hwnd: super::super::uia::owned_handle(window)?,
        events: Default::default(),
        dropped: Default::default(),
        count: Arc::new(AtomicUsize::new(0)),
        ingress: Some(receiver),
        canvas,
        blocked,
    })
}

/// Validate ownership and native hit testing before each physical pointer injection.
pub(in crate::ui::plugin::canvas::scene_native_tests) fn pointer(
    hwnd: usize,
    x: i32,
    y: i32,
    flags: MOUSE_EVENT_FLAGS,
) -> anyhow::Result<()> {
    let hwnd = HWND(hwnd as *mut _);
    let mut process = 0;
    let mut point = POINT { x, y };
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut process));
        anyhow::ensure!(process == std::process::id(), "drag input owner changed");
        ClientToScreen(hwnd, &mut point).ok()?;
        SetCursorPos(point.x, point.y)?;
        anyhow::ensure!(
            WindowFromPoint(point) == hwnd,
            "drag acceptance window is occluded"
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

/// Source runs on its own STA so GPUI keeps painting and receiving its native target callbacks.
pub(in crate::ui::plugin::canvas::scene_native_tests) fn run(probe: Probe) -> anyhow::Result<()> {
    let hwnd = HWND(probe.hwnd as *mut _);
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
    }
    pointer(probe.hwnd, 45, 45, MOUSEEVENTF_LEFTDOWN)?;
    let count = probe.count.clone();
    let alive = Arc::new(AtomicBool::new(true));
    let source_owner = probe.hwnd as isize;
    let source =
        std::thread::spawn(move || drag::start(source_owner, Arc::new(Producer(count)), alive));
    std::thread::sleep(Duration::from_millis(300));
    pointer(probe.hwnd, 85, 55, MOUSEEVENTF_MOVE)?;
    std::thread::sleep(Duration::from_millis(200));
    pointer(probe.hwnd, 85, 55, MOUSEEVENTF_LEFTUP)?;
    let outcome = source.join().unwrap()?;
    if probe.blocked {
        anyhow::ensure!(
            outcome == drag::Outcome::Cancelled,
            "a foreground native control must occlude its underlying Canvas receiver"
        );
        anyhow::ensure!(probe.ingress.unwrap().try_iter().next().is_none());
        anyhow::ensure!(probe.count.load(Ordering::Acquire) == 0);
        return Ok(());
    }
    anyhow::ensure!(
        outcome == drag::Outcome::Dropped,
        "OS must report a real successful drop"
    );
    if let Some(receiver) = probe.ingress {
        use crate::ui::plugin::transfer::Ingress;
        let mut phases = Vec::new();
        for event in receiver.try_iter() {
            match event {
                Ingress::Enter {
                    address, position, ..
                } => {
                    if probe.canvas {
                        let anchor = address
                            .canvas
                            .ok_or_else(|| anyhow::anyhow!("native Canvas address missing"))?;
                        anyhow::ensure!(
                            address.node == "drawing"
                                && anchor.identity == 62
                                && anchor.scene_revision == 1
                                && anchor.target.id == "receiver"
                                && anchor.target.epoch == 4
                        );
                        anyhow::ensure!(
                            position.x >= 20.
                                && position.y >= 10.
                                && position.x < 220.
                                && position.y < 110.
                        );
                    } else {
                        anyhow::ensure!(address.node == "drop-zone" && address.canvas.is_none());
                        anyhow::ensure!(position.x > 0. && position.y > 0. && position.y < 80.);
                    }
                    phases.push(protocol::transfer::Phase::Enter);
                }
                Ingress::Update { phase, .. } => phases.push(phase),
                Ingress::StartDrag { .. } | Ingress::FinishDrag { .. } => {
                    anyhow::bail!("incoming drag must not start a plugin source")
                }
            }
        }
        anyhow::ensure!(
            phases.first() == Some(&protocol::transfer::Phase::Enter)
                && phases.contains(&protocol::transfer::Phase::Move)
                && phases.last() == Some(&protocol::transfer::Phase::Drop),
            "ordinary native drop lifecycle: {phases:?}"
        );
        anyhow::ensure!(
            probe.count.load(Ordering::Acquire) == 0,
            "native node routing must remain metadata-only"
        );
        return Ok(());
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while probe.dropped.lock().unwrap().is_none() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let events = probe.events.lock().unwrap().clone();
    anyhow::ensure!(
        events.first() == Some(&"enter")
            && events.contains(&"move")
            && events.last() == Some(&"drop"),
        "real native lifecycle: {events:?}"
    );
    anyhow::ensure!(
        probe.count.load(Ordering::Acquire) == 0,
        "OS motion/drop must not render payloads"
    );
    let offer = probe
        .dropped
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| anyhow::anyhow!("OS drop missing"))?;
    let drag::Data::Bytes(bytes) = offer.read("text/html", 256 * 1024)? else {
        anyhow::bail!("HTML expected")
    };
    eprintln!("actual OLE transfer callbacks: {events:?}");
    anyhow::ensure!(bytes.starts_with(b"<table>") && probe.count.load(Ordering::Acquire) == 1);
    Ok(())
}
