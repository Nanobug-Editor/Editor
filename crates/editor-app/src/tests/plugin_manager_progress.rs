//! Native lifecycle progress is observed while the existing transport retains its queued receipt.
use super::*;

/// Hiding unavailable actions must keep the user's currently executing global toggle visible.
#[gpui::test]
fn global_toggle_keeps_progress_and_hides_other_write_actions(cx: &mut TestAppContext) {
    for (locale, dark) in [("zh-CN", false), ("en", true)] {
        with_manager(cx, locale, dark, |form, owner| {
            let region = form.debug_bounds("plugin-global-scope-region").unwrap();
            form.simulate_click(
                point(region.left() + region.size.width * 0.25, region.center().y),
                Default::default(),
            );
            draw(form);
            // The real queue records this receipt before its transport processes it; no test sets progress.
            assert!(
                form.update(
                    |_, cx| owner.read(cx).progress.as_ref().is_some_and(|progress| {
                        progress.id == "manager-first"
                            && progress.action == worker::LifecycleAction::Enable
                    })
                )
            );
            assert!(
                form.debug_bounds("plugin-manager-operation-progress")
                    .is_some(),
                "the current global action needs visible progress"
            );
            for selector in [
                "plugin-update-action-region",
                "plugin-uninstall-action-region",
                "plugin-restart-action",
                "plugin-project-scope-region",
                "install-local-plugin",
            ] {
                assert!(
                    form.debug_bounds(selector).is_none(),
                    "unavailable action remains visible: {selector}"
                );
            }
            // Filtering away the active row cannot hide the receipt for its already admitted operation.
            let search = form.debug_bounds("plugin-manager-search").unwrap();
            form.simulate_click(search.center(), Default::default());
            form.simulate_input("__no_matching_plugin__");
            draw(form);
            assert!(form.debug_bounds("plugin-row-manager-first").is_none());
            assert!(
                form.debug_bounds("plugin-manager-operation-progress")
                    .is_some(),
                "progress must survive an empty filtered selection"
            );
        });
        // The existing helper assumes one editor/dialog pair; retire this case before another locale creates it.
        cx.update(|cx| {
            for handle in cx.windows() {
                let _ = cx.update_window(handle, |_, window, _| window.remove_window());
            }
        });
    }
}
