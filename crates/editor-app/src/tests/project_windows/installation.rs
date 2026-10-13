//! One shared installation lane keeps both original project actors usable during real candidate preparation.
use super::*;

/// Reidentify the actual frozen guest through public ZIP inspection, preserving all ordinary SDK contracts.
fn candidate(source: &Package, version: u32, url: &str, payload: &[u8], broken: bool) -> Package {
    let mut files = source.files.clone();
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["id"] = json!("installed-side");
    manifest["version"] = json!(format!("{version}.0.0"));
    manifest["api"]["required"]["dependencies"] = json!("^1");
    manifest["permissions"]
        .as_array_mut()
        .unwrap()
        .push(json!("dependencies.prepare"));
    manifest["services"] = json!({"prepared-only": {
        "program":"unused-prepared-service", "args":[], "installation": {
            "executable":"artifact/tool.exe", "artifacts":[{
                "id":"artifact", "version":"1.0.0", "platform":format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
                "sha256":format!("{:x}",Sha256::digest(payload)), "source":{"kind":"url","url":url},
                "format":{"kind":"file","path":"tool.exe"}
            }]
        }
    }});
    if let Some(path) = manifest["contributions"].as_str() {
        let mut declaration: toml::Value =
            toml::from_str(std::str::from_utf8(&files[path]).unwrap()).unwrap();
        declaration["plugin"]["id"] = toml::Value::String("installed-side".into());
        declaration["plugin"]["version"] = toml::Value::String(format!("{version}.0.0"));
        files.insert(
            path.into(),
            toml::to_string(&declaration).unwrap().into_bytes(),
        );
    }
    if broken {
        // This is a valid component/package; the guest's actual migration hook rejects the candidate at cutover.
        manifest["data_format"] = json!({"version":2,"migration_hook":true});
        manifest["api"]["required"]["storage.migration"] = json!("^1");
        files.insert("migration-policy.txt".into(), b"fail".to_vec());
    }
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    crate::extensions::language_tests::packages::repack(files).unwrap()
}

/// Success serializes B's metadata change behind A's transaction; a failed update preserves committed data and both guests.
#[test]
#[ignore = "opt-in frozen capability-example Package/WASM and real loopback dependency acquisition"]
fn concurrent_project_installation_serializes_metadata_and_preserves_both_live_guests() {
    let store = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    std::fs::write(a.path().join("source.txt"), "workspace A").unwrap();
    std::fs::write(b.path().join("source.txt"), "workspace B").unwrap();
    let environment = |path: &std::path::Path| Environment {
        workspace: path.canonicalize().unwrap().display().to_string(),
        ..Default::default()
    };
    let package = crate::test_packages::package("capability-example");
    let mut manager = Manager::open(store.path().into(), environment(a.path())).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    drop(manager);
    let pool = Arc::new(crate::extensions::runtime_owners::RuntimePool::default());
    let first = Worker::start_background_in_pool(
        pool.clone(),
        store.path().into(),
        environment(a.path()),
        true,
    );
    let second = Worker::start_background_in_pool(
        pool.clone(),
        store.path().into(),
        environment(b.path()),
        true,
    );
    wait_for(&first, |state| state.ready);
    wait_for(&second, |state| state.ready);
    let visible = |state: &Published, text: &str| {
        state.views.values().any(
            |scene| matches!(&scene.root.kind, ui::Kind::Text { text: value } if value == text),
        )
    };
    first
        .tx
        .send(command_work(
            &first,
            "scope-write",
            json!({"text":"original A"}),
        ))
        .unwrap();
    second
        .tx
        .send(command_work(
            &second,
            "scope-write",
            json!({"text":"original B"}),
        ))
        .unwrap();
    wait_for(&first, |state| visible(state, "workspace A|original A"));
    wait_for(&second, |state| visible(state, "workspace B|original B"));

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/artifact", listener.local_addr().unwrap());
    let payload = b"unused preparation fixture";
    let (entered, waiting) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let download = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0; 4096];
        stream.read(&mut request).unwrap();
        entered.send(()).unwrap();
        let _ = gate.recv_timeout(Duration::from_secs(30));
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            payload.len()
        );
        let _ = stream.write_all(payload);
    });
    assert!(first.queue_lifecycle(Work::Install(candidate(&package, 1, &url, payload, false))));
    waiting.recv_timeout(Duration::from_secs(60)).unwrap();
    assert!(second.queue_lifecycle(Work::Disable("installed-side".into())));
    second
        .tx
        .send(command_work(
            &second,
            "scope-write",
            json!({"text":"B during A preparation"}),
        ))
        .unwrap();
    first
        .tx
        .send(command_work(
            &first,
            "scope-write",
            json!({"text":"A during preparation"}),
        ))
        .unwrap();
    wait_for(&second, |state| {
        visible(state, "workspace B|B during A preparation")
    });
    wait_for(&first, |state| {
        visible(state, "workspace A|A during preparation")
    });
    // External disk readers are deliberately excluded by the private-data transaction; native actors keep old metadata.
    assert!(
        !first
            .state
            .lock()
            .unwrap()
            .entries
            .iter()
            .any(|entry| entry.manifest.id == "installed-side")
    );
    release.send(()).unwrap();
    download.join().unwrap();
    wait_for(&first, |state| state.progress.is_none());
    wait_for(&second, |state| {
        state.progress.is_none()
            && state
                .entries
                .iter()
                .any(|entry| entry.manifest.id == "installed-side" && !entry.enabled)
    });
    let committed = Manager::read_registry(store.path()).unwrap();
    assert_eq!(committed["installed-side"].manifest.version, "1.0.0");
    assert!(
        !committed["installed-side"].enabled,
        "B's ordered metadata write must follow A's commit"
    );

    assert!(first.queue_lifecycle(Work::Install(candidate(&package, 2, &url, payload, true))));
    wait_for(&first, |state| {
        state.progress.is_none()
            && state
                .status
                .as_ref()
                .is_some_and(|status| status.plugin.as_deref() == Some("installed-side"))
    });
    assert!(
        !first
            .state
            .lock()
            .unwrap()
            .installation
            .as_ref()
            .unwrap()
            .installed
    );
    assert_eq!(
        Manager::read_registry(store.path()).unwrap()["installed-side"]
            .manifest
            .version,
        "1.0.0"
    );
    first
        .tx
        .send(command_work(
            &first,
            "scope-write",
            json!({"text":"A after failed candidate"}),
        ))
        .unwrap();
    second
        .tx
        .send(command_work(
            &second,
            "scope-write",
            json!({"text":"B after failed candidate"}),
        ))
        .unwrap();
    wait_for(&first, |state| {
        visible(state, "workspace A|A after failed candidate")
    });
    wait_for(&second, |state| {
        visible(state, "workspace B|B after failed candidate")
    });
    futures::executor::block_on(first.request_shutdown().unwrap())
        .unwrap()
        .unwrap();
    futures::executor::block_on(second.request_shutdown().unwrap())
        .unwrap()
        .unwrap();
    pool.shutdown().unwrap();
}
