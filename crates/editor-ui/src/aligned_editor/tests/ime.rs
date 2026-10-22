//! IME's notify-only native edits must invalidate source byte ranges, including equal-size candidates.
use super::*;

#[gpui::test]
fn aligned_editor_native_preedit_replacement_and_cancel_rebuild_only_changed_content(
    cx: &mut TestAppContext,
) {
    let original = "before\nrow06 中文😀\nafter\n";
    let (view, ui) = mount_sources(
        cx,
        [
            original.into(),
            "before\ninsert\nrow06 中文😀\nafter\n".into(),
        ],
    );
    let editor = ui.update(|window, cx| {
        let editor = view.read(cx).editors[0].clone();
        editor.update(cx, |state, cx| {
            state.set_selected_range(7..7, cx);
            state.focus(window, cx);
        });
        editor
    });
    settle(ui);
    let initial_epoch = ui.update(|_, cx| view.read(cx).group.state.read(cx).epoch);
    ui.update(|_, cx| editor.update(cx, |state, cx| state.set_selected_range(7..7, cx)));
    settle(ui);
    ui.update(|_, cx| {
        assert_eq!(
            view.read(cx).group.state.read(cx).epoch,
            initial_epoch,
            "selection notifications must not spend the row-plan budget again"
        );
    });
    ui.update(|window, cx| {
        editor.update(cx, |state, cx| {
            state.replace_and_mark_text_in_range(None, "拼😀", Some(1..3), window, cx);
        })
    });
    settle(ui);
    let (first_epoch, first_len) = ui.update(|window, cx| {
        let group = view.read(cx).group.clone();
        assert_eq!(group.status(cx), AlignmentStatus::Ready);
        assert!(group.state.read(cx).epoch > initial_epoch);
        editor.update(cx, |state, cx| {
            assert!(state.marked_text_range(window, cx).is_some())
        });
        (group.state.read(cx).epoch, editor.read(cx).text().len())
    });
    ui.update(|window, cx| {
        editor.update(cx, |state, cx| {
            state.replace_and_mark_text_in_range(None, "漢😃", Some(1..3), window, cx);
        })
    });
    settle(ui);
    let candidate_epoch = ui.update(|_, cx| {
        let fixture = view.read(cx);
        assert_eq!(editor.read(cx).text().len(), first_len);
        assert!(editor.read(cx).value().contains("漢😃"));
        assert_eq!(fixture.group.status(cx), AlignmentStatus::Ready);
        let epoch = fixture.group.state.read(cx).epoch;
        assert!(
            epoch > first_epoch,
            "equal-size native candidate replacement must invalidate"
        );
        epoch
    });
    ui.update(|window, cx| {
        editor.update(cx, |state, cx| {
            // This is Base's actual IME cancellation path: remove the marked bytes and notify only.
            state.replace_and_mark_text_in_range(None, "", None, window, cx);
            assert!(state.marked_text_range(window, cx).is_none());
        })
    });
    settle(ui);
    ui.update(|_, cx| {
        let fixture = view.read(cx);
        assert_eq!(editor.read(cx).value().as_ref(), original);
        assert_eq!(fixture.group.status(cx), AlignmentStatus::Ready);
        assert!(fixture.group.state.read(cx).epoch > candidate_epoch);
    });
}
