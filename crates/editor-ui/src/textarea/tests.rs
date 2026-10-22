//! The local multiline control exposes its painted input while Base owns the only editing state.
use super::*;
use gpui_base::Root;
use gpui_base::test_support::{ElementSnapshot, snapshots};
use gpui_kit::gpui;
use gpui_kit::{AppContext as _, Context, Render, Role, TestAppContext};

/// A normal form host supplies bounded space but no synthetic input role or focus behavior.
struct Form {
    state: Entity<TextareaState>,
    height: Option<gpui_kit::Pixels>,
}
impl Render for Form {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(320.)).h(px(88.)).child(
            Textarea::new(&self.state).when_some(self.height, |view, height| view.height(height)),
        )
    }
}

/// Locate the actual Base input semantics rather than accepting an arbitrary form container.
fn field(window: &Window) -> ElementSnapshot {
    let fields = snapshots(window)
        .into_iter()
        .filter(|node| node.role() == Some(Role::TextInput) && node.visible())
        .collect::<Vec<_>>();
    assert_eq!(
        fields.len(),
        1,
        "the local textarea must expose one painted native text input"
    );
    fields.into_iter().next().unwrap()
}

/// Pointer focus and Chinese multiline typing must reach the caller's original TextareaState.
#[gpui::test]
fn local_textarea_exposes_native_focus_and_edits_the_original_multiline_state(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
    });
    let mut retained = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let state = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("提交说明")
                .auto_grow(3, 8)
        });
        retained = Some(state.clone());
        let form = cx.new(|_| Form {
            state,
            height: None,
        });
        Root::new(form, window, cx)
    });
    let original = retained.unwrap();
    let click = visual.update(|window, cx| {
        window.draw(cx).clear(cx);
        let field = field(window);
        assert!(field.bounds().size.width > px(0.));
        assert!(field.bounds().size.height > px(0.));
        assert!(field.disabled() != Some(true));
        field.bounds().center()
    });
    // No state.focus(), direct text mutation or fabricated accessibility node can satisfy this step.
    visual.simulate_click(click, Default::default());
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
        assert_eq!(field(window).focused(), Some(true));
        assert!(
            original
                .read(cx)
                .presentation()
                .focus_handle()
                .is_focused(window)
        );
    });
    // Native GPUI test input keeps the original handler, selection and undo history.
    visual.update(|window, cx| crate::testing::input_text(window, "提交说明中文😀", cx));
    visual.update(|window, cx| crate::testing::press_key(window, "enter", cx));
    visual.update(|window, cx| crate::testing::input_text(window, "第二行", cx));
    visual.run_until_parked();
    visual.update(|window, cx| {
        // Repainting the local appearance must keep the same caller-owned editor and focus.
        window.refresh();
        window.draw(cx).clear(cx);
        assert_eq!(field(window).focused(), Some(true));
        assert!(
            original
                .read(cx)
                .presentation()
                .focus_handle()
                .is_focused(window)
        );
        assert_eq!(original.read(cx).value().as_ref(), "提交说明中文😀\n第二行");
    });
}

/// An explicit viewport exposes its full native hit area even with only one line of content.
#[gpui::test]
fn fixed_textarea_fills_the_requested_viewport(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
    });
    let (_, visual) = cx.add_window_view(|window, cx| {
        let state = cx.new(|cx| TextareaState::new(window, cx).auto_grow(1, 1));
        let form = cx.new(|_| Form {
            state,
            height: Some(px(148.)),
        });
        Root::new(form, window, cx)
    });
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
        let height = field(window).bounds().size.height;
        assert!(
            height >= px(130.) && height <= px(148.),
            "native viewport: {height:?}"
        );
    });
}
