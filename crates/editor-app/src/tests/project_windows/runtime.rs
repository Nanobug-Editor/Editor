//! A shared plugin store serves both real project actors, while their native publications remain independent.

use super::*;

/// Both projects use one installed WASM package; releasing A leaves B's ordinary command path available.
#[test]
#[ignore = "opt-in frozen capability-example Package/WASM; drives real background project actors"]
fn concurrent_project_actors_share_store_and_keep_native_publications_independent() {
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
    let mut installed = Manager::open(store.path().into(), environment(a.path())).unwrap();
    installed
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    drop(installed);
    let pool = Arc::new(crate::extensions::runtime_owners::RuntimePool::default());
    let first = Worker::start_background_in_pool(
        pool.clone(),
        store.path().into(),
        environment(a.path()),
        true,
    );
    wait_for(&first, |state| state.ready);
    let second = Worker::start_background_in_pool(
        pool.clone(),
        store.path().into(),
        environment(b.path()),
        true,
    );
    wait_for(&second, |state| state.ready || state.status.is_some());
    assert!(
        second.state.lock().unwrap().ready,
        "second project lost its runtime to the shared data lock"
    );
    let visible = |state: &Published, expected: &str| {
        state
            .views
            .values()
            .any(|scene| matches!(&scene.root.kind, ui::Kind::Text { text } if text == expected))
    };
    first
        .tx
        .send(command_work(
            &first,
            "scope-write",
            json!({"text":"private A"}),
        ))
        .unwrap();
    second
        .tx
        .send(command_work(
            &second,
            "scope-write",
            json!({"text":"private B"}),
        ))
        .unwrap();
    wait_for(&first, |state| visible(state, "workspace A|private A"));
    wait_for(&second, |state| visible(state, "workspace B|private B"));
    futures::executor::block_on(first.request_shutdown().unwrap())
        .unwrap()
        .unwrap();
    // A new B command after A's actual retirement proves the shared store and B's original authority survived.
    second
        .tx
        .send(command_work(
            &second,
            "scope-write",
            json!({"text":"still B"}),
        ))
        .unwrap();
    wait_for(&second, |state| visible(state, "workspace B|still B"));
    // Reopening the exact path must create a new live scope while preserving its committed private state.
    let reopened = Worker::start_background_in_pool(
        pool.clone(),
        store.path().into(),
        environment(a.path()),
        true,
    );
    wait_for(&reopened, |state| state.ready);
    reopened
        .tx
        .send(command_work(&reopened, "scope-read", json!({})))
        .unwrap();
    wait_for(&reopened, |state| visible(state, "workspace A|private A"));
    futures::executor::block_on(second.request_shutdown().unwrap())
        .unwrap()
        .unwrap();
    reopened
        .tx
        .send(command_work(
            &reopened,
            "scope-write",
            json!({"text":"reopened A"}),
        ))
        .unwrap();
    wait_for(&reopened, |state| visible(state, "workspace A|reopened A"));
    futures::executor::block_on(reopened.request_shutdown().unwrap())
        .unwrap()
        .unwrap();
    pool.shutdown().unwrap();
}
