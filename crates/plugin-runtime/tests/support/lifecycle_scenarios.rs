//! T32/T34 enter actual SDK archives, public activation receipts, HTTP/timers and managed recovery.
use super::*;

/// Changed must affect the running consumer, not merely its stored settings or a replacement instance.
#[cfg(windows)]
#[test]
#[ignore = "build cloud-translation/code-host-query 0.1.2 through current host --plugin-package; retain delivered 0.1.1 archives"]
fn t32_sdk_cloud_hot_configuration_changes_live_io_and_preserves_scope_and_validation() {
    let translation = endpoint::Server::new(18906);
    let query = endpoint::Server::new(18907);
    let mut credentials = packages::Credentials::new();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(
        credentials.root.path().into(),
        legacy::environment(a.path()),
    )
    .unwrap();
    // GET carries its query in an ASCII header; POST and both responses retain UTF-8 coverage.
    for (id, server, port, method, query_text) in [
        (
            "cloud-translation",
            &translation,
            18906,
            "POST",
            "hello 世界😀",
        ),
        ("code-host-query", &query, 18907, "GET", "Nanobug"),
    ] {
        let package = packages::package(id, "0.1.2", |_| {});
        assert_eq!(
            package.manifest.api.as_ref().unwrap().required["configuration"],
            semver::VersionReq::parse(">=1.1,<2").unwrap()
        );
        assert!(package.manifest.settings_hook);
        legacy::install(&mut manager, &package);
        assert!(
            manager.instance_id(id).is_none(),
            "A default-demand package must remain cold after install"
        );
        demand(&mut manager, id, "query");
        let owner = manager.instance_id(id).unwrap().to_string();
        for (key, token) in [
            ("api-token", "fake-lifecycle-default"),
            ("alternate-token", "fake-lifecycle-alternate"),
        ] {
            credentials.register(id, key);
            legacy::probe(
                &mut manager,
                id,
                api::Operation::Secrets {
                    operation: secrets::Operation::Write {
                        key: key.into(),
                        bytes: token.as_bytes().to_vec(),
                    },
                },
            )
            .unwrap();
        }
        let baseline = manager.resource_count();
        // Give an already admitted stream enough time for isolated validation; no timeout rollback is promised.
        manager
            .update_setting(id, Scope::User, "timeout", Some(json!(120000)))
            .unwrap();
        assert_eq!(
            manager.instance_id(id),
            Some(owner.as_str()),
            "Hot configuration must preserve its owner"
        );
        manager
            .invoke_command(
                id,
                "query",
                json!({"url":format!("http://127.0.0.1:{port}/held"),"text":query_text}),
            )
            .unwrap();
        pump_until(&mut manager, id, "held-request-start", server, |_| {
            server.count() == 1
        });
        let retained = manager.resource_count();
        assert!(retained > baseline);
        manager
            .update_setting(id, Scope::Project, "poll_interval", Some(json!(1000)))
            .unwrap();
        assert_eq!(manager.instance_id(id), Some(owner.as_str()));
        assert_eq!(
            manager.resource_count(),
            retained,
            "Changing a timer interval must not recreate transfer resources"
        );
        server.release();
        pump_for(&mut manager, Duration::from_millis(250));
        assert!(
            !legacy::text(&manager, id).contains("Complete"),
            "The old 25 ms poll must not survive Changed"
        );
        pump_until(
            &mut manager,
            id,
            "held-complete-after-hot-poll",
            server,
            |manager| legacy::text(manager, id).contains("Complete"),
        );
        assert!(legacy::text(&manager, id).contains("hot 世界😀"));
        assert_eq!(manager.resource_count(), baseline);
        let first = server.capture(0);
        assert_eq!(first.method, method);
        assert_eq!(first.path, "/held");
        assert!(
            first.uses_token("fake-lifecycle-default"),
            "First request must resolve the default own key"
        );
        if method == "POST" {
            assert_eq!(first.body, "hello 世界😀".as_bytes());
        } else {
            assert!(first.body.is_empty());
        }
        // This integer passes the schema; the isolated hook rejects the cross-field deadline invariant.
        let before = manager.effective_settings(id).unwrap();
        assert!(
            manager
                .update_setting(id, Scope::User, "timeout", Some(json!(200)))
                .is_err()
        );
        assert_eq!(manager.effective_settings(id).unwrap(), before);
        assert_eq!(manager.instance_id(id), Some(owner.as_str()));
        assert_eq!(manager.resource_count(), baseline);
        assert!(
            manager
                .update_setting(id, Scope::Project, "keyRef", Some(json!("foreign-key")))
                .is_err()
        );
        assert_eq!(
            manager.effective_settings(id).unwrap(),
            before,
            "Private references cannot become project grants"
        );
        manager
            .update_setting(id, Scope::Project, "poll_interval", None)
            .unwrap();
        manager
            .update_setting(id, Scope::User, "timeout", Some(json!(200)))
            .unwrap();
        // A password was offered for the old own key. Changed must revoke it before the key is replaced.
        manager.invoke_command(id, "set-token", Json::Null).unwrap();
        let input = manager
            .take_editor_requests()
            .into_iter()
            .find_map(|(owner, request)| {
                (owner == id
                    && matches!(
                        request.operation(),
                        api::EditorOperation::Interaction {
                            operation:
                                plugin_runtime::plugin_protocol::interaction::Operation::Input {
                                    password: true,
                                    ..
                                }
                        }
                    ))
                .then_some(request)
            })
            .expect("The public interaction queue must contain one password request");
        assert!(input.begin());
        manager
            .update_setting(id, Scope::User, "keyRef", Some(json!("alternate-token")))
            .unwrap();
        assert!(
            input.status().is_terminal(),
            "An old password intent cannot borrow the new secret key"
        );
        input.finish(Ok(api::EditorValue::Interaction(
            plugin_runtime::plugin_protocol::interaction::Value::Input(
                "fake-stale-password".into(),
            ),
        )));
        manager.poll();
        manager
            .invoke_command(
                id,
                "query",
                json!({"url":format!("http://127.0.0.1:{port}/stall")}),
            )
            .unwrap();
        // Match the public SDK error-code display while retaining the strict HTTP and polling deadlines.
        let timed_out = format!("{:?}", api::ErrorCode::TimedOut);
        pump_until(
            &mut manager,
            id,
            "stall-timeout-after-key-change",
            server,
            |manager| legacy::text(manager, id).contains(timed_out.as_str()),
        );
        assert_eq!(manager.instance_id(id), Some(owner.as_str()));
        assert_eq!(manager.resource_count(), baseline);
        assert_eq!(server.capture(1).path, "/stall");
        assert!(
            server.capture(1).uses_token("fake-lifecycle-alternate"),
            "Changed must select the new private key for the next request"
        );
        // Blank is an unconfigured reference, not a request to read a global or default credential.
        manager
            .update_setting(id, Scope::User, "keyRef", Some(json!("")))
            .unwrap();
        manager
            .invoke_command(
                id,
                "query",
                json!({"url":format!("http://127.0.0.1:{port}/plain")}),
            )
            .unwrap();
        pump_until(
            &mut manager,
            id,
            "plain-complete-with-empty-key",
            server,
            |manager| legacy::text(manager, id).contains("Complete"),
        );
        assert!(server.capture(2).unauthenticated());
        manager.invoke_command(id, "set-token", Json::Null).unwrap();
        assert_eq!(
            legacy::text(&manager, id),
            "Choose a secret reference first"
        );
        assert_eq!(
            manager.resource_count(),
            baseline,
            "An empty key must not open a password request"
        );
        manager
            .invoke_command(id, "delete-token", Json::Null)
            .unwrap();
        assert_eq!(
            legacy::text(&manager, id),
            "Choose a secret reference first"
        );
        manager
            .update_setting(id, Scope::User, "timeout", Some(json!(30000)))
            .unwrap();
        manager
            .update_setting(id, Scope::Project, "poll_interval", Some(json!(1000)))
            .unwrap();
        manager
            .switch_workspace(legacy::environment(b.path()), true)
            .unwrap();
        assert_eq!(
            manager.effective_settings(id).unwrap()["poll_interval"].value,
            json!(25)
        );
        assert_eq!(
            manager.effective_settings(id).unwrap()["poll_interval"].source,
            Source::Default
        );
        demand(&mut manager, id, "query");
        manager
            .invoke_command(
                id,
                "query",
                json!({"url":format!("http://127.0.0.1:{port}/workspace-b")}),
            )
            .unwrap();
        pump_until(
            &mut manager,
            id,
            "workspace-b-complete",
            server,
            |manager| legacy::text(manager, id).contains("Complete"),
        );
        manager
            .switch_workspace(legacy::environment(a.path()), true)
            .unwrap();
        assert_eq!(
            manager.effective_settings(id).unwrap()["poll_interval"].value,
            json!(1000)
        );
        assert_eq!(
            manager.effective_settings(id).unwrap()["poll_interval"].source,
            Source::Project
        );
        demand(&mut manager, id, "cancel");
        manager.invoke_command(id, "cancel", Json::Null).unwrap();
        assert!(!legacy::text(&manager, id).contains("fake-lifecycle-"));
        assert!(
            manager
                .runtime_logs()
                .records(id)
                .iter()
                .all(|record| !record.message.contains("fake-lifecycle-"))
        );
        assert!(
            !manager
                .live
                .get_mut(id)
                .unwrap()
                .snapshot()
                .unwrap()
                .data
                .contains("fake-lifecycle-")
        );
        manager.disable(id).unwrap();
    }
    manager.shutdown();
    assert_eq!(manager.resource_count(), 0);
    assert!(manager.live.is_empty());
    assert_eq!(std::fs::read_dir(a.path()).unwrap().count(), 0);
    assert_eq!(std::fs::read_dir(b.path()).unwrap().count(), 0);
}

