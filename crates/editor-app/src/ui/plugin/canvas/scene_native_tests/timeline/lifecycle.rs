//! Retire real native input and the owned HWND before this isolated application's ordinary quit.
use super::*;
use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::IsWindow};

/// Stop input first, drain two real paint opportunities, then await asynchronous native destruction.
/// Failed observations and retirement errors both survive; neither a sleep nor a frame proves exit.
pub(super) async fn retire(
    view: &Entity<PluginView>,
    window: AnyWindowHandle,
    cx: &mut gpui_kit::AsyncApp,
) -> anyhow::Result<()> {
    view.update(cx, |view, cx| view.set_scene_current(false, cx));
    let native = window
        .update(cx, |_, window, _| {
            let raw = window.window_handle().map_err(|error| {
                anyhow::anyhow!("timeline native handle unavailable: {error:?}")
            })?;
            let RawWindowHandle::Win32(raw) = raw.as_raw() else {
                anyhow::bail!("timeline retirement requires its own Win32 window");
            };
            Ok(raw.hwnd.get())
        })
        .and_then(|native| native);
    let input = retire_input(window, cx).await;
    // Even a failed frame wait must still execute ordinary window removal and report its result.
    let removed = window.update(cx, |_, window, _| window.remove_window());
    let destroyed = match native {
        Ok(hwnd) => wait_for_retirement(hwnd, cx).await,
        Err(error) => Err(error),
    };
    let failures = [input, removed, destroyed]
        .into_iter()
        .filter_map(Result::err)
        .map(|error| format!("{error:#}"))
        .collect::<Vec<_>>();
    anyhow::ensure!(
        failures.is_empty(),
        "timeline native retirement: {}",
        failures.join("; ")
    );
    eprintln!(
        "native timeline retirement: two post-blur frames completed; original HWND destroyed"
    );
    Ok(())
}

/// Match the production native harness: blur while alive, paint both retained buffers, await vsync.
async fn retire_input(window: AnyWindowHandle, cx: &mut gpui_kit::AsyncApp) -> anyhow::Result<()> {
    for _ in 0..2 {
        let (send, receive) = futures::channel::oneshot::channel();
        window.update(cx, |_, window, cx| {
            window.blur(cx);
            window.refresh();
            window.draw(cx).clear(cx);
            window.on_next_frame(move |_, _| {
                let _ = send.send(());
            });
        })?;
        let timeout = cx.background_executor().timer(Duration::from_secs(5));
        match futures::future::select(receive, Box::pin(timeout)).await {
            futures::future::Either::Left((delivered, _)) => {
                delivered
                    .map_err(|_| anyhow::anyhow!("timeline retirement frame callback dropped"))?;
            }
            futures::future::Either::Right(_) => {
                anyhow::bail!("timeline retirement frame timed out")
            }
        }
    }
    Ok(())
}

/// Query only the HWND borrowed above; yielding lets WindowsWindow's deferred DestroyWindow run.
async fn wait_for_retirement(hwnd: isize, cx: &mut gpui_kit::AsyncApp) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while unsafe { IsWindow(Some(HWND(hwnd as *mut _))).as_bool() } {
        anyhow::ensure!(
            Instant::now() < deadline,
            "timeline original HWND did not retire"
        );
        cx.background_executor()
            .timer(Duration::from_millis(10))
            .await;
    }
    Ok(())
}
