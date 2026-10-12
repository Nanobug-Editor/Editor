//! Shared private depth includes the reserved cache/temporary directory before any mutation.
use super::*;

/// A refused depth overflow leaves every private area usable for ordinary list/write/delete operations.
#[test]
#[ignore = "opt-in fixed SDK/WASM packages; rebuild sources: crates/plugin-runtime/tests/fixtures/community; see fixtures/packages/README.md"]
fn t29_private_area_depth_overflow_is_rejected_before_poisoning_shared_storage() {
    let root = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(root.path().into(), environment(root.path())).unwrap();
    install(&mut manager, &package("cloud-translation", |_| {}));
    for area in [
        storage::Area::Data,
        storage::Area::Cache,
        storage::Area::Temporary,
    ] {
        let handle = resource(
            probe(
                &mut manager,
                "cloud-translation",
                private_op(storage::Operation::Open { area }),
            )
            .unwrap(),
        );
        let levels = if matches!(area, storage::Area::Data) {
            16
        } else {
            15
        };
        let mut path = String::new();
        for _ in 0..levels {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str("level");
            probe(
                &mut manager,
                "cloud-translation",
                private_op(storage::Operation::CreateDirectory {
                    handle: handle.clone(),
                    path: path.clone(),
                }),
            )
            .unwrap();
        }
        let rejected = probe(
            &mut manager,
            "cloud-translation",
            private_op(storage::Operation::CreateDirectory {
                handle: handle.clone(),
                path: format!("{path}/overflow"),
            }),
        )
        .unwrap_err();
        assert_eq!(
            rejected.code,
            if matches!(area, storage::Area::Data) {
                ErrorCode::InvalidPath
            } else {
                ErrorCode::LimitExceeded
            }
        );
        // Ordinary WriteFile already counts reserved prefixes; a rejected child cannot alter the tree.
        assert_eq!(
            probe(
                &mut manager,
                "cloud-translation",
                Op::WriteFile {
                    handle: handle.clone(),
                    path: format!("{path}/overflow.txt"),
                    bytes: b"must not exist".to_vec(),
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidPath
        );
        let Value::PrivateListing(list) = probe(
            &mut manager,
            "cloud-translation",
            private_op(storage::Operation::List {
                handle: handle.clone(),
                path: path.clone(),
                after: None,
                limit: 128,
            }),
        )
        .unwrap() else {
            panic!("Expected a listing of the deepest valid directory")
        };
        assert!(list.entries.is_empty());
        probe(
            &mut manager,
            "cloud-translation",
            Op::WriteFile {
                handle: handle.clone(),
                path: "still-usable.txt".into(),
                bytes: b"owned bytes".to_vec(),
            },
        )
        .unwrap();
        probe(
            &mut manager,
            "cloud-translation",
            private_op(storage::Operation::Delete {
                handle: handle.clone(),
                path: "level".into(),
                recursive: true,
            }),
        )
        .unwrap();
        probe(
            &mut manager,
            "cloud-translation",
            private_op(storage::Operation::Delete {
                handle: handle.clone(),
                path: "still-usable.txt".into(),
                recursive: false,
            }),
        )
        .unwrap();
        probe(
            &mut manager,
            "cloud-translation",
            Op::CloseResource { handle },
        )
        .unwrap();
    }
    manager.disable("cloud-translation").unwrap();
    assert_eq!(manager.resource_count(), 0);
}