/// The actual query SDK rejects non-ASCII header values before transport while preserving UTF-8 responses.
#[cfg(windows)]
#[test]
#[ignore = "build code-host-query 0.1.2 through current host --plugin-package; port 18907 must be free"]
fn query_sdk_rejects_unicode_header_before_transport_and_preserves_unicode_response() {
    let server = endpoint::Server::new(18907);
    let root = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let mut manager =
        Manager::open(root.path().into(), legacy::environment(workspace.path())).unwrap();
    let id = "code-host-query";
    let package = packages::package(id, "0.1.2", |_| {});
    legacy::install(&mut manager, &package);
    assert!(manager.instance_id(id).is_none());
    demand(&mut manager, id, "query");
    let owner = manager.instance_id(id).unwrap().to_string();
    let baseline = manager.resource_count();
    let url = "http://127.0.0.1:18907/plain";

    // The guest puts text in X-Repository-Query; HTTP admission must reject this value without a socket.
    manager
        .invoke_command(id, "query", json!({"url":url,"text":"hello 世界😀"}))
        .unwrap();
    let invalid_request = format!("{:?}", api::ErrorCode::InvalidRequest);
    pump_until(
        &mut manager,
        id,
        "unicode-query-header-rejected",
        &server,
        |manager| legacy::text(manager, id).contains(invalid_request.as_str()),
    );
    assert_eq!(server.count(), 0);
    assert_eq!(manager.resource_count(), baseline);
    assert_eq!(manager.instance_id(id), Some(owner.as_str()));

    // Change only the domain query text; the same URL/owner must send one GET and decode its Unicode response.
    manager
        .invoke_command(id, "query", json!({"url":url,"text":"Nanobug"}))
        .unwrap();
    pump_until(
        &mut manager,
        id,
        "ascii-query-completes-with-unicode-response",
        &server,
        |manager| legacy::text(manager, id).contains("Complete"),
    );
    assert_eq!(server.count(), 1);
    let request = server.capture(0);
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, "/plain");
    assert!(request.body.is_empty());
    assert!(legacy::text(&manager, id).contains("hot 世界😀"));
    assert_eq!(manager.resource_count(), baseline);
    assert_eq!(manager.instance_id(id), Some(owner.as_str()));
    manager.shutdown();
    assert!(manager.live.is_empty());
    assert_eq!(manager.resource_count(), 0);
}

