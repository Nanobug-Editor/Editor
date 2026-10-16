//! A current native scene must lose action authority before IME can change its proposal bytes.
use super::*;

#[gpui::test]
#[ignore = "native current-scene IME authority; run explicitly with --ignored --test-threads=1"]
fn native_code_alignment_current_actions_reject_preedit_and_unmarked_old_content(
    cx: &mut TestAppContext,
) {
    let (mounted, ui) = mount(cx);
    rail(ui, false);
    let original_width = ui.update(|window, cx| {
        mounted.source.update(cx, |source, cx| {
            // Keep the old rail's byte anchor valid and visible: inserting before it can place
            // that stale offset inside a UTF-8 scalar, where no native rectangle should exist.
            let insertion = content(1).find("row03").unwrap();
            source.set_selected_range(insertion..insertion, cx);
            source.focus(window, cx);
        });
        mounted.source.read(cx).input_bounds().size.width
    });
    mounted.events.borrow_mut().clear();
    // No stale-scene flag or synthetic revision advance precedes composition. Test the event
    // outlet in the same update, before native Notify effects or another paint can rescue it.
    let accepted = ui.update(|window, cx| {
        mounted.source.update(cx, |source, cx| {
            source.replace_and_mark_text_in_range(None, "拼😀", Some(0..3), window, cx);
            assert!(source.marked_text_range(window, cx).is_some());
        });
        mounted.view.update(cx, |view, cx| {
            view.emit_version_in(
                "rail-action",
                mounted.document.revision,
                Action::Click,
                window,
                cx,
            )
        })
    });
    assert!(
        !accepted,
        "preedit cannot forward an old ignore/confirm event"
    );
    assert!(mounted.events.borrow().is_empty());
    settle(ui);
    rail(ui, true);

    ui.update(|window, cx| {
        mounted.source.update(cx, |source, cx| {
            // Same UTF-8 byte length still represents another native candidate.
            source.replace_and_mark_text_in_range(None, "漢😃", Some(0..3), window, cx);
        });
        mounted.view.update(cx, |view, cx| {
            view.set_code_sync_source(Some((mounted.version.clone(), mounted.source.clone())), cx);
            view.update_document(
                mounted.document.clone().revision(2),
                Environment::default(),
                window,
                cx,
            );
            view.set_scene_current(true, cx);
        });
    });
    settle(ui);
    rail(ui, true);
    let accepted = ui.update(|window, cx| {
        mounted
            .source
            .update(cx, |source, cx| source.unmark_text(window, cx));
        // Base unmark has no Change/Notify. A same-source UI republication must not acknowledge
        // different bytes using the old version, even though no marked range remains now.
        mounted.view.update(cx, |view, cx| {
            view.emit_version_in("rail-action", 2, Action::Click, window, cx)
        })
    });
    assert!(!accepted, "unmark cannot resurrect an old proposal version");
    assert!(mounted.events.borrow().is_empty());

    mounted.current_revision.set(8);
    ui.update(|window, cx| {
        let mut version = mounted.version.clone();
        version.revision = 8;
        mounted.view.update(cx, |view, cx| {
            view.set_code_sync_source(Some((version.clone(), mounted.source.clone())), cx);
            view.update_document(
                default_document(&version).revision(3),
                Environment::default(),
                window,
                cx,
            );
            view.set_scene_current(true, cx);
        });
    });
    settle(ui);
    let current = rail(ui, false);
    ui.simulate_click(current.center(), Default::default());
    settle(ui);
    assert_eq!(
        mounted.events.borrow().len(),
        1,
        "only the fresh proposal can activate"
    );
    ui.update(|_, cx| {
        assert!(mounted.source.read(cx).value().contains("漢😃row03"));
        assert!(
            (mounted.source.read(cx).input_bounds().size.width - original_width).abs() < px(0.5)
        );
    });
}
