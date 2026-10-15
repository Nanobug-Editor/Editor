//! Native opt-in alignment integration regressions through a real Base Root and PluginView.
use super::*;
use crate::ui::controls::{
    self,
    aligned_editor::{AlignmentGroup, AlignmentStatus},
    aligned_pointer,
};
use gpui_base::{Root, input::EditorState};
use gpui_kit::{
    Bounds, EntityInputHandler as _, Focusable as _, MouseButton, TestAppContext,
    VisualTestContext, gpui, point, px, size,
};
use std::cell::RefCell;

mod workbench;

/// Fixture snapshots are expectations only. All visible source edits and Undo use this one entity.
struct Mounted {
    view: Entity<PluginView>,
    source: Entity<EditorState>,
    version: DocumentVersion,
    document: Document,
    current_revision: Rc<Cell<u64>>,
    events: Rc<RefCell<Vec<UiEvent>>>,
}

fn content(side: usize) -> String {
    (0..48)
        .map(|row| {
            let inserted = match (side, row) {
                (1, 3) => "source insertion\n",
                (2, 6) => "right insertion\nright insertion again\n",
                _ => "",
            };
            let body = match (side, row) {
                (1, 8) => "source changed 中文😀",
                (2, 11) => "right changed 中文😀",
                _ => "中文😀",
            };
            format!("{inserted}row{row:02} {body}\n")
        })
        .collect()
}

/// Keep the original fixed-size fixture independent of the workbench's flexible layout cases.
fn default_document(version: &DocumentVersion) -> Document {
    let literal = |id: &str, side| {
        ui::Node::new(
            id,
            Kind::ReadonlyCode {
                text: content(side),
                changes: Vec::new(),
                sync_group: Some("comparison".into()),
            },
        )
        .width(340.)
        .height(210.)
    };
    let mut document = Document::new(
        ui::Node::row(
            "panes",
            vec![
                literal("left", 0),
                ui::Node::new(
                    "source",
                    Kind::EditorWithActions {
                        document: version.clone(),
                        left: vec![ui::EditorAction {
                            offset: content(1).find("row02").unwrap(),
                            content: Box::new(ui::Node::button("rail-action", "Apply")),
                        }],
                        right: Vec::new(),
                        sync_group: Some("comparison".into()),
                    },
                )
                .width(340.)
                .height(210.),
                literal("right", 2),
            ],
        )
        .height(210.),
    )
    .code_alignment(true)
    .revision(1);
    document.source = Some(version.clone());
    document.editor_layout = true;
    document
}

fn mount(cx: &mut TestAppContext) -> (Mounted, &mut VisualTestContext) {
    mount_with_document(
        cx,
        default_document,
        content(1),
        false,
        false,
        size(px(1060.), px(260.)),
    )
}

/// Exercise the production window-source adapter with a caller-owned public scene and viewport.
/// Old guest revisions retain paint geometry; only the exact live revision can activate actions.
/// Line numbers remain configurable because their native gutter consumes real editable width.
fn mount_with_document(
    cx: &mut TestAppContext,
    build_document: impl FnOnce(&DocumentVersion) -> Document,
    source_text: String,
    soft_wrap: bool,
    line_numbers: bool,
    viewport: gpui_kit::Size<Pixels>,
) -> (Mounted, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::typography::init(cx);
        crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(false), cx);
        aligned_pointer::init(cx);
        cx.set_reduce_motion(true);
    });
    let version = DocumentVersion {
        id: "source:aligned-workbench".into(),
        path: "aligned-workbench.txt".into(),
        revision: 7,
    };
    let document = build_document(&version);
    let events = Rc::new(RefCell::new(Vec::new()));
    let current_revision = Rc::new(Cell::new(version.revision));
    let capture = Rc::new(RefCell::new(None));
    let slot = capture.clone();
    let initial = document.clone();
    let initial_version = version.clone();
    let output = events.clone();
    let revision = current_revision.clone();
    let (_, ui) = cx.add_window_view(move |window, cx| {
        let source = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("text")
                .line_number(line_numbers)
                .soft_wrap(soft_wrap)
        });
        source.update(cx, |state, cx| state.set_value(source_text, window, cx));
        let view = cx.new(|cx| {
            let mut view = PluginView::new(
                "alignment-test".into(),
                initial,
                Environment::default(),
                move |event, _| output.borrow_mut().push(event),
                window,
                cx,
            );
            view.set_code_sync_source(Some((initial_version.clone(), source.clone())), cx);
            let editor = source.clone();
            let expected = initial_version.clone();
            let live = revision.clone();
            view.native_editor = Some(Rc::new(move |target, actions, alignment, _, cx| {
                if target.id != expected.id
                    || target.path != expected.path
                    || target.revision > live.get()
                {
                    return None;
                }
                let body = alignment
                    .as_ref()
                    .and_then(|(group, index)| group.pane(*index));
                let source = controls::window_editor_body(&editor, body, cx);
                Some(if let Some(actions) = actions {
                    let projection = alignment.map(|(group, index)| {
                        Rc::new(move |offset, cx: &App| {
                            group.project_range(index, offset..offset, cx)
                        }) as controls::EditorActionProjection
                    });
                    controls::with_projected_editor_actions(source, &editor, actions, projection)
                } else {
                    source
                })
            }));
            let expected = initial_version;
            view.action_source_current = Some(Rc::new(move |target, _| {
                target.id == expected.id
                    && target.path == expected.path
                    && target.revision == revision.get()
            }));
            view
        });
        *slot.borrow_mut() = Some((view.clone(), source));
        Root::new(view, window, cx)
    });
    ui.simulate_resize(viewport);
    ui.update(|window, _| window.activate_window());
    settle(ui);
    let (view, source) = capture.borrow_mut().take().unwrap();
    (
        Mounted {
            view,
            source,
            version,
            document,
            current_revision,
            events,
        },
        ui,
    )
}