/// An older compiled SDK remains runnable on a stable minor; optional absence is a typed feature failure.
#[test]
#[ignore = "retain delivered cloud 0.1.1 ZIPs and run with current lifecycle host; ports 18906/18907 must be free"]
fn t34_delivered_old_sdk_minor_optional_and_required_host_floor_are_real_archives() {
    let translation = endpoint::Server::new(18906);
    let query = endpoint::Server::new(18907);
    let root = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let mut manager =
        Manager::open(root.path().into(), legacy::environment(workspace.path())).unwrap();
    for (id, server, port) in [
        ("cloud-translation", &translation, 18906),
        ("code-host-query", &query, 18907),
    ] {
        let old = packages::package(id, "0.1.1", |manifest| {
            manifest["minimum_host"] = json!("0.1.0");
            manifest["api"]["required"]["configuration"] = json!("^1");
        });
        legacy::install(&mut manager, &old);
        demand(&mut manager, id, "query");
        manager
            .invoke_command(
                id,
                "query",
                json!({"url":format!("http://127.0.0.1:{port}/old-sdk")}),
            )
            .unwrap();
        pump_until(&mut manager, id, "old-sdk-complete", server, |manager| {
            legacy::text(manager, id).contains("Complete")
        });
        assert_eq!(server.count(), 1);
        manager
            .invoke_command(id, "read-cache", Json::Null)
            .unwrap();
        assert_eq!(legacy::text(&manager, id), "hot 世界😀");
        let optional = packages::package(id, "0.1.1", |manifest| {
            manifest["version"] = json!("0.1.2");
            manifest["api"]["required"]
                .as_object_mut()
                .unwrap()
                .remove("network.http");
            manifest["api"]["optional"] = json!({"network.http":">=1.1,<2"});
        });
        legacy::install(&mut manager, &optional);
        demand(&mut manager, id, "query");
        let baseline = manager.resource_count();
        manager
            .invoke_command(
                id,
                "query",
                json!({"url":format!("http://127.0.0.1:{port}/omitted")}),
            )
            .unwrap();
        assert!(legacy::text(&manager, id).contains("CapabilityUnavailable"));
        assert!(manager.instance_id(id).is_some());
        assert_eq!(
            server.count(),
            1,
            "An omitted optional capability cannot open the socket"
        );
        assert_eq!(manager.resource_count(), baseline);
        let failure = legacy::probe(
            &mut manager,
            id,
            api::Operation::Http {
                operation: http::Operation::Start {
                    request: http::Request::get(format!("http://127.0.0.1:{port}/omitted")),
                },
            },
        )
        .unwrap_err();
        assert_eq!(failure.code, api::ErrorCode::CapabilityUnavailable);
        manager.invoke_command(id, "cancel", Json::Null).unwrap();
        assert_eq!(legacy::text(&manager, id), "Cancelled");
        // These declarations are reinspected over the same real WASM, not a fake component header.
        for rejection in ["required", "experimental_required", "minimum", "protocol"] {
            let mut files = old.files.clone();
            let mut manifest: Json = serde_json::from_slice(&files["manifest.json"]).unwrap();
            match rejection {
                "required" => manifest["api"]["required"]["configuration"] = json!(">=1.2,<2"),
                "experimental_required" => {
                    manifest["api"]["required"]["language.direct"] = json!(">=0.2,<0.3");
                }
                "minimum" => manifest["minimum_host"] = json!("0.1.1"),
                "protocol" => manifest["protocol"] = json!(6),
                _ => unreachable!(),
            }
            files.insert(
                "manifest.json".into(),
                serde_json::to_vec(&manifest).unwrap(),
            );
            assert!(
                packages::inspect(files).is_err(),
                "Expected real archive rejection: {rejection}"
            );
        }
        manager.disable(id).unwrap();
    }
    manager.shutdown();
    assert_eq!(manager.resource_count(), 0);
    assert_eq!(std::fs::read_dir(workspace.path()).unwrap().count(), 0);
}

