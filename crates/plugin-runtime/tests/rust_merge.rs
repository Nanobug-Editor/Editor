//! Unified public-SDK Rust package acceptance: offline Cargo, lazy preparation, replacement and load cost.
#![cfg(windows)]
/// Explicit consumption replaces retired eager installation assumptions in real package regressions.
#[path = "support/demand.rs"]
mod demand_support;
/// Historical migration inputs retain their exact bytes and versions, independently of the removed source trees.
#[path = "support/packages.rs"]
mod frozen_packages;
#[path = "support/debugger_packages.rs"]
mod packages;
use plugin_runtime::{
    DemandReason, InstallControl, Manager, Package, TargetRequest,
    plugin_protocol::{
        Environment,
        api::{self, RequestUpdate},
        settings::Scope,
    },
};
use serde_json::{Value, json};
use std::{
    path::Path,
    time::{Duration, Instant},
};

/// Minimal Cargo metadata stays offline and independent from installed user projects.
fn project(root: &Path) {
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"merge-probe\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    )
    .unwrap();
    std::fs::write(root.join("src/main.rs"), "fn main() { println!(\"ok\"); }").unwrap();
}
fn manager(root: &Path) -> Manager {
    Manager::open(
        root.join("plugins"),
        Environment {
            workspace: root.display().to_string(),
            os: "windows".into(),
            ..Default::default()
        },
    )
    .unwrap()
}
/// Drive the existing public target request until genuine metadata or build completion.
fn answer(manager: &mut Manager, request: TargetRequest) -> Value {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        match request.status() {
            RequestUpdate::Completed { result } => return result.unwrap(),
            RequestUpdate::Cancelled { reason, .. } => panic!("Cargo cancelled: {reason:?}"),
            _ => {
                assert!(Instant::now() < deadline);
                manager.poll();
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}
fn discover(manager: &mut Manager, root: &Path) -> Value {
    let request = manager
        .begin_target_call(
            "rust",
            "discover",
            json!({"workspace":root.display().to_string()}),
        )
        .unwrap();
    answer(manager, request)
}
/// Repackage only a declared source; WASM, bridge/hash and public contracts stay genuine.
fn source_package(source: Value) -> Package {
    let package = packages::rust_package("rust");
    let mut manifest: Value = serde_json::from_slice(&package.files["manifest.json"]).unwrap();
    manifest["services"]["adapter"]["installation"]["artifacts"][0]["source"] = source;
    packages::package(package.files, manifest)
}

#[test]
#[ignore = "build rust-0.6.1 through --plugin-package and prepare pinned CodeLLDB VSIX"]
fn lazy_failure_keeps_cargo_and_the_language_contributions_available() {
    let root = tempfile::tempdir().unwrap();
    project(root.path());
    let package = source_package(
        json!({"kind":"local", "path":root.path().join("missing.vsix").display().to_string()}),
    );
    let mut manager = manager(root.path());
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    assert!(
        !manager.live.contains_key("rust"),
        "installation stays cold"
    );
    assert_eq!(manager.installed["rust"].manifest.language_servers.len(), 1);
    assert!(package.files.contains_key("grammar/rust.wasm"));
    let request = manager
        .begin_debug_call("start", json!({"program":"missing.exe","args":[]}))
        .unwrap();
    // The first explicit debug request includes cold guest compilation within its original deadline.
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        manager.poll();
        if let RequestUpdate::Completed { result } = request.status() {
            assert!(result.unwrap_err().message.contains("preparation"));
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    let targets = discover(&mut manager, root.path());
    assert!(!targets["targets"].as_array().unwrap().is_empty());
    assert_eq!(manager.live["rust"].process_count(), 0);
    assert!(manager.installed["rust"].error.is_none());
    manager.shutdown();
}

#[test]
#[ignore = "build rust-0.6.1 through --plugin-package and prepare pinned CodeLLDB VSIX"]
fn cancelled_dependency_download_allows_concurrent_cargo_and_never_starts_a_target() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::mpsc,
    };
    let root = tempfile::tempdir().unwrap();
    project(root.path());
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let url = format!("http://{}/adapter", listener.local_addr().unwrap());
    let (connected, receiver) = mpsc::channel();
    let (release, stop) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let _ = socket.read(&mut [0; 2048]).unwrap();
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 50943475\r\nConnection: close\r\n\r\n")
            .unwrap();
        connected.send(()).unwrap();
        let _ = stop.recv_timeout(Duration::from_secs(30));
    });
    let package = source_package(json!({"kind":"url","url":url}));
    let mut manager = manager(root.path());
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    // Installation succeeds without contacting the source. Only the explicit start begins acquisition.
    assert!(receiver.try_recv().is_err());
    let request = manager
        .begin_debug_call("start", json!({"program":"missing.exe","args":[]}))
        .unwrap();
    // Cold birth may compile the real WASM before the retained start begins dependency acquisition.
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        manager.poll();
        if receiver.try_recv().is_ok() {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !discover(&mut manager, root.path())["targets"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    manager.force_debug_session(request.session()).unwrap();
    manager.poll();
    release.send(()).unwrap();
    server.join().unwrap();
    assert!(matches!(request.status(), RequestUpdate::Cancelled { .. }));
    assert_eq!(manager.live["rust"].process_count(), 0);
    assert!(manager.installed["rust"].error.is_none());
    manager.shutdown();
}

#[test]
#[ignore = "build rust-0.6.1 through --plugin-package and prepare pinned CodeLLDB VSIX"]
fn confirmed_replacement_preserves_disabled_scope_choices_and_private_data() {
    let root = tempfile::tempdir().unwrap();
    project(root.path());
    let old = legacy_debugger("retired-debugger");
    let mut manager = manager(root.path());
    manager
        .install(&old, old.manifest.permissions.clone())
        .unwrap();
    manager
        .set_service_provider(
            api::InstanceScope::Workspace,
            Scope::User,
            "debug.session",
            Some("retired-debugger"),
        )
        .unwrap();
    manager
        .set_service_provider(
            api::InstanceScope::Workspace,
            Scope::Project,
            "debug.session",
            Some("retired-debugger"),
        )
        .unwrap();
    manager.disable("retired-debugger").unwrap();
    // Original opaque bytes live outside the new package's data authority.
    let old_files = Manager::persisted_data_directory(
        &root.path().join("plugins"),
        &old.manifest,
        &root.path().display().to_string(),
    )
    .unwrap();
    std::fs::create_dir_all(&old_files).unwrap();
    std::fs::write(old_files.join("keep.bin"), b"opaque-original").unwrap();
    let package = packages::rust_package("rust");
    let mut manifest: Value = serde_json::from_slice(&package.files["manifest.json"]).unwrap();
    manifest["replaces"] = json!([{"id":"retired-debugger","versions":"^0.2","contracts":["debug.session"],"enabled_setting":"debugger.enabled"}]);
    let package = packages::package(package.files, manifest);
    assert!(
        manager
            .install(&package, package.manifest.permissions.clone())
            .is_err()
    );
    assert!(manager.installed["retired-debugger"].replaced_by.is_none());
    let control = InstallControl::default().with_replacements(["retired-debugger".into()]);
    manager
        .install_with_control(&package, package.manifest.permissions.clone(), &control)
        .unwrap();
    assert_eq!(
        manager.installed["retired-debugger"].replaced_by.as_deref(),
        Some("rust")
    );
    assert!(!manager.live.contains_key("retired-debugger"));
    assert!(manager.debug_availability().is_err());
    assert!(
        !discover(&mut manager, root.path())["targets"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        std::fs::read(old_files.join("keep.bin")).unwrap(),
        b"opaque-original"
    );
    let preferences: Value = serde_json::from_slice(
        &std::fs::read(root.path().join("plugins/service-providers.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(preferences["user"]["workspace/debug.session"], "rust");
    assert!(
        preferences["projects"]
            .as_object()
            .unwrap()
            .values()
            .all(|value| value["debug.session"] == "rust")
    );
    manager.shutdown();
    drop(manager);
    let mut reopened = self::manager(root.path());
    assert!(reopened.debug_availability().is_err());
    reopened
        .update_setting("rust", Scope::User, "debugger.enabled", Some(true.into()))
        .unwrap();
    assert_eq!(reopened.debug_availability().unwrap(), "rust");
    // Reinstall requires no replacement consent and never replaces the user's later setting.
    reopened
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    assert_eq!(reopened.debug_availability().unwrap(), "rust");
    reopened.shutdown();
}

/// A new owner must retain a fully cached plan after removing the old owner's receipt and source.
#[test]
#[ignore = "build the fixed official Rust package and prepare the pinned CodeLLDB VSIX"]
fn cached_adapter_survives_replaced_owner_uninstall_and_offline_restart() {
    let root = tempfile::tempdir().unwrap();
    let (_, binary) = packages::program(root.path());
    let base = packages::debugger("cached-old");
    let mut manifest: Value = serde_json::from_slice(&base.files["manifest.json"]).unwrap();
    let original =
        manifest["services"]["adapter"]["installation"]["artifacts"][0]["source"]["path"]
            .as_str()
            .unwrap();
    let source = root.path().join("adapter.vsix");
    std::fs::copy(original, &source).unwrap();
    manifest["services"]["adapter"]["installation"]["artifacts"][0]["source"] =
        json!({"kind":"local","path":source.display().to_string()});
    let old = packages::package(base.files, manifest.clone());
    let mut manager = manager(root.path());
    manager
        .install(&old, old.manifest.permissions.clone())
        .unwrap();
    let start = |manager: &mut Manager| {
        let request = manager.begin_debug_call("start", json!({
            "program":binary.display().to_string(),"args":[],"cwd":root.path().display().to_string(),
            "name":"Cached dependency regression","breakpoints":[]
        })).unwrap();
        debug_answer(manager, request)["session"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let session = start(&mut manager);
    let stop = manager
        .begin_debug_call("stop", json!({"session":session}))
        .unwrap();
    debug_answer(&mut manager, stop);
    // Both actual guests use identical native bytes/plans, so the second owner exercises a full cache hit.
    manifest["id"] = "cached-new".into();
    manifest["replaces"] = json!([{"id":"cached-old","versions":"^0.6", "contracts":["debug.session"],"enabled_setting":"debugger.enabled"}]);
    let next = packages::package(old.files, manifest);
    manager
        .install_with_control(
            &next,
            next.manifest.permissions.clone(),
            &InstallControl::default().with_replacements(["cached-old".into()]),
        )
        .unwrap();
    let session = start(&mut manager);
    let stop = manager
        .begin_debug_call("stop", json!({"session":session}))
        .unwrap();
    debug_answer(&mut manager, stop);
    manager.uninstall("cached-old", false).unwrap();
    std::fs::remove_file(source).unwrap();
    manager.collect_dependency_cache().unwrap();
    let session = start(&mut manager);
    let stop = manager
        .begin_debug_call("stop", json!({"session":session}))
        .unwrap();
    debug_answer(&mut manager, stop);
    assert_eq!(manager.live["cached-new"].process_count(), 0);
    manager.shutdown();
}

/// An oversized replacement must fail before commit, retaining the old provider and its opaque data.
#[test]
#[ignore = "build the fixed official Rust package through the public SDK entry"]
fn replacement_quota_failure_preserves_the_original_installation() {
    let root = tempfile::tempdir().unwrap();
    let old = packages::debugger("a");
    let mut manager = manager(root.path());
    manager
        .install(&old, old.manifest.permissions.clone())
        .unwrap();
    let store = root.path().join("plugins");
    let data = Manager::persisted_data_directory(
        &store,
        &old.manifest,
        &root.path().display().to_string(),
    )
    .unwrap();
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(data.join("keep.bin"), b"original-private-data").unwrap();
    // A legal persisted input can grow beyond quota when a short provider ID is replaced.
    // Populate independent project choices once, rather than thousands of settings API writes.
    let projects: serde_json::Map<String, Value> = (0..7000)
        .map(|index| (format!("{index:064x}"), json!({"debug.session":"a"})))
        .collect();
    let preferences = serde_json::to_vec_pretty(&json!({"user":{},"projects":projects})).unwrap();
    assert!(preferences.len() < 1024 * 1024);
    let preferences_path = store.join("service-providers.json");
    std::fs::write(&preferences_path, &preferences).unwrap();
    let mut manifest: Value = serde_json::from_slice(&old.files["manifest.json"]).unwrap();
    let next_id = format!("combined-{}", "provider".repeat(10));
    manifest["id"] = next_id.clone().into();
    manifest["replaces"] = json!([{"id":"a","versions":"^0.6", "contracts":["debug.session"],"enabled_setting":"debugger.enabled"}]);
    let next = packages::package(old.files, manifest);
    let result = manager.install_with_control(
        &next,
        next.manifest.permissions.clone(),
        &InstallControl::default().with_replacements(["a".into()]),
    );
    assert!(
        result.is_err(),
        "oversized migrated preferences must be rejected"
    );
    assert!(manager.installed["a"].replaced_by.is_none());
    assert!(!manager.installed.contains_key(&next_id));
    assert_eq!(manager.debug_availability().unwrap(), "a");
    assert_eq!(std::fs::read(&preferences_path).unwrap(), preferences);
    assert_eq!(
        std::fs::read(data.join("keep.bin")).unwrap(),
        b"original-private-data"
    );
    manager.shutdown();
    drop(manager);
    let mut reopened = self::manager(root.path());
    assert_eq!(reopened.debug_availability().unwrap(), "a");
    assert!(reopened.installed["a"].replaced_by.is_none());
    reopened.shutdown();
}

/// Wait for a real debug exchange; cached dependencies must work without reading the deleted source.
fn debug_answer(manager: &mut Manager, request: plugin_runtime::DebugRequest) -> Value {
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        match request.status() {
            RequestUpdate::Completed { result } => return result.unwrap(),
            RequestUpdate::Cancelled { .. } => panic!("debug request cancelled"),
            _ => {
                assert!(Instant::now() < deadline, "debug exchange timed out");
                manager.poll();
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

/// Use the saved pre-merge distribution, so migration covers the real 0.2.0 guest rather than an alias.
fn legacy_debugger(id: &str) -> Package {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let old = Package::read(&frozen_packages::archive("rust-debugger-0.2.0")).unwrap();
    let mut manifest: Value = serde_json::from_slice(&old.files["manifest.json"]).unwrap();
    manifest["id"] = json!(id);
    manifest["services"]["adapter"]["installation"]["artifacts"][0]["source"] = json!({"kind":"local",
        "path":repo.join("target/debug-adapters/codelldb-1.12.3/codelldb-win32-x64.vsix").canonicalize().unwrap().display().to_string()});
    packages::package(old.files, manifest)
}

#[test]
#[ignore = "save pre-merge 0.5.1/0.2.0 distributions; build unified rust and prepare the pinned VSIX"]
fn failed_replacement_restores_metadata_and_keeps_an_alternative_choice() {
    let root = tempfile::tempdir().unwrap();
    project(root.path());
    let old = legacy_debugger("retired-debugger");
    let alternative = packages::debugger("independent-debugger");
    let mut manager = manager(root.path());
    manager
        .install(&old, old.manifest.permissions.clone())
        .unwrap();
    assert!(!manager.live.contains_key("retired-debugger"));
    // This scenario needs an actual preexisting owner to prove failed replacement preserves it.
    // Consuming its declared service starts the guest without invoking the adapter's start method.
    demand_support::ready(
        &mut manager,
        "retired-debugger",
        DemandReason::Service("debug.session".into()),
    );
    manager
        .install(&alternative, alternative.manifest.permissions.clone())
        .unwrap();
    manager
        .set_service_provider(
            api::InstanceScope::Workspace,
            Scope::User,
            "debug.session",
            Some("retired-debugger"),
        )
        .unwrap();
    manager
        .set_service_provider(
            api::InstanceScope::Workspace,
            Scope::Project,
            "debug.session",
            Some("independent-debugger"),
        )
        .unwrap();
    let package = packages::rust_package("rust");
    let mut manifest: Value = serde_json::from_slice(&package.files["manifest.json"]).unwrap();
    manifest["replaces"] = json!([{"id":"retired-debugger","versions":"^0.2","contracts":["debug.session"],"enabled_setting":"debugger.enabled"}]);
    let package = packages::package(package.files, manifest);
    let path = root.path().join("plugins/service-providers.json");
    let preferences = std::fs::read(&path).unwrap();
    let original = manager.availability("retired-debugger").unwrap();
    let owner = manager.instance_id("retired-debugger").unwrap().to_owned();
    let candidate = manager
        .prepare_installation(
            &package,
            package.manifest.permissions.clone(),
            &InstallControl::default().with_replacements(["retired-debugger".into()]),
        )
        .unwrap();
    // The normal commit stage is a public fault seam; fail after journaling and before publication.
    let withdrawal = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let signal = withdrawal.clone();
    let control = InstallControl::new(move |stage| {
        if stage == plugin_runtime::InstallStage::Committing {
            signal.store(false, std::sync::atomic::Ordering::Release);
        }
    })
    .with_guard(move || withdrawal.load(std::sync::atomic::Ordering::Acquire))
    .with_replacements(["retired-debugger".into()]);
    assert!(manager.commit_installation(candidate, &control).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), preferences);
    assert!(manager.live.contains_key("retired-debugger"));
    assert_eq!(
        manager.instance_id("retired-debugger"),
        Some(owner.as_str())
    );
    assert_eq!(manager.availability("retired-debugger").unwrap(), original);
    assert!(manager.installed["retired-debugger"].replaced_by.is_none());
    assert!(!manager.installed.contains_key("rust"));
    let accepted = InstallControl::default().with_replacements(["retired-debugger".into()]);
    manager
        .install_with_control(&package, package.manifest.permissions.clone(), &accepted)
        .unwrap();
    // Durable replacement revokes the original publication and its pending-entry authority.
    assert!(
        manager
            .begin_demand(
                &original,
                DemandReason::Service("debug.session".into()),
                120_000
            )
            .is_err()
    );
    assert!(manager.instance_id("retired-debugger").is_none());
    let preferences: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(preferences["user"]["workspace/debug.session"], "rust");
    assert!(
        preferences["projects"]
            .as_object()
            .unwrap()
            .values()
            .all(|value| value["debug.session"] == "independent-debugger")
    );
    assert_eq!(
        manager.debug_availability().unwrap(),
        "independent-debugger"
    );
    manager.shutdown();
}

#[test]
#[ignore = "save pre-merge 0.5.1/0.2.0 distributions; build unified rust and prepare the pinned VSIX"]
fn compare_cold_plugin_discovery_without_language_server_or_adapter_startup() {
    let language = Package::read(&frozen_packages::archive("rust-0.5.1")).unwrap();
    let debug = legacy_debugger("rust-debugger");
    let unified = packages::rust_package("rust");
    for (label, packages) in [
        ("language-only", vec![&language]),
        ("two-plugins", vec![&language, &debug]),
        ("unified", vec![&unified]),
    ] {
        let root = tempfile::tempdir().unwrap();
        project(root.path());
        let mut runtime = manager(root.path());
        for package in &packages {
            runtime
                .install(package, package.manifest.permissions.clone())
                .unwrap();
        }
        runtime.shutdown();
        drop(runtime);
        let mut times = Vec::new();
        for _ in 0..7 {
            let start = Instant::now();
            let mut runtime = manager(root.path());
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            // Cold startup restores every installation and static contribution, without compiling
            // a component merely to improve an activation benchmark or starting native providers.
            assert!(runtime.live.is_empty());
            assert_eq!(runtime.published_entries().len(), packages.len());
            assert_eq!(runtime.resource_count(), 0);
            assert!(
                runtime
                    .installed
                    .values()
                    .all(|entry| entry.error.is_none())
            );
            assert!(
                runtime
                    .live
                    .values()
                    .all(|instance| instance.process_count() == 0)
            );
            runtime.shutdown();
        }
        times.sort_by(f64::total_cmp);
        let wasm_bytes: usize = packages
            .iter()
            .map(|package| package.files[package.manifest.component.as_ref().unwrap()].len())
            .sum();
        eprintln!(
            "cold discovery {label}: median={:.2} ms; min={:.2}; max={:.2}; wasm={wasm_bytes} bytes; installations={}",
            times[3],
            times[0],
            times[6],
            packages.len()
        );
    }
}
