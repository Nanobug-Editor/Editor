//! Base navigation offsets are UTF-8 bytes; the real stdio request must encode their UTF-16 position.
use super::*;
use gpui_base::input::DefinitionProvider as _;
use serde_json::{Value, json};

/// Exercise the actual Base provider hook after a multibyte prefix and inspect the independent
/// framed server's position. No generated language result or duplicate conversion defines expected 6.
#[gpui::test]
#[ignore = "build plugin-runtime example lsp_fixture before the real stdio navigation regression"]
fn definition_provider_preserves_base_utf8_offset_after_unicode(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let source = "中😀 name";
    let path = directory.path().join("source.novel");
    std::fs::write(&path, source).unwrap();
    let path = path.canonicalize().unwrap();
    let log = directory.path().join("wire.jsonl");
    let executable = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/examples/lsp_fixture.exe")
        .canonicalize()
        .unwrap();
    let manifest = json!({"id":"navigation-offsets","name":"Navigation offsets","version":"0.1.0","protocol":7,
        "contributions":"plugin.toml","storage_limit":1024,
        "api":{"base":"^1","required":{"language.lsp":"^1","process":"^1"}},
        "permissions":["process.service.analysis"],
        "services":{"analysis":{"program":executable,"args":[log]}},
        "language_servers":[{"id":"analysis","language":"unfamiliar-offsets","service":"analysis"}]});
    let package = Package::from_files(std::collections::BTreeMap::from([
        ("manifest.json".into(), serde_json::to_vec(&manifest).unwrap()),
        ("plugin.toml".into(), b"[plugin]\nid='navigation-offsets'\nname='Navigation offsets'\nversion='0.1.0'\nhost_version='^0.1'\n".to_vec()),
    ])).unwrap();
    let (app, visual, mut manager) = shell(cx, directory.path(), vec![package]);
    open(&app, &path, visual);
    crate::extensions::demand_plugin_fixture(
        &mut manager,
        "navigation-offsets",
        plugin_runtime::DemandReason::Language {
            provider: "navigation-offsets/analysis".into(),
            role: plugin_runtime::LanguageRole::Lsp,
        },
    )
    .unwrap();
    let plan = manager.language_services()["navigation-offsets/analysis"]
        .as_ref()
        .unwrap()
        .clone();
    let server = Arc::new(crate::language::navigation::LanguageServer::from_service(plan).unwrap());
    let provider =
        crate::language::navigation::LanguageDefinitionProvider::new(&path, server).unwrap();
    let task = visual.update(|window, cx| {
        let rope = app.read(cx).editor.read(cx).text().clone();
        assert_eq!(rope.to_string(), source);
        provider.definitions(&rope, 10, window, cx)
    });
    let result = Rc::new(RefCell::new(None));
    let completed = result.clone();
    visual.update(|_, cx| {
        app.update(cx, |_, cx| {
            cx.spawn(async move |_, _| {
                *completed.borrow_mut() = Some(task.await);
            })
            .detach();
        })
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while result.borrow().is_none() {
        pump(visual, 10);
        assert!(
            std::time::Instant::now() < deadline,
            "real definition did not return"
        );
    }
    assert!(!result.borrow_mut().take().unwrap().unwrap().is_empty());
    let frames = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    let definition = frames
        .iter()
        .find(|frame| frame["method"] == "textDocument/definition")
        .unwrap();
    assert_eq!(
        definition["params"]["position"],
        json!({"line":0,"character":6}),
        "Base supplied byte offset 10 after 中😀; LSP must address the middle of name, not EOF"
    );
    manager.shutdown();
}