fn settle(ui: &mut VisualTestContext) {
    for _ in 0..8 {
        ui.update(|window, cx| {
            window.draw(cx).clear(cx);
            window.simulate_next_frame(cx);
        });
        ui.run_until_parked();
    }
}

/// Read the real Base button's accessibility and hit bounds. This Base version cannot publish
/// aria-disabled; it disables controls by withdrawing their native focus/click registration.
/// Check that supported focus seam, then observe real activation and source-focus preservation.
fn rail(ui: &mut VisualTestContext, disabled: bool) -> Bounds<Pixels> {
    ui.update(|window, _| {
        let snapshots = gpui_base::test_support::snapshots(window);
        let button = snapshots
            .iter()
            .find(|snapshot| {
                snapshot.role() == Some(gpui_kit::Role::Button)
                    && snapshot.path().iter().any(|part| {
                        matches!(part, gpui_kit::ElementId::Name(name) if name.as_str() == "plugin-ui-rail-action")
                    })
            })
            .expect("the actual native rail button must remain painted");
        assert!(button.visible());
        assert_eq!(button.focused().is_none(), disabled);
        button.bounds()
    })
}

fn group(mounted: &Mounted, cx: &App) -> AlignmentGroup {
    let view = mounted.view.read(cx);
    let group = &view.code_sync.groups["comparison"];
    assert!(group.alignment_requested);
    assert_eq!(group.panes[1].editor.as_ref(), Some(&mounted.source));
    let alignment = group.alignment.clone().expect("opted-in native group");
    assert_eq!(alignment.status(cx), AlignmentStatus::Ready);
    alignment
}

/// Compare actual original-native range geometry projected by the public group. At least three
/// unchanged source labels must occupy the same visible y in all three panes, before/after input.
fn assert_visible_alignment(mounted: &Mounted, ui: &mut VisualTestContext) {
    ui.update(|_, cx| {
        let alignment = group(mounted, cx);
        let view = mounted.view.read(cx);
        let editors = [
            view.readonly_code["left"].editor.clone(),
            mounted.source.clone(),
            view.readonly_code["right"].editor.clone(),
        ];
        let text = editors
            .each_ref()
            .map(|editor| editor.read(cx).value().to_string());
        let mut visible = 0;
        for row in 0..48 {
            let rectangles = std::array::from_fn::<_, 3, _>(|pane| {
                let offset = text[pane].find(&format!("row{row:02}"))?;
                alignment.project_range(pane, offset..offset, cx)
            });
            if let [Some(left), Some(source), Some(right)] = rectangles {
                assert!((left.top() - source.top()).abs() < px(0.75));
                assert!((right.top() - source.top()).abs() < px(0.75));
                visible += 1;
            }
        }
        assert!(
            visible >= 3,
            "must inspect real visible rows, not an offscreen metadata plan"
        );
    });
}

fn source_point(mounted: &Mounted, ui: &mut VisualTestContext, offset: usize) -> Point<Pixels> {
    ui.update(|_, cx| {
        let alignment = group(mounted, cx);
        let bounds = alignment.project_range(1, offset..offset, cx).unwrap();
        let pointer = point(
            bounds.left() + px(1.),
            bounds.top() + bounds.size.height / 2.,
        );
        let viewport = alignment.viewport(1, cx).unwrap();
        assert!(
            viewport.contains(&pointer),
            "native click {pointer:?} must lie inside the source viewport {viewport:?}; caret {bounds:?}"
        );
        pointer
    })
}

