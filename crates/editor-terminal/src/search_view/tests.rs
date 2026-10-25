//! Real cached-view frames reproduce focus transfer after a preceding toolbar has been painted.

use super::*;
use editor_ui::Button;
use gpui_kit::{FocusHandle, StyleRefinement};
use std::{cell::RefCell, time::Duration};

/// The dock renders terminal bodies as cached views, after its ordinary toolbar children.
struct CachedBody {
    bar: Entity<SearchBar>,
    activate: bool,
}

impl Render for CachedBody {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let activate = std::mem::take(&mut self.activate);
        let model = SearchModel {
            session: 1,
            query: "needle".into(),
            ..Default::default()
        };
        self.bar
            .update(cx, |bar, cx| bar.update(model, activate, window, cx));
        div().size_full().child(self.bar.clone())
    }
}

/// One focused toolbar button and one cached body are the load-bearing parts of the native crash.
struct Probe {
    trigger: FocusHandle,
    body: Entity<CachedBody>,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        gpui_base::v_flex()
            .size_full()
            .child(
                Button::new("terminal-find-trigger")
                    .label("Find")
                    .track_focus(&self.trigger)
                    .on_click(cx.listener(|probe, _, _, cx| {
                        probe.body.update(cx, |body, cx| {
                            body.activate = true;
                            cx.notify();
                        })
                    })),
            )
            .child(
                div().relative().flex_1().min_h_0().child(
                    self.body
                        .clone()
                        .cached(StyleRefinement::default().absolute().size_full()),
                ),
            )
    }
}

/// This native-only loop uses public GPUI lifecycle hooks and asserts the focused field's name/value.
#[test]
#[ignore = "opens a real native window; set NANOBUG_TERMINAL_FIND_FOCUS_OUTPUT; requires active UIA"]
fn native_cached_find_preserves_one_accessible_focus_owner() {
    let output = std::path::PathBuf::from(
        std::env::var_os("NANOBUG_TERMINAL_FIND_FOCUS_OUTPUT")
            .expect("explicit accessibility artifact"),
    );
    assert!(output.is_absolute());
    let outcome = Rc::new(RefCell::new(None));
    let capture = outcome.clone();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .with_quit_mode(gpui_kit::QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            editor_ui::typography::init(cx);
            editor_ui::theme::apply_theme(editor_ui::theme::builtin_theme(false), cx);
            let slot = Rc::new(RefCell::new(None));
            let body_slot = slot.clone();
            let window = cx
                .open_window(gpui_kit::WindowOptions::default(), |window, cx| {
                    window.set_window_title("Nanobug cached terminal find focus probe");
                    let trigger = cx.focus_handle();
                    trigger.focus(window, cx);
                    let bar =
                        cx.new(|cx| SearchBar::new(SearchModel::default(), |_, _| {}, window, cx));
                    let body = cx.new(|_| CachedBody {
                        bar,
                        activate: false,
                    });
                    *body_slot.borrow_mut() = Some(body.clone());
                    let probe = cx.new(|_| Probe { trigger, body });
                    cx.new(|cx| gpui_base::Root::new(probe, window, cx))
                })
                .unwrap();
            let body = slot.borrow_mut().take().unwrap();
            cx.spawn(async move |cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                body.update(cx, |body, cx| {
                    body.activate = true;
                    cx.notify();
                });
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let observed = window
                    .update(cx, |_, window, _| -> anyhow::Result<()> {
                        anyhow::ensure!(window.is_a11y_active(), "native UIA activation required");
                        let json = window
                            .debug_a11y_tree_json()
                            .ok_or_else(|| anyhow::anyhow!("missing native accessibility tree"))?;
                        std::fs::write(output, &json)?;
                        let tree: serde_json::Value = serde_json::from_str(&json)?;
                        let focus = tree["gpui_focus"]
                            .as_str()
                            .ok_or_else(|| anyhow::anyhow!("find input must own focus"))?;
                        let focused = tree["nodes"][focus].to_string();
                        anyhow::ensure!(
                            focused.contains(&t!("terminal.find").to_string())
                                && focused.contains("needle"),
                            "find must own named native focus/value: {focused}"
                        );
                        Ok(())
                    })
                    .and_then(|value| value);
                *capture.borrow_mut() = Some(observed);
                window
                    .update(cx, |_, window, _| window.remove_window())
                    .unwrap();
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let _ = cx.update(|cx| cx.quit());
            })
            .detach();
        });
    outcome
        .borrow_mut()
        .take()
        .expect("native focus probe completed")
        .unwrap();
}
