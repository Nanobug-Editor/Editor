//! The real Vue Package makes compatibility decisions through public SDK service calls.
//! A controlled TS provider supplies policy input only; actual compiler semantics are tested by the app.
use plugin_runtime::{
    DemandReason, Manager, Package,
    plugin_protocol::{Environment, api, settings::Scope},
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// An unknown consumer must receive explainable version evidence from the production Rust guest.
#[test]
#[ignore = "set NANOBUG_VUE_PACKAGE and both NANOBUG_VUE_POLICY_{PROVIDER,CONSUMER}_PACKAGE to current SDK packages"]
fn real_vue_package_reports_its_selected_versions_without_attaching_native_modules() {
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("node_modules/vue")).unwrap();
    std::fs::write(
        root.join("package.json"),
        r#"{"dependencies":{"vue":"^2.7.0"}}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("node_modules/vue/package.json"),
        r#"{"name":"vue","version":"2.7.16"}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("App.vue"),
        "<template><p>policy probe</p></template>",
    )
    .unwrap();
    let profile = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(
        profile.path().into(),
        Environment {
            workspace: root.display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    for variable in [
        "NANOBUG_VUE_POLICY_PROVIDER_PACKAGE",
        "NANOBUG_VUE_PACKAGE",
        "NANOBUG_VUE_POLICY_CONSUMER_PACKAGE",
    ] {
        let package = Package::read(&input(variable)).unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
    }
    // Start at the public configuration boundary; an undeclared target fails before service calls.
    manager
        .update_setting(
            "vue",
            Scope::Project,
            "vue.targetVersion",
            Some(json!("2.7.16")),
        )
        .expect("Vue declares an exact target setting through the public configuration contract");
    activate(&mut manager, "controlled-typescript-context", "set-context");
    manager.invoke_command("controlled-typescript-context", "set-context", json!({
        "context":"controlled-project-reference", "project":"controlled-project", "generation":17,
        "kind":"configured", "root":root, "config":root.join("tsconfig.json"),
        "engineVersion":"5.9.3", "features":["completion","diagnostics","hover","definition","references","rename","formatting"]
    })).unwrap();
    activate(&mut manager, "unknown-vue-policy-consumer", "run");
    let receipt = query(&mut manager, json!({"feature":"compatibility"}));
    assert_eq!(
        receipt["state"], "complete",
        "actual Vue policy query failed: {receipt}"
    );
    let encoded = receipt["value"]["payload"]
        .as_str()
        .expect("bounded public payload");
    let decision: Value = serde_json::from_str(encoded).unwrap();
    assert_eq!(decision["state"], "candidate");
    assert_eq!(decision["version"], "2.7.16");
    assert_eq!(decision["record"], "vue27-legacy");
    assert_eq!(decision["backend"]["version"], "3.0.11");
    assert_eq!(decision["engine"]["version"], "5.9.3");
    assert!(
        decision["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["kind"] == "installed"
                && item["path"] == "node_modules/vue/package.json"
                && item["version"] == "2.7.16")
    );
    assert_eq!(
        snapshot(&mut manager, "controlled-typescript-context")["attaches"],
        0
    );
    assert_eq!(
        std::fs::read_to_string(root.join("App.vue")).unwrap(),
        "<template><p>policy probe</p></template>"
    );

    // An explicit target can explain missing evidence, but cannot hide installed contradictions.
    // The setting uses the ordinary host-confirmed project configuration API, never caller payload.
    manager
        .update_setting(
            "vue",
            Scope::Project,
            "vue.targetVersion",
            Some(json!("3.5.22")),
        )
        .expect("Vue declares an exact target setting through the public configuration contract");
    std::fs::write(
        root.join("package.json"),
        r#"{"dependencies":{"vue":"^3.0.0"}}"#,
    )
    .unwrap();
    std::fs::remove_file(root.join("node_modules/vue/package.json")).unwrap();
    let missing = query_decision(&mut manager);
    assert_eq!(missing["state"], "dependencies-missing");
    assert_eq!(missing["version"], "3.5.22");
    assert_eq!(missing["backend"]["export"], "modern");
    assert!(
        missing["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["kind"] == "override"
                && item["version"] == "3.5.22"
                && item["source"] == "project")
    );
    std::fs::write(
        root.join("node_modules/vue/package.json"),
        r#"{"name":"vue","version":"2.7.16"}"#,
    )
    .unwrap();
    let conflict = query_decision(&mut manager);
    assert_eq!(conflict["state"], "conflict");
    for (kind, version) in [("override", "3.5.22"), ("installed", "2.7.16")] {
        assert!(
            conflict["evidence"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["kind"] == kind && item["version"] == version)
        );
    }
    let rejected = query(
        &mut manager,
        json!({"feature":"quickinfo","arguments":{"line":1,"offset":1}}),
    );
    assert_eq!(rejected["state"], "failed");
    assert_eq!(rejected["error"]["code"], "conflict");
    assert_eq!(
        snapshot(&mut manager, "controlled-typescript-context")["attaches"],
        0
    );
    unresolved_and_historical_matrix(&mut manager, &root);
    context_and_link_boundaries(&mut manager, &root);
    assert_eq!(
        snapshot(&mut manager, "controlled-typescript-context")["attaches"],
        0
    );
    assert_eq!(
        std::fs::read_to_string(root.join("App.vue")).unwrap(),
        "<template><p>policy probe</p></template>"
    );
}

/// One live consumer observes missing evidence, retained historical gaps and conflicting locks.
fn unresolved_and_historical_matrix(manager: &mut Manager, root: &Path) {
    manager
        .update_setting("vue", Scope::Project, "vue.targetVersion", None)
        .unwrap();
    std::fs::write(
        root.join("package.json"),
        r#"{"dependencies":{"vue":"^3.0.0"}}"#,
    )
    .unwrap();
    std::fs::remove_file(root.join("node_modules/vue/package.json")).unwrap();
    let unresolved = query_decision(manager);
    assert_eq!(unresolved["state"], "unresolved");
    assert!(
        unresolved["backend"].is_null(),
        "missing evidence must not select the latest backend"
    );

    std::fs::write(
        root.join("package.json"),
        r#"{"dependencies":{"vue":"1.0.28"}}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("node_modules/vue/package.json"),
        r#"{"name":"vue","version":"1.0.28"}"#,
    )
    .unwrap();
    let historical = query_decision(manager);
    assert_eq!(historical["state"], "unsupported");
    assert_eq!(historical["version"], "1.0.28");
    assert!(
        historical["backend"].is_null(),
        "an unaccepted historical version has no fallback backend"
    );
    let strict = query(
        manager,
        json!({"feature":"quickinfo","arguments":{"line":1,"offset":1}}),
    );
    assert_eq!(strict["error"]["code"], "capability_unavailable");

    // The actual installed version is authoritative evidence, but a contradictory lock remains visible.
    std::fs::write(
        root.join("package.json"),
        r#"{"dependencies":{"vue":"^2.7.0"}}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("node_modules/vue/package.json"),
        r#"{"name":"vue","version":"2.7.16"}"#,
    )
    .unwrap();
    std::fs::write(root.join("pnpm-lock.yaml"), "lockfileVersion: '9.0'\nimporters:\n  .:\n    dependencies:\n      vue:\n        specifier: ^2.7.0\n        version: 3.5.22\n").unwrap();
    let locked = query_decision(manager);
    assert_eq!(locked["state"], "conflict");
    assert!(
        locked["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["kind"] == "lock"
                && item["path"] == "pnpm-lock.yaml"
                && item["importer"] == "."
                && item["version"] == "3.5.22")
    );
    std::fs::remove_file(root.join("pnpm-lock.yaml")).unwrap();

    // A scoped npm alias names metadata only; it does not lend an arbitrary native module path.
    std::fs::create_dir_all(root.join("node_modules/@compat/vue-legacy")).unwrap();
    std::fs::write(
        root.join("node_modules/@compat/vue-legacy/package.json"),
        r#"{"name":"vue","version":"2.7.16"}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("package.json"),
        r#"{"dependencies":{"@compat/vue-legacy":"npm:vue@2.7.16"}}"#,
    )
    .unwrap();
    let alias = query_decision(manager);
    assert_eq!(alias["state"], "candidate");
    assert!(
        alias["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["kind"] == "installed"
                && item["path"] == "node_modules/@compat/vue-legacy/package.json")
    );
    std::fs::write(
        root.join("package.json"),
        r#"{"dependencies":{"vue":"^2.7.0"}}"#,
    )
    .unwrap();

    // The host validates a replacement without committing malformed explicit configuration.
    let before = manager.effective_settings("vue").unwrap();
    assert!(
        manager
            .update_setting(
                "vue",
                Scope::Project,
                "vue.targetVersion",
                Some(json!("^3.0.0"))
            )
            .is_err()
    );
    assert_eq!(manager.effective_settings("vue").unwrap(), before);
    assert_eq!(query_decision(manager)["state"], "candidate");
}

/// Context roots and linked package metadata retain the consumer's original workspace authority.
fn context_and_link_boundaries(manager: &mut Manager, root: &Path) {
    set_context(manager, root, "5.8.3");
    let engine = query_decision(manager);
    assert_eq!(engine["state"], "engine-incompatible");
    assert_eq!(engine["engine"]["version"], "5.8.3");
    assert_eq!(engine["backend"]["typescript"], "5.9.3");
    let strict = query(
        manager,
        json!({"feature":"quickinfo","arguments":{"line":1,"offset":1}}),
    );
    assert_eq!(strict["error"]["code"], "capability_unavailable");
    let outside = tempfile::tempdir().unwrap();
    let outside_root = outside.path().canonicalize().unwrap();
    set_context(manager, &outside_root, "5.9.3");
    let rejected = query(manager, json!({"feature":"compatibility"}));
    assert_eq!(rejected["error"]["code"], "invalid_request");
    assert!(
        rejected["error"]["message"]
            .as_str()
            .unwrap()
            .contains("outside the workspace")
    );
    set_context(manager, root, "5.9.3");

    #[cfg(windows)]
    {
        // Real junctions follow the same public metadata reader as normal dependencies. Cleanup
        // removes only each junction entry before either temporary directory can be recursively dropped.
        std::fs::remove_file(root.join("node_modules/vue/package.json")).unwrap();
        std::fs::remove_dir(root.join("node_modules/vue")).unwrap();
        let store = root.join("package-store/vue");
        std::fs::create_dir_all(&store).unwrap();
        let metadata = r#"{"name":"vue","version":"2.7.16"}"#;
        std::fs::write(store.join("package.json"), metadata).unwrap();
        {
            let _link = Junction::new(root.join("node_modules/vue"), &store);
            assert_eq!(query_decision(manager)["state"], "candidate");
            assert_eq!(
                std::fs::read_to_string(store.join("package.json")).unwrap(),
                metadata
            );
        }
        std::fs::write(outside_root.join("package.json"), metadata).unwrap();
        {
            let _link = Junction::new(root.join("node_modules/vue"), &outside_root);
            let escaped = query(manager, json!({"feature":"compatibility"}));
            assert_eq!(escaped["state"], "failed");
            assert_eq!(escaped["error"]["code"], "invalid_path");
            assert_eq!(
                std::fs::read_to_string(outside_root.join("package.json")).unwrap(),
                metadata
            );
        }
    }
}

/// A controlled provider reports only policy metadata; this command does not start a TS engine.
fn set_context(manager: &mut Manager, root: &Path, engine: &str) {
    manager.invoke_command("controlled-typescript-context", "set-context", json!({
        "context":"controlled-project-reference", "project":"controlled-project", "generation":17,
        "kind":"configured", "root":root, "config":root.join("tsconfig.json"),
        "engineVersion":engine,"features":["completion","diagnostics","hover","definition","references","rename","formatting"]
    })).unwrap();
}

/// A test-owned Windows reparse entry is always removed without traversing its target.
#[cfg(windows)]
struct Junction(PathBuf);
#[cfg(windows)]
impl Junction {
    /// Arguments remain fixed code; generated test paths travel only as native environment values.
    fn new(link: PathBuf, target: &Path) -> Self {
        use std::os::windows::process::CommandExt;
        let guard = Self(link);
        // PowerShell accepts a verbatim target but creates an unreadable reparse entry for it.
        // Keep the consumer's canonical root; only the fixture construction uses a Win32 target.
        let target_text = target.to_str().expect("temporary test paths are UTF-8");
        let win32_target = if let Some(unc) = target_text.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{unc}")
        } else {
            target_text
                .strip_prefix(r"\\?\")
                .unwrap_or(target_text)
                .to_owned()
        };
        let result = std::process::Command::new("powershell.exe")
            .creation_flags(0x08000000)
            .args(["-NoProfile", "-NonInteractive", "-Command", "$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:NANOBUG_VUE_POLICY_LINK -Value $env:NANOBUG_VUE_POLICY_TARGET | Out-Null"])
            .env("NANOBUG_VUE_POLICY_LINK", &guard.0).env("NANOBUG_VUE_POLICY_TARGET", win32_target)
            .output().unwrap();
        assert!(
            result.status.success(),
            "junction creation failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            guard
                .0
                .canonicalize()
                .expect("junction is natively readable before the public API probe"),
            target.canonicalize().unwrap(),
            "junction resolves to the intended test-owned target"
        );
        guard
    }
}
#[cfg(windows)]
impl Drop for Junction {
    fn drop(&mut self) {
        use std::os::windows::fs::MetadataExt;
        if std::fs::symlink_metadata(&self.0)
            .is_ok_and(|metadata| metadata.file_attributes() & 0x400 != 0)
        {
            let _ = std::fs::remove_dir(&self.0);
        }
    }
}

/// A readonly compatibility response retains both successful and unresolved policy observations.
#[track_caller]
fn query_decision(manager: &mut Manager) -> Value {
    let receipt = query(manager, json!({"feature":"compatibility"}));
    assert_eq!(
        receipt["state"], "complete",
        "policy explanation failed: {receipt}"
    );
    serde_json::from_str(receipt["value"]["payload"].as_str().unwrap()).unwrap()
}

/// Explicit environment inputs prevent an older retained fixture silently substituting for current code.
fn input(name: &str) -> PathBuf {
    std::env::var_os(name).map(PathBuf::from).expect(name)
}

/// Cold metadata installation becomes execution only under an actual declared command demand.
fn activate(manager: &mut Manager, plugin: &str, command: &str) {
    let availability = manager.availability(plugin).unwrap();
    let demand = manager
        .begin_demand(
            &availability,
            DemandReason::Command(command.into()),
            120_000,
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        manager.poll();
        match demand.status() {
            api::RequestUpdate::Completed { result } => {
                manager
                    .admit_receipt(&availability, &result.unwrap())
                    .unwrap();
                return;
            }
            api::RequestUpdate::Cancelled { reason, .. } => {
                panic!("probe activation cancelled: {reason:?}")
            }
            _ => assert!(
                Instant::now() < deadline,
                "policy probe activation timed out"
            ),
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A normal guest snapshot exposes immutable results without any production test-only API.
fn snapshot(manager: &mut Manager, plugin: &str) -> Value {
    let snapshot = manager.live.get_mut(plugin).unwrap().snapshot().unwrap();
    serde_json::from_str(&snapshot.data).unwrap()
}

/// The public request traverses the broker, controlled context provider and actual Vue guest.
fn query(manager: &mut Manager, payload: Value) -> Value {
    manager
        .invoke_command(
            "unknown-vue-policy-consumer",
            "run",
            json!({
                "document":{"path":"App.vue","generation":0}, "payload":payload.to_string()
            }),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        manager.poll();
        let receipt = snapshot(manager, "unknown-vue-policy-consumer")["receipt"].clone();
        if receipt["state"] == "complete" || receipt["state"] == "failed" {
            return receipt;
        }
        assert!(
            Instant::now() < deadline,
            "public compatibility query timed out: {receipt}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
