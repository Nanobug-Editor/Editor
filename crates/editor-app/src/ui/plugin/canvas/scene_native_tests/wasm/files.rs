//! A real OS file drag reaches an actual WASM recipient and becomes only a bounded import handle.
use super::*;
use futures::StreamExt as _;
use gpui_kit::{AsyncApp, Role};
use platform_windows::drag;
use protocol::api;
use protocol::transfer::{self, Operation, Value};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE,
};

/// The external OS producer exposes paths only to the native boundary, never to serialized guest input.
struct Files {
    path: std::path::PathBuf,
    reads: Arc<AtomicUsize>,
}
impl drag::Provider for Files {
    fn formats(&self) -> Vec<String> {
        vec![transfer::FILES_MIME.into()]
    }
    fn read(&self, mime: &str) -> Result<drag::Data, drag::Error> {
        if mime != transfer::FILES_MIME {
            return Err(drag::Error::Invalid);
        }
        self.reads.fetch_add(1, Ordering::AcqRel);
        Ok(drag::Data::Files(vec![self.path.clone()]))
    }
}
/// Transport through the ordinary native port, then exercise public selected-read and chunk operations.
pub(in crate::ui::plugin::canvas::scene_native_tests) async fn run(
    owner: InstalledFlow,
    view: &Entity<PluginView>,
    events: Rc<RefCell<Vec<ui::UiEvent>>>,
    window: gpui_kit::AnyWindowHandle,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("用户拖入.txt");
    std::fs::write(&path, "只读中文 / Imported")?;
    let reads = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicBool::new(false));
    let owner = Rc::new(RefCell::new(owner));
    let port = owner.clone();
    let completed = dropped.clone();
    let (sender, mut receiver) = futures::channel::mpsc::unbounded();
    cx.spawn(async move |_| {
        let mut delivered = 0;
        while let Some(ingress) = receiver.next().await {
            let fresh = events.borrow()[delivered..].to_vec();
            delivered += fresh.len();
            let is_drop = matches!(
                &ingress,
                crate::ui::plugin::transfer::Ingress::Update {
                    phase: transfer::Phase::Drop,
                    ..
                }
            );
            let mut port = port.borrow_mut();
            if let Err(error) = port
                .deliver(&fresh)
                .and_then(|_| port.native_source(ingress))
            {
                eprintln!("native file ingress: {error:#}");
            } else if is_drop {
                completed.store(true, Ordering::Release);
            }
        }
    })
    .detach();
    let (hwnd, x, y) = window.update(cx, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.set_native_transfers(
                Rc::new(move |ingress| sender.unbounded_send(ingress).is_ok()),
                window,
                cx,
            )
        })?;
        window.draw(cx).clear(cx);
        let target = gpui_base::test_support::snapshots(window)
            .into_iter()
            .find(|node| {
                node.visible()
                    && node.role() == Some(Role::Button)
                    && node.label() == Some("拖入文字或图片 / Drop text or image")
            })
            .ok_or_else(|| anyhow::anyhow!("actual file recipient missing"))?;
        let visible = target.bounds().intersect(&gpui_kit::Bounds::new(
            gpui_kit::point(px(0.), px(0.)),
            window.viewport_size(),
        ));
        anyhow::ensure!(
            visible.size.width > px(16.) && visible.size.height > px(0.),
            "file recipient must intersect its actual viewport"
        );
        let center = visible.center();
        Ok::<_, anyhow::Error>((
            super::super::uia::owned_handle(window)?,
            (center.x / px(1.) * window.scale_factor()) as i32,
            (center.y / px(1.) * window.scale_factor()) as i32,
        ))
    })??;
    let provider = Files {
        path,
        reads: reads.clone(),
    };
    cx.background_executor()
        .spawn(async move {
            use super::super::transfer::pointer;
            use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::*};
            // Raise only this owned acceptance HWND; the original physical-hit guard stays mandatory.
            unsafe {
                SetWindowPos(
                    HWND(hwnd as *mut _),
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                )?;
            }
            pointer(hwnd, x, y, MOUSEEVENTF_LEFTDOWN)?;
            let source = std::thread::spawn(move || {
                drag::start(
                    hwnd as isize,
                    Arc::new(provider),
                    Arc::new(AtomicBool::new(true)),
                )
            });
            std::thread::sleep(Duration::from_millis(300));
            pointer(hwnd, x + 8, y, MOUSEEVENTF_MOVE)?;
            std::thread::sleep(Duration::from_millis(200));
            pointer(hwnd, x + 8, y, MOUSEEVENTF_LEFTUP)?;
            anyhow::ensure!(
                source.join().unwrap()? == drag::Outcome::Dropped,
                "real file drag must be accepted by the generic recipient"
            );
            Ok::<_, anyhow::Error>(())
        })
        .await?;
    let deadline = Instant::now() + Duration::from_secs(3);
    while !dropped.load(Ordering::Acquire) && Instant::now() < deadline {
        cx.background_executor()
            .timer(Duration::from_millis(10))
            .await;
    }
    anyhow::ensure!(
        dropped.load(Ordering::Acquire),
        "actual guest missed native file Drop"
    );
    anyhow::ensure!(
        reads.load(Ordering::Acquire) == 0,
        "Enter/Move/Drop must be metadata only"
    );
    let event: transfer::Event = serde_json::from_str(&exchange(&owner.borrow()))?;
    let value = request(
        &owner,
        Operation::Read {
            offer: event.offer.handle.clone(),
            preferred: vec![transfer::FILES_MIME.into()],
        },
        cx,
    )
    .await?;
    let Value::Files(files) = value else {
        anyhow::bail!("controlled imported entries expected")
    };
    anyhow::ensure!(files.len() == 1 && files[0].name == "用户拖入.txt");
    anyhow::ensure!(
        !serde_json::to_string(&files)?.contains(&temporary.path().display().to_string())
    );
    let value = request(
        &owner,
        Operation::ReadFile {
            file: files[0].handle.clone(),
            offset: 0,
            max_bytes: 256,
        },
        cx,
    )
    .await?;
    let Value::FileBytes { bytes, .. } = value else {
        anyhow::bail!("bounded file bytes expected")
    };
    anyhow::ensure!(
        bytes == "只读中文 / Imported".as_bytes() && reads.load(Ordering::Acquire) == 1
    );
    {
        let mut owner = owner.borrow_mut();
        let plugin = owner.plugin.clone();
        owner.manager.invoke_command(
            &plugin,
            "exchange-probe",
            serde_json::to_value(api::Operation::CloseResource {
                handle: event.offer.handle,
            })?,
        )?;
    }
    // Closing the guest's original offer must release its pinned file before the temporary source is removed.
    temporary.close()?;
    Ok(())
}
/// Read only an ordinary guest-published observation; there is no test-only host result API.
fn exchange(owner: &InstalledFlow) -> String {
    let mut value = String::new();
    owner.manager.live[&owner.plugin].views[&owner.panel]
        .root
        .visit(&mut |node| {
            if node.id == "exchange"
                && let Kind::Text { text } = &node.kind
            {
                value = text.clone();
            }
        });
    value
}
/// Execute the same admitted resource IO as the production worker, retaining its original completion.
async fn request(
    owner: &Rc<RefCell<InstalledFlow>>,
    operation: Operation,
    cx: &mut AsyncApp,
) -> anyhow::Result<Value> {
    let request = {
        let mut owner = owner.borrow_mut();
        let plugin = owner.plugin.clone();
        owner.manager.invoke_command(
            &plugin,
            "exchange-probe",
            serde_json::to_value(api::Operation::Editor {
                operation: api::EditorOperation::Transfer { operation },
                timeout_ms: 30_000,
            })?,
        )?;
        let (_, request) = owner
            .manager
            .take_editor_requests()
            .pop()
            .ok_or_else(|| anyhow::anyhow!("actual guest transfer request missing"))?;
        anyhow::ensure!(request.begin());
        request
    };
    let read = request.clone();
    let value = cx
        .background_executor()
        .spawn(async move {
            let offer = read.transfer_offer().unwrap();
            let api::EditorOperation::Transfer { operation } = read.operation() else {
                unreachable!()
            };
            match operation {
                Operation::Read { preferred, .. } => offer.read_selected(preferred),
                Operation::ReadFile {
                    file,
                    offset,
                    max_bytes,
                } => offer.read_file(file, *offset, *max_bytes),
            }
        })
        .await?;
    request.finish(Ok(api::EditorValue::Transfer(value)));
    owner.borrow_mut().manager.poll();
    let update: api::RequestUpdate<api::EditorValue> =
        serde_json::from_str(&exchange(&owner.borrow()))?;
    let api::RequestUpdate::Completed {
        result: Ok(api::EditorValue::Transfer(value)),
    } = update
    else {
        anyhow::bail!("guest did not observe successful selected transfer")
    };
    Ok(value)
}
