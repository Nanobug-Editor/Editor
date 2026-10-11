//! Native Problems controls retain Base modal semantics, accessible names and the editor focus lease.

use gpui_kit::{Focusable as _, TestAppContext, gpui};

/// Even an empty projection exposes named controls and returns keyboard focus on Escape.
#[gpui::test]
fn native_problems_heading_controls_and_escape_restore_focus(cx: &mut TestAppContext) {
    super::with_shortcut_editor(cx, false, Vec::new(), |visual, app, _| {
        visual.simulate_keystrokes("ctrl-shift-m");
        visual.run_until_parked();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
            let nodes = gpui_base::test_support::snapshots(window);
            assert!(
                nodes
                    .iter()
                    .any(|node| node.role() == Some(gpui_kit::Role::Dialog))
            );
            let heading = nodes
                .iter()
                .find(|node| node.role() == Some(gpui_kit::Role::Heading))
                .expect("Problems must publish its visible title as a native heading");
            assert_eq!(
                heading.label(),
                Some(rust_i18n::t!("problems.title", count = 0).as_ref())
            );
            for label in [
                rust_i18n::t!("problems.reveal"),
                rust_i18n::t!("problems.close"),
            ] {
                assert!(
                    nodes.iter().any(|node| {
                        node.role() == Some(gpui_kit::Role::Button)
                            && node.label() == Some(label.as_ref())
                            && node.visible()
                    }),
                    "Problems control has no native accessible name: {label}"
                );
            }
        });
        visual.simulate_keystrokes("escape");
        visual.run_until_parked();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
            assert!(app.read(cx).editor.focus_handle(cx).is_focused(window));
            assert!(
                !gpui_base::test_support::snapshots(window)
                    .iter()
                    .any(|node| node.role() == Some(gpui_kit::Role::Dialog))
            );
        });
    });
}
