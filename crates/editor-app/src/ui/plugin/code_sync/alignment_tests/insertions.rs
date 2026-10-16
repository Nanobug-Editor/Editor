//! Peer-only insertion anchors must align in native panes without changing document state.
use super::*;
use std::ops::Range;

const TEXTS: [&str; 3] = ["a\nz\n", "a\nX\n共同\nz\n", "a\n共同\nY\nz\n"];

/// Use the public three-pane scene with the one editable result still bound to its source entity.
fn document(version: &DocumentVersion) -> Document {
    let peer = |id, text: &str| {
        ui::Node::new(
            id,
            Kind::ReadonlyCode {
                text: text.into(),
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
                peer("left", TEXTS[0]),
                ui::Node::new(
                    "source",
                    Kind::EditorWithActions {
                        document: version.clone(),
                        left: Vec::new(),
                        right: Vec::new(),
                        sync_group: Some("comparison".into()),
                    },
                )
                .width(340.)
                .height(210.),
                peer("right", TEXTS[2]),
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

/// Require visible rectangles from each original native editor, rather than inspecting logical
/// slots. Common insertion text and later pivot anchors must agree after the real pixel mapping.
fn assert_native_rows(
    mounted: &Mounted,
    ui: &mut VisualTestContext,
    selections: &[Range<usize>; 3],
) {
    ui.update(|_, cx| {
        let alignment = group(mounted, cx);
        let view = mounted.view.read(cx);
        let editors = [
            view.readonly_code["left"].editor.clone(),
            mounted.source.clone(),
            view.readonly_code["right"].editor.clone(),
        ];
        for pane in 0..3 {
            let editor = editors[pane].read(cx);
            assert_eq!(editor.value().as_ref(), TEXTS[pane]);
            assert_eq!(editor.selected_range(), selections[pane]);
        }
        let project = |pane: usize, label: &str| {
            let offset = if label.is_empty() {
                TEXTS[pane].len()
            } else {
                TEXTS[pane].find(label).unwrap()
            };
            let rectangle = alignment.project_range(pane, offset..offset, cx).unwrap();
            assert!(
                alignment
                    .viewport(pane, cx)
                    .unwrap()
                    .contains(&rectangle.center()),
                "the native row must be visible, not an offscreen alignment-plan entry",
            );
            rectangle
        };
        let common = [project(1, "共同"), project(2, "共同")];
        assert!((common[0].top() - common[1].top()).abs() < px(0.75));
        assert!(project(1, "X").top() < common[0].top());
        assert!(common[1].top() < project(2, "Y").top());
        assert!(project(2, "Y").top() < project(1, "z").top());
        // A trailing LF keeps its real EOF caret row in all panes, alongside ordinary anchors.
        for label in ["a", "z", ""] {
            let pivot = project(0, label);
            for pane in [1, 2] {
                assert!((project(pane, label).top() - pivot.top()).abs() < px(0.75));
            }
        }
    });
}

#[gpui::test]
#[ignore = "native peer-insertion geometry/state regression; run explicitly with --ignored --test-threads=1"]
fn native_code_alignment_peer_insertions_share_visible_rows_without_text_or_selection_changes(
    cx: &mut TestAppContext,
) {
    let (mounted, ui) = mount_with_document(
        cx,
        document,
        TEXTS[1].into(),
        false,
        false,
        size(px(1060.), px(260.)),
    );
    let selections = ui.update(|window, cx| {
        mounted.source.update(cx, |source, cx| {
            let start = TEXTS[1].find("共同").unwrap();
            source.set_selected_range(start..start + "共同".len(), cx);
            source.focus(window, cx);
        });
        let view = mounted.view.read(cx);
        [
            view.readonly_code["left"].editor.read(cx).selected_range(),
            mounted.source.read(cx).selected_range(),
            view.readonly_code["right"].editor.read(cx).selected_range(),
        ]
    });
    settle(ui);
    assert_native_rows(&mounted, ui, &selections);
    // Remeasure through the real window without using selection or source text as layout storage.
    ui.simulate_resize(size(px(1120.), px(300.)));
    settle(ui);
    assert_native_rows(&mounted, ui, &selections);
    assert!(mounted.events.borrow().is_empty());
}
