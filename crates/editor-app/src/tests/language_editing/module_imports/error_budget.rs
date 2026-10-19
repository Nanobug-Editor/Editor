//! Oversized native failures remain bounded without losing an exact import-release receipt.
use super::*;
use protocol::language_control::MAX_CONTROL_BYTES;

/// An independently built SDK guest must receive a bounded failure while the rejected import's
/// explicit release still protects the already accepted module and its original native process.
#[gpui::test]
#[ignore = "set current SDK NANOBUG_CONTROL_FIXTURE_PACKAGE and NANOBUG_NATIVE_CONSUMERS_PACKAGE; build lsp_fixture"]
fn oversized_native_errors_preserve_released_imports_and_the_healthy_context(
    cx: &mut TestAppContext,
) {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let path = workspace.path().join("source.txt");
    std::fs::write(&path, "saved source\n").unwrap();
    let path = path.canonicalize().unwrap();
    let wire = profile.path().join("wire.jsonl");
    let (app, visual, mut manager) = editor(
        cx,
        workspace.path(),
        profile.path(),
        packages_with_large_control_error(&wire),
    );
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
    let refusal_deadline = Instant::now() + Duration::from_secs(20);
    let refused = begin(
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
    finish(&app, &mut manager, visual, &refused);
    assert_bounded_failure(&app, &mut manager, visual, &refused, refusal_deadline);
    manager.disable("rejected-module-source").unwrap();
    crate::extensions::lsp_tests::publish(&app, &mut manager, visual);
    assert!(
        original.is_active(),
        "bounding the failure must preserve the exact release receipt for only its new import"
    );
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
            .filter(|message| message["method"] == "fixture/refuse")
            .count(),
        1,
        "the failed result must come from the real native control request"
    );
    assert_eq!(
        observed
            .iter()
            .filter(|message| message["method"] == "initialize")
            .count(),
        1,
        "the later barrier must use the original process rather than a replacement"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "saved source\n");
    manager.shutdown();
}

/// Revalidate the ordinary packages after opting only the owner into the larger fixture response.
fn packages_with_large_control_error(wire: &Path) -> Vec<Package> {
    let mut packages = packages(wire, true);
    let mut files = packages.remove(0).files;
    let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["services"]["analysis"]["args"]
        .as_array_mut()
        .unwrap()
        .push(json!("--large-control-error"));
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    packages.insert(0, Package::from_files(files).unwrap());
    packages
}

/// Observe the original public request and the independent guest's decoded lifecycle notification.
/// The byte assertion covers serialized UTF-8 Failure data, not a Unicode character count.
/// Drive Manager polling within the same 20-second observer guard, keeping the request's own
/// 15-second native timeout. Contribution publication alone does not poll an already warm guest.
fn assert_bounded_failure(
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    visual: &mut VisualTestContext,
    request: &plugin_runtime::EditorRequest,
    deadline: Instant,
) {
    let api::RequestUpdate::Completed { result: Err(error) } = request.status() else {
        panic!("the native refusal must complete its original request as an error");
    };
    assert_eq!(
        error.code,
        api::ErrorCode::LimitExceeded,
        "an oversized native control error must become a typed bounded failure"
    );
    assert!(serde_json::to_vec(&error).unwrap().len() <= MAX_CONTROL_BYTES);
    loop {
        manager.poll();
        crate::extensions::lsp_tests::publish(app, manager, visual);
        let Kind::Text { text } = &manager.live["capability-example"].views["welcome"]
            .as_ref()
            .root
            .kind
        else {
            panic!(
                "the SDK diagnostic view must expose request {:?}",
                request.handle()
            );
        };
        if let Ok(update) = serde_json::from_str::<api::RequestUpdate<api::EditorValue>>(text) {
            if let api::RequestUpdate::Completed { result: Err(error) } = &update {
                assert_eq!(
                    error.code,
                    api::ErrorCode::LimitExceeded,
                    "the SDK returned the wrong failure for {:?}: {text}",
                    request.handle()
                );
                assert!(serde_json::to_vec(error).unwrap().len() <= MAX_CONTROL_BYTES);
                break;
            }
        }
        assert!(
            Instant::now() < deadline,
            "the SDK must decode the completed failure within the observer guard; request={:?}, observed={text}",
            request.handle()
        );
        pump(visual, 10);
    }
}
