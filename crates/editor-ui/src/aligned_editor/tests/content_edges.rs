//! Whole-content navigation includes peer-only leading/trailing rows without moving source selections.
use super::*;

/// Variable pane membership exercises the same native control and projection for two/three sources.
struct EdgeFixture {
    group: AlignmentGroup,
    router: AlignedPointerRouter,
    editors: Vec<Entity<EditorState>>,
    originals: Vec<String>,
    /// Only this peer owns the outer display rows; the middle result has neither of them.
    outer: usize,
}

impl Render for EdgeFixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut panes = div().size_full().flex().line_height(px(20.));
        for index in 0..self.editors.len() {
            panes = panes.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.group.pane(index).unwrap()),
            );
        }
        self.router.wrap(panes)
    }
}

/// No source ends in a newline: EOF must not rely on a matching final empty row in another pane.
fn mount_edges(
    cx: &mut TestAppContext,
    count: usize,
) -> (Entity<EdgeFixture>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
        geometry_glyphs::init(cx);
    });
    let (view, ui) = cx.add_window_view(|window, cx| {
        let common = (0..80)
            .map(|row| format!("common row {row:03}"))
            .collect::<Vec<_>>()
            .join("\n");
        let outer = if count == 2 { 0 } else { 2 };
        let mut originals = vec![common.clone(); count];
        originals[outer] = format!("peer-only leading\n{common}\npeer-only trailing");
        let editors = originals
            .iter()
            .map(|text| {
                cx.new(|cx| {
                    let mut editor = EditorState::new(window, cx)
                        .line_number(false)
                        .soft_wrap(true);
                    editor.set_editor_paddings(Default::default());
                    editor.set_value(text.clone(), window, cx);
                    editor.set_selected_range(2..5, cx);
                    editor
                })
            })
            .collect::<Vec<_>>();
        let router = AlignedPointerRouter::default();
        let mut group = AlignmentGroup::new(cx);
        group
            .configure(
                editors
                    .iter()
                    .enumerate()
                    .map(|(index, editor)| {
                        AlignmentPane::new(editor.clone(), 1, format!("edge source {index}"))
                    })
                    .collect(),
                &router,
                cx,
            )
            .unwrap();
        EdgeFixture {
            group,
            router,
            editors,
            originals,
            outer,
        }
    });
    ui.simulate_resize(size(px(count as f32 * 320.), px(100.)));
    ui.update(|window, cx| {
        window.activate_window();
        cx.set_reduce_motion(true);
    });
    settle(ui);
    (view, ui)
}

/// Actual glyph bounds and the shared viewport, rather than a chosen pane's byte position, define edges.
fn verify_edges(cx: &mut TestAppContext, count: usize) {
    let (view, ui) = mount_edges(cx, count);
    for folded in [false, true] {
        if folded {
            ui.update(|_, cx| {
                let group = view.read(cx).group.clone();
                let svg: std::sync::Arc<[u8]> = std::sync::Arc::from(
                    include_bytes!("../../../assets/icons/explorer-expand-all.svg").as_slice(),
                );
                group.set_folding(UnchangedFolds::new(3, [svg.clone(), svg]), cx);
                group.fold_unchanged(true, cx);
            });
            settle(ui);
            ui.update(|_, cx| {
                assert!(
                    !view
                        .read(cx)
                        .group
                        .state
                        .read(cx)
                        .folding
                        .as_ref()
                        .unwrap()
                        .collapsed()
                        .is_empty()
                );
            });
        }
        // Visit the far end first, ensuring the subsequent start request really moves the viewport.
        for end in [true, false, true] {
            ui.update(|_, cx| {
                assert!(view.read(cx).group.clone().reveal_content_edge(end, cx));
            });
            settle(ui);
            ui.update(|_, cx| {
                let fixture = view.read(cx);
                assert_eq!(fixture.group.status(cx), AlignmentStatus::Ready);
                for (editor, original) in fixture.editors.iter().zip(&fixture.originals) {
                    assert_eq!(editor.read(cx).value().as_ref(), original);
                    assert_eq!(
                        editor.read(cx).selected_range(),
                        2..5,
                        "edge navigation must preserve native source selection"
                    );
                }
                let text = &fixture.originals[fixture.outer];
                let byte = if end { text.len() } else { 0 };
                let glyph = fixture
                    .group
                    .project_range(fixture.outer, byte..byte, cx)
                    .expect("peer-only outer row must be visible");
                let state = fixture.group.state.read(cx);
                let viewport = state.frames[fixture.outer].as_ref().unwrap().viewport;
                let delta = if end {
                    glyph.bottom() - viewport.bottom()
                } else {
                    glyph.top() - viewport.top()
                };
                assert!(
                    delta.abs() < px(1.),
                    "count={count}, folded={folded}, end={end}: {glyph:?} in {viewport:?}"
                );
                assert_eq!(
                    fixture.group.scroll_offset(cx),
                    if end { state.max_scroll() } else { px(0.) }
                );
            });
        }
    }
}

/// The result pane has gaps at both edges, including its no-newline EOF.
#[gpui::test]
fn content_edges_include_two_pane_leading_and_trailing_peer_rows(cx: &mut TestAppContext) {
    verify_edges(cx, 2);
}

/// A third parent's exclusive outer rows remain visible with complete and collapsed common regions.
#[gpui::test]
fn content_edges_include_three_pane_leading_and_trailing_peer_rows(cx: &mut TestAppContext) {
    verify_edges(cx, 3);
}
