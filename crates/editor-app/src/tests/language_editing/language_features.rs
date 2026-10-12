//! Pending native linked input cannot revive an obsolete language intent after cancellation or tabs.
use super::*;
use plugin_runtime::plugin_protocol::direct;

/// Observe the actual painted heading, including loading/error cards with no selectable row.
fn references_visible(visual: &mut VisualTestContext) -> bool {
    visual.update(|window, _| {
        let name = rust_i18n::t!("language_features.references");
        gpui_base::test_support::snapshots(window)
            .iter()
            .any(|node| {
                node.role() == Some(gpui_kit::Role::Heading)
                    && node.label() == Some(name.as_ref())
                    && node.visible()
            })
    })
}

/// A semantic response is a real slow stdio fixture, not a manufactured pending host state.
#[gpui::test]
#[ignore = "build javascript.zip for the approved private Node runtime"]
fn pending_linked_input_cannot_revive_cancelled_language_intents(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let source = "<!-- 中文🙂 -->\n<item>内容🙂</item>\n";
    let path = directory.path().join("pending.linked");
    let other = directory.path().join("other.txt");
    std::fs::write(&path, source).unwrap();
    std::fs::write(&other, "other native document").unwrap();
    let path = path.canonicalize().unwrap();
    let other = other.canonicalize().unwrap();
    let package = editing_fixture::package(
        "pending-intent-language",
        "fixture",
        true,
        true,
        &directory.path().join("wire.jsonl"),
        150,
        "slow",
    );
    let (app, visual, mut manager) = shell(cx, directory.path(), vec![package]);
    open(&app, &path, visual);
    visual.update(|_, cx| {
        let state = app.read(cx);
        let server = state.language_servers.get("fixture").unwrap_or_else(|| {
            panic!(
                "Actual prepared service/selection failure: {:?}, {:?}, {}",
                manager
                    .language_services()
                    .into_iter()
                    .map(|(key, result)| (key, result.err()))
                    .collect::<Vec<_>>(),
                crate::language::providers::language_servers(),
                state.status
            )
        });
        server.prepare_until_ready().unwrap()
    });

    // Prove the observation seam first. This server does not advertise References: its honest
    // unsupported result still has a native heading and must be rejected when the intent expires.
    visual.simulate_keystrokes("shift-f12");
    await_condition(
        visual,
        references_visible,
        "fresh language intent paints its native heading",
    );
    visual.simulate_keystrokes("escape");
    pump(visual, 30);
    assert!(!references_visible(visual));

    let start = source.find("<item>").unwrap() + 2;
    for stage in ["explicit cancel", "A→B→A", "close/reopen"] {
        immediate_caret(&app, start, visual);
        visual.simulate_input("X");
        assert!(
            visual.update(|_, cx| app
                .read(cx)
                .language_edits
                .bridge
                .as_ref()
                .unwrap()
                .read(cx)
                .has_pending()),
            "{stage} must enter during real semantic wait"
        );
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.request_language_feature(direct::Feature::References, window, cx);
                match stage {
                    "explicit cancel" => app.cancel_language_features(window, cx),
                    "A→B→A" => {
                        app.open_file(other.clone(), window, cx);
                        app.open_file(path.clone(), window, cx);
                    }
                    _ => {
                        let previous = app
                            .plugin_document_version(app.active_text_tab_index().unwrap())
                            .unwrap();
                        app.discard_tab(path.clone(), window, cx);
                        assert!(app.tabs.iter().all(|tab| tab.path() != path));
                        app.open_file(path.clone(), window, cx);
                        let current = app
                            .plugin_document_version(app.active_text_tab_index().unwrap())
                            .unwrap();
                        assert_ne!(
                            previous.id, current.id,
                            "close/reopen retires the native incarnation"
                        );
                    }
                }
            })
        });
        // Drain both real IO and the test executor's timer, observing every frame rather than
        // asserting only that an empty/error result happens to have no reference rows.
        for _ in 0..50 {
            pump(visual, 10);
            assert!(
                !references_visible(visual),
                "A pending intent revived after {stage}"
            );
        }
        if stage == "explicit cancel" {
            assert_text(&app, &source.replace("item", "iXtem"), visual);
        } else if stage == "A→B→A" {
            assert_text(&app, &source.replacen("item", "iXtem", 1), visual);
        } else {
            assert_text(&app, source, visual);
        }
        if stage != "close/reopen" {
            visual.simulate_keystrokes("ctrl-z");
            assert_text(&app, source, visual);
        }
    }
    // Cancellation only retires old readonly intent; the same healthy service admits a new one.
    visual.simulate_keystrokes("shift-f12");
    await_condition(
        visual,
        references_visible,
        "new language intent remains available",
    );
    visual.simulate_keystrokes("escape");
    assert_text(&app, source, visual);
}