fn assert_painted_caret(mounted: &Mounted, ui: &mut VisualTestContext) {
    ui.update(|window, cx| {
        let alignment = group(mounted, cx);
        let native = mounted.source.read(cx);
        let (caret, _) = native.cursor_layout().unwrap();
        let expected = alignment.project_bounds(1, caret, cx).unwrap();
        let scale = window.scale_factor();
        assert!(
            window.painted_quads().iter().any(|quad| {
                (quad.bounds.left().0 / scale - f32::from(expected.left())).abs() < 0.5
                    && (quad.bounds.top().0 / scale - f32::from(expected.top())).abs() < 0.5
                    && (quad.bounds.size.width.0 / scale - f32::from(expected.size.width)).abs()
                        < 0.5
                    && (quad.bounds.size.height.0 / scale - f32::from(expected.size.height)).abs()
                        < 0.5
                    && quad.background.as_solid().is_some_and(|color| color.a > 0.)
                    && quad.content_mask.bounds.contains(&quad.bounds.center())
            }),
            "the unique native source caret must actually paint at its projected position"
        );
    });
}

#[gpui::test]
#[ignore = "native code-alignment input/IME/Undo integration; run explicitly with --ignored --test-threads=1"]
fn native_code_alignment_keeps_the_source_entity_and_geometry_through_edit_ime_and_undo(
    cx: &mut TestAppContext,
) {
    let (mounted, ui) = mount(cx);
    assert_visible_alignment(&mounted, ui);
    let original = content(1);
    let offset = original.find("row06").unwrap() + 6;
    // Row six follows insertion buckets in both peers and may be only partially painted at the
    // viewport bottom. Use ordinary source navigation before clicking its real visible caret.
    ui.update(|_, cx| assert!(group(&mounted, cx).reveal_offset(1, offset, cx)));
    settle(ui);
    let pointer = source_point(&mounted, ui, offset);
    ui.simulate_click(pointer, Default::default());
    ui.run_until_parked();
    settle(ui);
    let initial_width = ui.update(|_, cx| mounted.source.read(cx).input_bounds().size.width);
    ui.update(|window, cx| {
        assert_eq!(mounted.source.read(cx).selected_range(), offset..offset);
        assert!(mounted.source.focus_handle(cx).is_focused(window));
    });
    ui.simulate_input("中文😀");
    settle(ui);
    ui.update(|_, cx| {
        assert_eq!(
            mounted.source.read(cx).value().as_ref(),
            format!("{}中文😀{}", &original[..offset], &original[offset..])
        );
    });
    mounted.current_revision.set(8);
    ui.update(|_, cx| {
        mounted.view.update(cx, |view, cx| {
            let mut current = mounted.version.clone();
            current.revision = 8;
            view.set_code_sync_source(Some((current, mounted.source.clone())), cx);
            view.set_scene_current(false, cx);
        });
    });
    settle(ui);
    assert_visible_alignment(&mounted, ui);
    assert_painted_caret(&mounted, ui);
    ui.update(|_, cx| {
        assert!(
            (mounted.source.read(cx).input_bounds().size.width - initial_width).abs() < px(0.5)
        );
    });
    // Bring the disabled rail back into view while keeping the unchanged native caret visible.
    let rail_offset = original.find("row02").unwrap();
    ui.update(|_, cx| assert!(group(&mounted, cx).reveal_offset(1, rail_offset, cx)));
    settle(ui);
    let disabled_rail = rail(ui, true);
    mounted.events.borrow_mut().clear();
    ui.update(|window, cx| assert!(mounted.source.focus_handle(cx).is_focused(window)));
    ui.simulate_click(disabled_rail.center(), Default::default());
    ui.run_until_parked();
    settle(ui);
    ui.update(|window, cx| assert!(mounted.source.focus_handle(cx).is_focused(window)));
    assert!(
        mounted.events.borrow().is_empty(),
        "stale revision cannot activate the rail"
    );
    ui.simulate_keystrokes("ctrl-z");
    settle(ui);
    ui.update(|_, cx| {
        assert_eq!(mounted.source.read(cx).value().as_ref(), original);
    });
    assert_visible_alignment(&mounted, ui);
    // IME invokes the original native EntityInputHandler. No guest mutation request or second
    // mutable text model is involved; native-buffer notifications must rebuild the same group
    // even though preedit does not emit an ordinary InputEvent::Change.
    ui.update(|window, cx| {
        mounted.source.update(cx, |source, cx| {
            source.replace_and_mark_text_in_range(None, "拼😀", Some(1..3), window, cx);
            assert!(source.marked_text_range(window, cx).is_some());
        });
    });
    settle(ui);
    assert_visible_alignment(&mounted, ui);
    ui.update(|window, cx| {
        mounted.source.update(cx, |source, cx| {
            source.replace_text_in_range(None, "中文😀", window, cx);
        });
    });
    ui.simulate_keystrokes("ctrl-z");
    settle(ui);
    ui.update(|window, cx| {
        assert_eq!(mounted.source.read(cx).value().as_ref(), original);
        assert!(mounted.source.focus_handle(cx).is_focused(window));
        let view = mounted.view.read(cx);
        assert_eq!(
            view.readonly_code["left"].editor.read(cx).value().as_ref(),
            content(0)
        );
        assert_eq!(
            view.readonly_code["right"].editor.read(cx).value().as_ref(),
            content(2)
        );
    });
    assert_visible_alignment(&mounted, ui);
    assert_painted_caret(&mounted, ui);
}

