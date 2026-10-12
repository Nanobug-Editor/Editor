//! Private files and credentials remain isolated across plugins and outside project data.
use super::*;

/// Secrets and private roots are isolated even when both independent consumers have their own grants.
#[test]
#[ignore = "opt-in fixed SDK/WASM packages; rebuild sources: crates/plugin-runtime/tests/fixtures/community; see fixtures/packages/README.md"]
fn t27_t29_private_directories_credentials_isolation_and_temporary_retirement() {
    let credentials = SecretCleanup::new("cloud-translation", "api-token");
    let root = credentials.root.path();
    let mut manager = Manager::open(root.into(), environment(root)).unwrap();
    for id in ["cloud-translation", "code-host-query"] {
        install(
            &mut manager,
            &package(id, |manifest| manifest["storage_limit"] = json!(1024)),
        );
    }
    probe(
        &mut manager,
        "cloud-translation",
        secret_op(secrets::Operation::Write {
            key: "api-token".into(),
            bytes: b"fake-isolated-key".to_vec(),
        }),
    )
    .unwrap();
    for (id, expected) in [
        ("cloud-translation", "secret-present"),
        ("code-host-query", "secret-missing"),
    ] {
        // The second namespace has not yet consumed a command, unlike the first writer above.
        demand(&mut manager, id, DemandReason::Command("probe".into()));
        manager
            .invoke_command(
                id,
                "probe",
                serde_json::to_value(secret_op(secrets::Operation::Read {
                    key: "api-token".into(),
                }))
                .unwrap(),
            )
            .unwrap();
        assert_eq!(text(&manager, id), expected);
    }
    let data = resource(probe(&mut manager, "cloud-translation", Op::OpenData).unwrap());
    probe(
        &mut manager,
        "cloud-translation",
        private_op(storage::Operation::CreateDirectory {
            handle: data.clone(),
            path: "queries".into(),
        }),
    )
    .unwrap();
    probe(
        &mut manager,
        "cloud-translation",
        Op::WriteFile {
            handle: data.clone(),
            path: "queries/result.txt".into(),
            bytes: b"query-result".to_vec(),
        },
    )
    .unwrap();
    let Value::PrivateListing(list) = probe(
        &mut manager,
        "cloud-translation",
        private_op(storage::Operation::List {
            handle: data.clone(),
            path: "queries".into(),
            after: None,
            limit: 1,
        }),
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(list.entries[0].name, "result.txt");
    assert_eq!(list.entries[0].bytes, 12);
    assert!(!list.has_more);
    probe(
        &mut manager,
        "cloud-translation",
        private_op(storage::Operation::Rename {
            handle: data.clone(),
            path: "queries/result.txt".into(),
            destination: "queries/renamed.txt".into(),
        }),
    )
    .unwrap();
    let Value::Bytes(bytes) = probe(
        &mut manager,
        "cloud-translation",
        Op::ReadFile {
            handle: data.clone(),
            path: "queries/renamed.txt".into(),
        },
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(bytes, b"query-result");
    assert_eq!(
        probe(
            &mut manager,
            "code-host-query",
            Op::ReadFile {
                handle: data.clone(),
                path: "queries/renamed.txt".into()
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidHandle
    );
    for path in ["../other", "C:/outside", "CON", "a:stream", "a\\b"] {
        assert_eq!(
            probe(
                &mut manager,
                "cloud-translation",
                Op::WriteFile {
                    handle: data.clone(),
                    path: path.into(),
                    bytes: vec![1]
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidPath
        );
    }
    assert_eq!(
        probe(
            &mut manager,
            "cloud-translation",
            private_op(storage::Operation::Delete {
                handle: data.clone(),
                path: "".into(),
                recursive: true
            })
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidPath
    );
    for area in [storage::Area::Cache, storage::Area::Temporary] {
        let handle = resource(
            probe(
                &mut manager,
                "cloud-translation",
                private_op(storage::Operation::Open { area }),
            )
            .unwrap(),
        );
        probe(
            &mut manager,
            "cloud-translation",
            Op::WriteFile {
                handle,
                path: "owned.txt".into(),
                bytes: b"owned private bytes".to_vec(),
            },
        )
        .unwrap();
    }
    assert_eq!(
        probe(
            &mut manager,
            "cloud-translation",
            Op::WriteFile {
                handle: data.clone(),
                path: "queries/too-large.txt".into(),
                bytes: vec![b'x'; 1024]
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::LimitExceeded
    );
    manager.disable("cloud-translation").unwrap();
    manager.enable("cloud-translation").unwrap();
    assert_eq!(
        probe(
            &mut manager,
            "cloud-translation",
            Op::ReadFile {
                handle: data,
                path: "queries/renamed.txt".into()
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidHandle
    );
    let cache = resource(
        probe(
            &mut manager,
            "cloud-translation",
            private_op(storage::Operation::Open {
                area: storage::Area::Cache,
            }),
        )
        .unwrap(),
    );
    assert!(matches!(
        probe(
            &mut manager,
            "cloud-translation",
            Op::ReadFile {
                handle: cache,
                path: "owned.txt".into()
            }
        )
        .unwrap(),
        Value::Bytes(_)
    ));
    let temporary = resource(
        probe(
            &mut manager,
            "cloud-translation",
            private_op(storage::Operation::Open {
                area: storage::Area::Temporary,
            }),
        )
        .unwrap(),
    );
    assert_eq!(
        probe(
            &mut manager,
            "cloud-translation",
            Op::ReadFile {
                handle: temporary,
                path: "owned.txt".into()
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::NotFound
    );
    manager
        .invoke_command(
            "cloud-translation",
            "probe",
            serde_json::to_value(secret_op(secrets::Operation::Read {
                key: "api-token".into(),
            }))
            .unwrap(),
        )
        .unwrap();
    assert_eq!(text(&manager, "cloud-translation"), "secret-present");
    probe(
        &mut manager,
        "cloud-translation",
        secret_op(secrets::Operation::Delete {
            key: "api-token".into(),
        }),
    )
    .unwrap();
    let data = resource(probe(&mut manager, "cloud-translation", Op::OpenData).unwrap());
    probe(
        &mut manager,
        "cloud-translation",
        private_op(storage::Operation::Delete {
            handle: data,
            path: "queries".into(),
            recursive: true,
        }),
    )
    .unwrap();
    #[cfg(windows)]
    reparse_boundary(&mut manager, root);
    entry_quotas(&mut manager, root);
    manager.disable("cloud-translation").unwrap();
    manager.disable("code-host-query").unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// The fixture locates only this test's owned data root; SDK calls never receive its absolute path.
fn private_owner(root: &std::path::Path) -> std::path::PathBuf {
    use sha2::{Digest, Sha256};
    root.join("data/cloud-translation/workspaces")
        .join(format!(
            "{:x}",
            Sha256::digest(root.canonicalize().unwrap().to_string_lossy().as_bytes())
        ))
        .join("files")
}

/// A real Windows junction cannot redirect private reads, writes, enumeration or deletion to outside data.
#[cfg(windows)]
fn reparse_boundary(manager: &mut Manager, root: &std::path::Path) {
    let outside = tempfile::tempdir().unwrap();
    let sentinel = outside.path().join("sentinel.txt");
    std::fs::write(&sentinel, b"outside sentinel").unwrap();
    struct Junction(std::path::PathBuf);
    impl Drop for Junction {
        fn drop(&mut self) {
            // Remove only the junction entry, never recursively traverse its external target.
            let _ = std::fs::remove_dir(&self.0);
        }
    }
    let junction = Junction(private_owner(root).join("junction"));
    let quote =
        |path: &std::path::Path| format!("'{}'", path.to_string_lossy().replace('\'', "''"));
    let status = std::process::Command::new("pwsh")
        .args(["-NoProfile", "-NonInteractive", "-Command"])
        .arg(format!(
            "New-Item -ItemType Junction -Path {} -Target {} -ErrorAction Stop | Out-Null",
            quote(&junction.0),
            quote(outside.path())
        ))
        .status()
        .unwrap();
    assert!(status.success());
    let data = resource(probe(manager, "cloud-translation", Op::OpenData).unwrap());
    for operation in [
        Op::ReadFile {
            handle: data.clone(),
            path: "junction/sentinel.txt".into(),
        },
        Op::WriteFile {
            handle: data.clone(),
            path: "junction/extra.txt".into(),
            bytes: vec![1],
        },
        private_op(storage::Operation::Stat {
            handle: data.clone(),
            path: "junction".into(),
        }),
        private_op(storage::Operation::List {
            handle: data.clone(),
            path: "junction".into(),
            after: None,
            limit: 128,
        }),
        private_op(storage::Operation::Rename {
            handle: data.clone(),
            path: "junction".into(),
            destination: "moved".into(),
        }),
        private_op(storage::Operation::Delete {
            handle: data.clone(),
            path: "junction".into(),
            recursive: true,
        }),
    ] {
        assert_eq!(
            probe(manager, "cloud-translation", operation)
                .unwrap_err()
                .code,
            ErrorCode::InvalidPath
        );
    }
    assert_eq!(std::fs::read(sentinel).unwrap(), b"outside sentinel");
    assert!(!outside.path().join("extra.txt").exists());
    probe(
        manager,
        "cloud-translation",
        Op::CloseResource { handle: data },
    )
    .unwrap();
}

/// Native fixture preparation reaches metadata limits efficiently; mutations still enter through the real SDK.
fn entry_quotas(manager: &mut Manager, root: &std::path::Path) {
    let owner = private_owner(root);
    fn usage(path: &std::path::Path) -> (usize, usize) {
        let mut entries = 0;
        let mut names = 0;
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            entries += 1;
            names += entry.file_name().to_string_lossy().len();
            if entry.path().is_dir() {
                let child = usage(&entry.path());
                entries += child.0;
                names += child.1;
            }
        }
        (entries, names)
    }
    let data = resource(probe(manager, "cloud-translation", Op::OpenData).unwrap());
    let count = usage(&owner).0;
    for index in count..1024 {
        std::fs::create_dir(owner.join(format!("entry{index:04}"))).unwrap();
    }
    for operation in [
        private_op(storage::Operation::CreateDirectory {
            handle: data.clone(),
            path: "one-more".into(),
        }),
        Op::WriteFile {
            handle: data.clone(),
            path: "one-more.txt".into(),
            bytes: vec![1],
        },
    ] {
        assert_eq!(
            probe(manager, "cloud-translation", operation)
                .unwrap_err()
                .code,
            ErrorCode::LimitExceeded
        );
    }
    assert!(!owner.join("one-more").exists());
    assert!(!owner.join("one-more.txt").exists());
    let Value::PrivateListing(page) = probe(
        manager,
        "cloud-translation",
        private_op(storage::Operation::List {
            handle: data.clone(),
            path: "".into(),
            after: None,
            limit: 128,
        }),
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(page.entries.len(), 128);
    assert!(page.has_more);
    for entry in std::fs::read_dir(&owner).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name().to_string_lossy().starts_with("entry") {
            std::fs::remove_dir(entry.path()).unwrap();
        }
    }
    let names = usage(&owner).1;
    let count = (65536 - names) / 255;
    for index in 0..count {
        std::fs::create_dir(owner.join(format!("{}{:05}", "n".repeat(250), index))).unwrap();
    }
    assert_eq!(
        probe(
            manager,
            "cloud-translation",
            private_op(storage::Operation::CreateDirectory {
                handle: data.clone(),
                path: "z".repeat(255)
            })
        )
        .unwrap_err()
        .code,
        ErrorCode::LimitExceeded
    );
    assert!(!owner.join("z".repeat(255)).exists());
    probe(
        manager,
        "cloud-translation",
        Op::CloseResource { handle: data },
    )
    .unwrap();
}
