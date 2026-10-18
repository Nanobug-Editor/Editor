//! Explicit installation owns Git availability; retaining private data never reinstalls the package.
use super::*;

/// The same unmodified host starts without Git, loads the separately built ZIP on demand and
/// removes every contribution/resource on uninstall, including after reopening its private profile.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; public install/uninstall boundary"]
fn git_package_is_optional_and_uninstall_does_not_restore_it_on_reopen() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::package_lifecycle::git_package_is_optional_and_uninstall_does_not_restore_it_on_reopen",
    ) {
        return;
    }
    let repository = repository();
    let root = repository.path();
    git(root, &["commit", "--allow-empty", "-m", "initial"]);
    let profile = tempfile::tempdir().unwrap();
    let environment = Environment {
        workspace: root.display().to_string(),
        os: "windows".into(),
        locale: "en".into(),
        ..Default::default()
    };
    let store = profile.path().join("plugins");
    let mut manager = Manager::open(store.clone(), environment.clone()).unwrap();
    assert!(manager.installed.is_empty());
    assert!(manager.live.is_empty());
    assert!(manager.availability("git").is_err());

    let package = candidate();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    assert!(manager.installed.contains_key("git"));
    assert!(
        manager.live.is_empty(),
        "installation does not activate a guest"
    );
    manager = reopen(manager, profile.path(), root);
    assert!(manager.live["git"].views.contains_key("staging"));
    assert!(manager.live["git"].views.contains_key("history"));
    let retained_trees = manager.live["git"].tree_resources();
    assert!(!retained_trees.is_empty());

    // Uninstall is performed only against this disposable profile. Keeping data is deliberately
    // distinct from keeping executable availability or a mounted native tree authority.
    manager.uninstall("git", false).unwrap();
    assert!(!manager.installed.contains_key("git"));
    assert!(!manager.live.contains_key("git"));
    assert_eq!(manager.resource_count(), 0);
    assert!(retained_trees.iter().all(|tree| !tree.is_live()));
    manager.shutdown_checked().unwrap();
    drop(manager);

    let mut reopened = Manager::open(store, environment).unwrap();
    assert!(!reopened.installed.contains_key("git"));
    assert!(reopened.live.is_empty());
    assert!(reopened.availability("git").is_err());
    assert_eq!(reopened.resource_count(), 0);
}
