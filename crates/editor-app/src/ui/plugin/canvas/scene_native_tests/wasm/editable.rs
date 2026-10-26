//! Real OS Chinese input round-trips through an independent WASM package and public Manager.
use super::*;
use gpui_kit::{AsyncApp, EntityInputHandler};
pub(in crate::ui::plugin::canvas::scene_native_tests) mod uia;

/// Drain original native events, publish actual guest updates and await real native display receipts.
async fn settle(
    owner: &mut InstalledFlow,
    view: &Entity<PluginView>,
    events: &Rc<RefCell<Vec<ui::UiEvent>>>,
    consumed: &mut usize,
    window: gpui_kit::AnyWindowHandle,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let mut stable = 0;
    for _ in 0..24 {
        let pending = events.borrow()[*consumed..].to_vec();
        *consumed += pending.len();
        owner.deliver(&pending)?;
        let changed = window.update(cx, |_, _, cx| view.read(cx).document != owner.document)?;
        if changed {
            let document = owner.document.clone();
            let documents =
                BTreeMap::from([("ordinary/diagram".into(), Arc::new(document.clone()))]);
            let images = cx
                .background_executor()
                .spawn(async move {
                    let mut renderer =
                        super::super::super::super::images::VectorRenderer::default();
                    renderer.set_environment(Environment::default());
                    renderer.prepare_resources(&documents, &BTreeMap::new())
                })
                .await;
            window.update(cx, |_, window, cx| {
                view.update(cx, |view, cx| {
                    view.update_document(document, Environment::default(), window, cx);
                    view.update_images("ordinary/diagram", &images, window, cx);
                });
                window.draw(cx).clear(cx);
            })?;
        }
        cx.background_executor()
            .timer(Duration::from_millis(50))
            .await;
        stable = if events.borrow().len() == *consumed && !changed {
            stable + 1
        } else {
            0
        };
        if stable == 2 {
            return Ok(());
        }
    }
    anyhow::bail!(
        "native guest updates did not settle within the bounded acceptance window; ui_revision={}, pending_events={}",
        owner.document.revision,
        events.borrow().len().saturating_sub(*consumed)
    )
}
/// Focus the guest's declared emoji text object and select the original joined emoji via native UTF-16.
fn focus(
    kind: &str,
    view: &Entity<PluginView>,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<()> {
    let snapshot = view
        .read(cx)
        .native_canvas_input_snapshot("drawing", window, cx)
        .unwrap();
    let (x, y) = if kind == "table" {
        (180., 20.)
    } else {
        (35., 35.)
    };
    let position = point(
        px(snapshot["origin_x"].as_f64().unwrap() as f32 + x),
        px(snapshot["origin_y"].as_f64().unwrap() as f32 + y),
    );
    window.dispatch_event(
        gpui_kit::PlatformInput::MouseDown(MouseDownEvent {
            position,
            button: MouseButton::Left,
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        }),
        cx,
    );
    window.dispatch_event(
        gpui_kit::PlatformInput::MouseUp(MouseUpEvent {
            position,
            button: MouseButton::Left,
            modifiers: Default::default(),
            click_count: 1,
        }),
        cx,
    );
    window.draw(cx).clear(cx);
    let input = super::super::super::targets::native_text_tests::focused_endpoint(view, window, cx);
    input.update(cx, |input, cx| {
        input.set_selected_text_range(3..8, window, cx);
        anyhow::ensure!(
            input.selected_text_range(false, window, cx).unwrap().range == (3..8),
            "actual package native emoji selection"
        );
        Ok(())
    })
}
/// Read published domain text through the declared session association, never through guest internals.
fn domain_text(owner: &InstalledFlow, kind: &str) -> String {
    let Kind::Canvas(canvas) = &owner.document.active_node("drawing").unwrap().kind else {
        unreachable!()
    };
    let scene = canvas.scene.as_ref().unwrap();
    let id = if kind == "table" { "cell-1" } else { "start" };
    let binding = scene
        .targets
        .iter()
        .find(|target| target.id == id)
        .unwrap()
        .text_input
        .as_ref()
        .unwrap();
    scene
        .paragraph_text(&binding.paragraph, &binding.layout_key)
        .unwrap()
        .to_owned()
}
/// Exercise actual installed Chinese IME, public frame/input gates, WASM domain commit and native ACK.
pub(in crate::ui::plugin::canvas::scene_native_tests) async fn run(
    kind: &str,
    owner: &mut InstalledFlow,
    view: &Entity<PluginView>,
    events: &Rc<RefCell<Vec<ui::UiEvent>>>,
    window: gpui_kit::AnyWindowHandle,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let mut consumed = 0;
    settle(owner, view, events, &mut consumed, window, cx)
        .await
        .map_err(|error| anyhow::anyhow!("initial display: {error:#}"))?;
    let handle = window.update(cx, |_, window, _| super::super::uia::owned_handle(window))??;
    window.update(cx, |_, _, _| super::super::ime::prepare_owned(handle))??;
    cx.background_executor()
        .timer(Duration::from_millis(150))
        .await;
    window.update(cx, |_, window, cx| focus(kind, view, window, cx))??;
    settle(owner, view, events, &mut consumed, window, cx)
        .await
        .map_err(|error| anyhow::anyhow!("after native focus/selection: {error:#}"))?;
    window.update(cx, |_, _, _| super::super::ime::start_owned(handle))??;
    cx.background_executor()
        .timer(Duration::from_millis(500))
        .await;
    settle(owner, view, events, &mut consumed, window, cx)
        .await
        .map_err(|error| anyhow::anyhow!("during OS preedit: {error:#}"))?;
    window.update(cx, |_, window, cx| {
        let input =
            super::super::super::targets::native_text_tests::focused_endpoint(view, window, cx);
        input.update(cx, |input, cx| {
            anyhow::ensure!(
                input.marked_text_range(window, cx).is_some(),
                "actual package lost OS preedit during WASM/native roundtrip"
            );
            Ok::<_, anyhow::Error>(())
        })?;
        Ok::<_, anyhow::Error>(())
    })??;
    anyhow::ensure!(
        domain_text(owner, kind) == "e\u{301} 👩‍💻",
        "preedit must not commit guest domain text"
    );
    window.update(cx, |_, _, _| super::super::ime::commit_owned(handle))??;
    cx.background_executor()
        .timer(Duration::from_millis(450))
        .await;
    settle(owner, view, events, &mut consumed, window, cx)
        .await
        .map_err(|error| anyhow::anyhow!("after OS commit: {error:#}"))?;
    anyhow::ensure!(
        domain_text(owner, kind) == "e\u{301} 中文",
        "actual WASM did not publish the independent OS commit: {:?}",
        domain_text(owner, kind)
    );
    super::super::ime::verify(&events.borrow())?;
    window.update(cx, |_, window, cx| {
        let input =
            super::super::super::targets::native_text_tests::focused_endpoint(view, window, cx);
        input.update(cx, |input, cx| {
            anyhow::ensure!(
                input.marked_text_range(window, cx).is_none(),
                "actual guest ACK retained ended preedit"
            );
            anyhow::ensure!(
                input
                    .text_for_range(0..65536, &mut None, window, cx)
                    .as_deref()
                    == Some("e\u{301} 中文"),
                "actual native text after guest ACK"
            );
            Ok(())
        })
    })??;
    eprintln!("actual {kind} WASM + OS Chinese IME + native ACK passed");
    let target = if kind == "table" {
        "canvas:1:semantic-1"
    } else {
        "canvas:1:start-name"
    };
    let before = cx
        .background_executor()
        .spawn(async move { uia::probe(handle, target, true) })
        .await?;
    anyhow::ensure!(
        before["text"] == "e\u{301} 中文" && before["focus"] == true,
        "actual guest UIA before edit: {before}"
    );
    if kind == "table" {
        anyhow::ensure!(
            before["grid"]
                .as_str()
                .is_some_and(|text| text.contains("100000")),
            "actual table native virtual totals: {before}"
        );
    }
    settle(owner, view, events, &mut consumed, window, cx)
        .await
        .map_err(|error| anyhow::anyhow!("after UIA SetValue: {error:#}"))?;
    anyhow::ensure!(
        domain_text(owner, kind) == "辅助 value",
        "native UIA SetValue did not reach the actual guest"
    );
    let after = cx
        .background_executor()
        .spawn(async move { uia::probe(handle, target, false) })
        .await?;
    anyhow::ensure!(
        after["text"] == "辅助 value",
        "actual guest native semantic update missing: {after}"
    );
    eprintln!("actual {kind} WASM + Windows UIA SetValue/native redraw passed: {after}");
    Ok(())
}