/// Real old-SDK migration hooks preserve post-prepare writes across failure, then upgrade successfully.
#[test]
#[ignore = "retain protocol7 capability-example 0.19.0 actual SDK ZIP for lifecycle migration"]
fn t34_sdk_managed_migration_failure_recovers_latest_private_data() {
    use sha2::{Digest, Sha256};
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/plugin-api-test/capability-example.zip");
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)).to_ascii_uppercase(),
        "BDFC5D3327B5B7A4F3D25BA1AE95E2A6A47B62634FB040E17FB4430C9AA46773"
    );
    let original = Package::from_bytes(&bytes).unwrap();
    assert_eq!(original.manifest.version.to_string(), "0.19.0");
    let fixture = |version: u32, policy: &str| {
        let mut files = original.files.clone();
        let mut manifest: Json = serde_json::from_slice(&files["manifest.json"]).unwrap();
        manifest["version"] = json!(format!("{version}.0.0"));
        manifest["data_format"] = json!({"version":version,"migration_hook":true});
        manifest["api"]["required"]["storage.migration"] = json!("^1");
        files.insert(
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        );
        files.insert("migration-policy.txt".into(), policy.as_bytes().to_vec());
        packages::inspect(files).unwrap()
    };
    let root = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("source.txt"), "workspace").unwrap();
    let mut manager =
        Manager::open(root.path().into(), legacy::environment(workspace.path())).unwrap();
    let old = fixture(1, "ok");
    legacy::install(&mut manager, &old);
    demand(&mut manager, "capability-example", "scope-write");
    // These visible values come from the old compiled Validate/Apply hook after negotiation to 1.1.
    let plugin_runtime::plugin_protocol::ui::Kind::Text { text } =
        &manager.live["capability-example"].views["welcome"]
            .root
            .kind
    else {
        panic!("Expected the old SDK's native panel");
    };
    assert!(text.contains("enabled=true"));
    assert!(text.contains("Optional feature unavailable; native fallback active."));
    let control = plugin_runtime::InstallControl::default();
    for policy in ["fail", "activate-fail"] {
        // Reset the committed data so the later write proves which side of preparation supplied recovery.
        manager
            .invoke_command(
                "capability-example",
                "scope-write",
                json!({"text":"initial"}),
            )
            .unwrap();
        let owner = manager
            .instance_id("capability-example")
            .unwrap()
            .to_owned();
        let candidate = fixture(2, policy);
        if policy == "fail" {
            // The disposable migration preview rejects before cutover; its staged write cannot retire
            // the old owner or overwrite the latest data already committed through the old SDK guest.
            manager
                .invoke_command(
                    "capability-example",
                    "scope-write",
                    json!({"text":"latest"}),
                )
                .unwrap();
            let failure = manager
                .prepare_installation(&candidate, candidate.manifest.permissions.clone(), &control)
                .err()
                .expect("The actual guest must reject its isolated migration preview");
            assert!(
                format!("{failure:#}").contains("Fixture rejected migration after a staged write")
            );
            assert_eq!(
                manager.instance_id("capability-example"),
                Some(owner.as_str())
            );
        } else {
            let prepared = manager
                .prepare_installation(&candidate, candidate.manifest.permissions.clone(), &control)
                .unwrap();
            // This write follows the successful preview. Final-copy migration and fresh rollback must
            // preserve it instead of replaying the preview's older "initial" data or WASM handles.
            manager
                .invoke_command(
                    "capability-example",
                    "scope-write",
                    json!({"text":"latest"}),
                )
                .unwrap();
            let failure = manager
                .commit_installation(prepared, &control)
                .err()
                .expect("The actual guest must reject final business activation");
            assert!(format!("{failure:#}").contains("Fixture activation failed"));
            assert!(manager.instance_id("capability-example").is_some());
            assert_ne!(
                manager.instance_id("capability-example"),
                Some(owner.as_str())
            );
        }
        assert_eq!(
            manager.installed["capability-example"]
                .manifest
                .version
                .to_string(),
            "1.0.0"
        );
        assert_eq!(private_text(&mut manager), "workspace|latest");
    }
    let candidate = fixture(2, "ok");
    legacy::install(&mut manager, &candidate);
    assert_eq!(private_text(&mut manager), "workspace|v2:latest");
    assert_eq!(
        std::fs::read_to_string(workspace.path().join("source.txt")).unwrap(),
        "workspace"
    );
    manager.shutdown();
    assert_eq!(manager.resource_count(), 0);
}
