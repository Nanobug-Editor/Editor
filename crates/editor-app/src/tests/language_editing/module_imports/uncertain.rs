//! A failure without an exact release receipt cannot revoke authority over possibly loaded code.
use super::*;

/// A standard JSON-RPC business failure says nothing about whether its imported code is retained.
#[gpui::test]
#[ignore = "set current SDK NANOBUG_CONTROL_FIXTURE_PACKAGE and NANOBUG_NATIVE_CONSUMERS_PACKAGE; build lsp_fixture"]
fn plain_native_failures_keep_possible_module_loads_authorized(cx: &mut TestAppContext) {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let path = workspace.path().join("source.txt");
    std::fs::write(&path, "saved source\n").unwrap();
    let path = path.canonicalize().unwrap();
    let wire = profile.path().join("wire.jsonl");
    let (app, visual, mut manager) =
        editor(cx, workspace.path(), profile.path(), packages(&wire, false));
    open(&app, &path, visual);
    for (plugin, reason) in [
        (
            "capability-example",
            plugin_runtime::DemandReason::Command("scope-probe".into()),
        ),
        (
            "rejected-module-source",
            plugin_runtime::DemandReason::Service("novel.module".into()),
        ),
    ] {
        crate::extensions::demand_plugin_fixture(&mut manager, plugin, reason).unwrap();
    }
    let accepted = begin(
        &app,
        &mut manager,
        visual,
        "fixture/barrier",
        vec![Import {
            export: "healthy".into(),
            reference: None,
        }],
        15_000,
    );
    finish(&app, &mut manager, visual, &accepted);
    assert_completed(&accepted);
    let document = DocumentContext {
        path: "source.txt".into(),
        generation: manager
            .availability("capability-example")
            .unwrap()
            .configuration_generation(),
    };
    let original = manager
        .language_document_service("capability-example/analysis", &document)
        .unwrap()
        .unwrap();
    let reference = open_module(&mut manager);
    let request = begin(
        &app,
        &mut manager,
        visual,
        "fixture/refuse",
        vec![Import {
            export: "rejected".into(),
            reference: Some(reference),
        }],
        15_000,
    );
    finish(&app, &mut manager, visual, &request);
    assert!(
        matches!(request.status(),api::RequestUpdate::Completed {result:Err(ref error)}
        if error.code==api::ErrorCode::OperationFailed),
        "expected a plain native business failure: {:?}",
        request.status()
    );
    // Unknown outcome affects only this wait. A real matched barrier must still use the old pipe.
    let barrier = begin(
        &app,
        &mut manager,
        visual,
        "fixture/barrier",
        Vec::new(),
        15_000,
    );
    finish(&app, &mut manager, visual, &barrier);
    assert_completed(&barrier);
    assert!(Arc::ptr_eq(
        &original,
        &manager
            .language_document_service("capability-example/analysis", &document)
            .unwrap()
            .unwrap()
    ));
    let observed = frames(&wire);
    assert_eq!(
        observed
            .iter()
            .filter(|message| message["method"] == "initialize")
            .count(),
        1
    );
    assert_eq!(
        observed
            .iter()
            .filter(|message| message["method"] == "fixture/refuse")
            .count(),
        1
    );
    manager.disable("rejected-module-source").unwrap();
    crate::extensions::lsp_tests::publish(&app, &mut manager, visual);
    assert!(
        !original.is_active(),
        "unknown native outcome must retain the imported source's revocation authority"
    );
    assert_eq!(original.process_count(), 0);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "saved source\n");
    manager.shutdown();
}