#[gpui::test]
#[ignore = "native code-alignment wheel/action-rail integration; run explicitly with --ignored --test-threads=1"]
fn native_code_alignment_routes_real_rails_and_wheel_across_three_panes(cx: &mut TestAppContext) {
    let (mounted, ui) = mount(cx);
    assert_visible_alignment(&mounted, ui);
    let button = rail(ui, false);
    ui.update(|_, cx| {
        let offset = content(1).find("row02").unwrap();
        let expected = group(&mounted, cx)
            .project_range(1, offset..offset, cx)
            .unwrap();
        assert!((button.top() - expected.top()).abs() < px(0.75));
    });
    ui.simulate_click(button.center(), Default::default());
    assert!(
        mounted
            .events
            .borrow()
            .iter()
            .any(|event| event.node == "rail-action" && event.action == Action::Click)
    );
    mounted.events.borrow_mut().clear();
    for (pane, delta) in [(0, -160.), (2, 60.)] {
        let before = ui.update(|_, cx| group(&mounted, cx).scroll_offset(cx));
        let viewport = ui.update(|_, cx| group(&mounted, cx).viewport(pane, cx).unwrap());
        ui.simulate_event(gpui_kit::ScrollWheelEvent {
            position: viewport.center(),
            delta: gpui_kit::ScrollDelta::Pixels(point(px(0.), px(delta))),
            modifiers: Default::default(),
            touch_phase: gpui_kit::TouchPhase::Moved,
        });
        settle(ui);
        ui.update(|_, cx| assert_ne!(group(&mounted, cx).scroll_offset(cx), before));
        assert_visible_alignment(&mounted, ui);
    }
    ui.update(|_, cx| assert_eq!(mounted.source.read(cx).value().as_ref(), content(1)));
    assert!(
        mounted.events.borrow().is_empty(),
        "native scrolling cannot emit a guest write/action"
    );
}

#[gpui::test]
#[ignore = "native code-alignment withdrawal/remount integration; run explicitly with --ignored --test-threads=1"]
fn native_code_alignment_withdrawal_releases_drag_before_the_same_source_is_remounted(
    cx: &mut TestAppContext,
) {
    let (mounted, ui) = mount(cx);
    let text = content(1);
    let start = text.find("row00").unwrap();
    let end = text.find("row04").unwrap();
    let first = source_point(&mounted, ui, start);
    let later = source_point(&mounted, ui, end);
    ui.simulate_mouse_down(first, MouseButton::Left, Default::default());
    ui.simulate_mouse_move(later, MouseButton::Left, Default::default());
    ui.update(|window, cx| {
        assert_ne!(mounted.source.read(cx).selected_range(), start..start);
        mounted.view.update(cx, |view, cx| {
            view.retire_code_panes(cx);
            view.set_code_sync_source(None, cx);
            view.update_document(
                Document::new(ui::Node::text("hidden", "Hidden")).revision(2),
                Environment::default(),
                window,
                cx,
            );
        });
    });
    settle(ui);
    ui.update(|window, cx| {
        assert!(mounted.view.read(cx).code_sync.groups.is_empty());
        assert_eq!(
            mounted.view.read(cx).code_sync.alignment_budget.get(),
            MAX_CELLS
        );
        mounted.view.update(cx, |view, cx| {
            view.set_code_sync_source(Some((mounted.version.clone(), mounted.source.clone())), cx);
            view.update_document(
                mounted.document.clone().revision(3),
                Environment::default(),
                window,
                cx,
            );
            view.set_scene_current(true, cx);
        });
    });
    settle(ui);
    let selection = ui.update(|_, cx| mounted.source.read(cx).selected_range());
    let first = source_point(&mounted, ui, start);
    ui.simulate_mouse_move(first, None, Default::default());
    ui.simulate_mouse_up(
        point(px(1050.), px(250.)),
        MouseButton::Left,
        Default::default(),
    );
    ui.simulate_mouse_move(first, MouseButton::Left, Default::default());
    ui.update(|_, cx| {
        assert_eq!(mounted.source.read(cx).selected_range(), selection);
        assert_eq!(mounted.source.read(cx).value().as_ref(), text);
    });
    assert_visible_alignment(&mounted, ui);
}
