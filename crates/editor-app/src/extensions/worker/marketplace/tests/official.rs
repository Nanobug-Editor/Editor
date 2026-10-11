//! Each independent Release enters the real native consent, download and installation path.
use super::*;
use crate::*;
use gpui_kit::{VisualTestContext, gpui};

/// Frozen GitHub snapshots and exact downloaded ZIPs cover all seven first installations once.
/// Negative permission, compatibility and outage matrices remain owned by the shared market tests.
#[gpui::test]
#[ignore = "export seven fixed official Release ZIPs and their registry-built target/marketplace-official/catalog.json"]
fn marketplace_official_releases_install_with_native_consent(cx: &mut gpui_kit::TestAppContext) {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/marketplace-official");
    let catalog: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture.join("catalog.json")).unwrap()).unwrap();
    let listings = catalog["plugins"].as_array().unwrap();
    let ids = listings
        .iter()
        .map(|entry| entry["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        [
            "html",
            "javascript",
            "markdown",
            "rust",
            "svg",
            "toml",
            "xml"
        ]
    );
    let previous = rust_i18n::locale().to_string();
    for (index, listing) in listings.iter().enumerate() {
        let locale = if index % 2 == 0 { "zh-CN" } else { "en" };
        rust_i18n::set_locale(locale);
        cx.update(|cx| {
            gpui_kit::init(cx);
            typography::init(cx);
            apply_theme(builtin_theme(index % 2 == 1), cx);
            cx.set_reduce_motion(true);
        });
        let id = listing["id"].as_str().unwrap();
        let release = &listing["versions"][0];
        let filename = format!("{id}-{}.zip", release["version"].as_str().unwrap());
        let bytes = std::fs::read(fixture.join(filename)).unwrap();
        let package = Package::from_bytes(&bytes).unwrap();
        assert_eq!(package.digest, release["sha256"].as_str().unwrap());
        let one = serde_json::json!({"schema": catalog["schema"], "plugins": [listing]});
        let server = Server::from_bytes(&serde_json::to_vec(&one).unwrap(), bytes);
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join(".runtime-plugin-test");
        let worker = Arc::new(Worker::start_background(
            root.clone(),
            Environment {
                os: std::env::consts::OS.into(),
                workspace: home.path().display().to_string(),
                ..Default::default()
            },
            true,
        ));
        wait(&worker, |state| state.ready);
        worker.refresh_market(
            root.join("marketplace/catalog.json"),
            format!("{}/catalog.json", server.url),
        );
        wait(&worker, |state| state.market.fresh());
        let workspace = Workspace::open(home.path()).unwrap();
        let slot = Rc::new(std::cell::RefCell::new(None));
        let captured = slot.clone();
        let (_, editor_cx) = cx.add_window_view(move |window, cx| {
            let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
            *captured.borrow_mut() = Some(app.clone());
            Root::new(app, window, cx)
        });
        let app = slot.borrow_mut().take().unwrap();
        let before = editor_cx.update(|_, cx| cx.windows());
        let panel = editor_cx.update(|window, cx| {
            let panel = app.read(cx).extensions.clone();
            panel.update(cx, |panel, cx| {
                panel.worker = worker.clone();
                panel.poll(cx);
            });
            app.update(cx, |app, cx| app.toggle_extensions(window, cx));
            panel.update(cx, |panel, cx| {
                panel.manager_market = true;
                cx.notify();
            });
            panel
        });
        let window = editor_cx
            .update(|_, cx| cx.windows())
            .into_iter()
            .find(|window| !before.contains(window))
            .unwrap();
        let form = VisualTestContext::from_window(window, editor_cx).into_mut();
        form.simulate_resize(size(px(1160.), px(840.)));
        form.run_until_parked();
        form.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(
            server.downloads.load(Ordering::Acquire),
            0,
            "{id}: browsing"
        );
        let install = form.debug_bounds("market-install").expect(id);
        form.simulate_click(install.center(), Default::default());
        form.run_until_parked();
        form.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(server.downloads.load(Ordering::Acquire), 0, "{id}: consent");
        let confirm = form.debug_bounds("confirm-market-install").expect(id);
        let started = Instant::now();
        form.simulate_click(confirm.center(), Default::default());
        // Installation and pure language workers compile the real component independently in debug builds.
        wait_with_budget(&worker, Duration::from_secs(180), |state| {
            state.progress.is_none()
        });
        let state = worker.state.lock().unwrap();
        assert!(
            state.status.is_none(),
            "{id}: {:?}",
            state.status.as_ref().map(|s| &s.message)
        );
        if package.manifest.component.is_some() {
            assert!(
                state.instance_epochs.contains_key(id),
                "{id}: real WASM activation"
            );
        }
        drop(state);
        form.update(|_, cx| panel.update(cx, |panel, cx| panel.poll(cx)));
        let records = Manager::read_registry(&root).unwrap();
        assert_eq!(records.len(), 1, "{id}: isolated first installation");
        let installed = &records[id];
        assert_eq!(
            serde_json::to_value(&installed.manifest).unwrap(),
            serde_json::to_value(&package.manifest).unwrap(),
            "{id}: identity and contract"
        );
        assert_eq!(installed.digest, package.digest, "{id}: full ZIP digest");
        assert_eq!(
            installed.grants, package.manifest.permissions,
            "{id}: explicit consent"
        );
        assert!(installed.enabled);
        assert_eq!(
            server.downloads.load(Ordering::Acquire),
            1,
            "{id}: exactly one download"
        );
        verify_language(id, home.path());
        eprintln!(
            "official native first install: {id} {} {} ({locale}, {:.2}s)",
            installed.manifest.version,
            installed.digest,
            started.elapsed().as_secs_f64()
        );
        worker.tx.send(Work::Shutdown(None)).unwrap();
    }
    rust_i18n::set_locale(&previous);
}

/// Recognition and actual grammar captures must come from the just-installed public contribution.
fn verify_language(id: &str, workspace: &std::path::Path) {
    let (extension, source) = match id {
        "html" => ("html", "<main class=\"content\">hello</main>"),
        "javascript" => ("js", "const answer = 42;"),
        "markdown" => ("md", "# Heading\n\n**content**\n"),
        "rust" => ("rs", "fn main() { let answer = 42; }"),
        "toml" => ("toml", "[package]\nname = \"sample\"\n"),
        "xml" => ("xml", "<root id=\"中文🙂\"><item/></root>"),
        "svg" => return, // Native image drawing has its own delivered-package acceptance test.
        _ => panic!("unexpected official fixture: {id}"),
    };
    let language = crate::language::providers::language_for_path(
        &workspace.join(format!("sample.{extension}")),
    )
    .expect(id);
    let selection = crate::language::code_highlighting::selected(&language).expect(id);
    let tokens = crate::language::code_highlighting::highlight(
        &selection,
        source,
        &std::sync::atomic::AtomicBool::new(false),
        Instant::now() + Duration::from_secs(30),
    );
    assert!(
        !tokens.is_empty(),
        "{id}: installed grammar must emit real captures"
    );
}
