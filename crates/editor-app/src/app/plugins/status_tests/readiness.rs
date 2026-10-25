//! A real stdio service must stay ready across publication and subsequent document demand.
use super::*;
use std::time::Instant;

/// Repackage the public declarative fixture with the explicitly built native executable and wire log.
fn native_package(log: &Path) -> Package {
    let executable = std::env::var_os("NANOBUG_LSP_FIXTURE")
        .map(PathBuf::from)
        .expect("set NANOBUG_LSP_FIXTURE to the built lsp_fixture executable")
        .canonicalize()
        .unwrap();
    let mut files = crate::tests::language_service_package().files;
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["services"]["analysis"] = serde_json::json!({"program": executable, "args": [log]});
    manifest["language_servers"][0]["readiness"] = serde_json::json!({
        "notification": "fixture/status", "pointer": "/state", "expected": "ready", "timeout_ms": 5000
    });
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        archive
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(&bytes).unwrap();
    }
    Package::from_bytes(&archive.finish().unwrap().into_inner()).unwrap()
}

/// Usable native requests and a refreshed footer must agree after another document joins the same service.
#[gpui::test]
#[ignore = "build plugin-runtime example lsp_fixture and set NANOBUG_LSP_FIXTURE to that executable"]
fn ready_native_service_stays_ready_after_publication_and_another_document(
    cx: &mut TestAppContext,
) {
    with_status(cx, 0, "en", false, |form, app, manager| {
        let root = form.update(|_, cx| app.read(cx).workspace.root().to_owned());
        let package = native_package(&root.join("wire.jsonl"));
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        crate::extensions::lsp_tests::publish(app, manager, form);
        let path = root.join("first.fixture");
        std::fs::write(&path, "fixture").unwrap();
        form.update(|window, cx| app.update(cx, |app, cx| app.open_file(path.clone(), window, cx)));
        crate::extensions::lsp_tests::publish(app, manager, form);
        let server = form.update(|_, cx| app.read(cx).language_servers["fixture"].clone());
        server.prepare_until_ready().unwrap();
        let completions = server
            .completions(
                language_navigation::file_uri(&path).unwrap(),
                "fixture".into(),
                lsp_types::Position::new(0, 1),
            )
            .unwrap();
        assert!(
            serde_json::to_string(&completions)
                .unwrap()
                .contains("fixture-completion")
        );
        // Let the real asynchronous handshake completion publish its visible result.
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            draw(form);
            if form.debug_bounds("service-starting-indicator").is_none() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "a usable native service must stop showing startup"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        crate::extensions::lsp_tests::publish(app, manager, form);
        draw(form);
        assert!(
            form.debug_bounds("service-starting-indicator").is_none(),
            "republishing the same usable service must not restore its startup indicator"
        );
        let second = root.join("second.fixture");
        std::fs::write(&second, "fixture two").unwrap();
        form.update(|window, cx| app.update(cx, |app, cx| app.open_file(second, window, cx)));
        crate::extensions::lsp_tests::publish(app, manager, form);
        draw(form);
        assert!(
            form.debug_bounds("service-starting-indicator").is_none(),
            "another document must reuse readiness of the same live service"
        );
        assert!(Arc::ptr_eq(
            &server,
            &form.update(|_, cx| app.read(cx).language_servers["fixture"].clone())
        ));
    });
}
