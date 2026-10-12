//! Failed candidate checkpoints and durable publication barriers cannot mutate a previously stored system key.
use super::*;
use plugin_runtime::{InstallControl, InstallStage};

/// Real system credentials survive write/delete attempts from an unpublished SDK candidate.
#[test]
#[ignore = "opt-in fixed secret-candidate package; source: crates/plugin-runtime/tests/fixtures/secret-candidate"]
fn t27_t30_unpublished_snapshot_and_publish_failure_preserve_system_token() {
    let credentials = SecretCleanup::new("secret-candidate", "api-token");
    let root = credentials.root.path();
    let mut manager = Manager::open(root.into(), environment(root)).unwrap();
    let old = package("secret-candidate", |_| {});
    install(&mut manager, &old);
    let store = platform_windows::credentials::CredentialStore::new(&credentials.target).unwrap();
    let write_committed = |manager: &mut Manager| {
        probe(
            manager,
            "secret-candidate",
            secret_op(secrets::Operation::Write {
                key: "api-token".into(),
                bytes: b"fake-committed-token".to_vec(),
            }),
        )
        .unwrap();
        assert_eq!(store.read().unwrap().unwrap(), b"fake-committed-token");
    };
    write_committed(&mut manager);
    for fail_snapshot in [true, false] {
        for mutation in ["write", "delete"] {
            let mut files = package("secret-candidate", |manifest| {
                manifest["version"] = json!("0.1.1")
            })
            .files;
            files.insert(
                "snapshot-policy.json".into(),
                serde_json::to_vec(&json!({"mutation":mutation,"fail":fail_snapshot})).unwrap(),
            );
            let candidate = archive_files(files);
            // Cancellation at the ordinary public barrier forces rollback after Snapshot and the data swap.
            let control_slot = std::sync::Arc::new(std::sync::Mutex::new(None::<InstallControl>));
            let slot = control_slot.clone();
            let control = InstallControl::new(move |stage| {
                if !fail_snapshot && stage == InstallStage::Committing {
                    slot.lock().unwrap().as_ref().unwrap().cancel();
                }
            });
            *control_slot.lock().unwrap() = Some(control.clone());
            let error = manager
                .install_with_control(&candidate, candidate.manifest.permissions.clone(), &control)
                .unwrap_err();
            control_slot.lock().unwrap().take();
            assert_eq!(
                store.read().unwrap(),
                Some(b"fake-committed-token".to_vec()),
                "unpublished {mutation}; snapshot failure={fail_snapshot}"
            );
            if fail_snapshot {
                assert!(format!("{error:#}").contains("mutation=InvalidState"));
            } else {
                assert!(format!("{error:#}").contains("cancelled"));
            }
            assert_eq!(
                manager.installed["secret-candidate"].manifest.version,
                old.manifest.version
            );
            // Recovery uses a fresh installed owner and must reopen mutation only after its own activation succeeds.
            write_committed(&mut manager);
        }
    }
    manager.disable("secret-candidate").unwrap();
    assert_eq!(store.read().unwrap().unwrap(), b"fake-committed-token");
    manager.enable("secret-candidate").unwrap();
    write_committed(&mut manager);
    manager
        .update_setting(
            "secret-candidate",
            plugin_runtime::plugin_protocol::settings::Scope::User,
            "flag",
            Some(json!(true)),
        )
        .unwrap();
    write_committed(&mut manager);
    drop(manager);
    let mut reopened = Manager::open(root.into(), environment(root)).unwrap();
    write_committed(&mut reopened);
    probe(
        &mut reopened,
        "secret-candidate",
        secret_op(secrets::Operation::Delete {
            key: "api-token".into(),
        }),
    )
    .unwrap();
    assert_eq!(store.read().unwrap(), None);
}
