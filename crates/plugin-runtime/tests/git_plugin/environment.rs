//! Git fixture setup rejects caller-selected repository authority before any private Git writes.
use super::*;
#[path = "isolation.rs"]
mod isolation;
use isolation::REPOSITORY_OVERRIDES;
pub(super) use isolation::run_test as run_isolated_test;

/// Refuse even empty/non-Unicode overrides, preserving the caller's process environment intact.
pub(super) fn require_isolated_git_environment() {
    for name in REPOSITORY_OVERRIDES {
        assert!(
            std::env::var_os(name).is_none(),
            "isolated Git fixture requires inherited {name} to be unset"
        );
    }
    isolation::require_private_configuration()
        .expect("Git factory requires its isolated test child");
}

/// Real init/add/commit/status must ignore inherited hooks, monitors and templates without changing
/// their source files or the parent environment; the ordinary Git factory is the regression boundary.
#[test]
fn git_fixture_ignores_external_hooks_monitors_and_templates() {
    isolation::check_external_configuration(
        "environment::git_fixture_ignores_external_hooks_monitors_and_templates",
        |_| {
            let repository = tempfile::tempdir().unwrap();
            let root = repository.path();
            git(root, &["init", "-b", "main"]);
            git(root, &["config", "user.name", "Configuration fixture"]);
            git(
                root,
                &["config", "user.email", "configuration@example.invalid"],
            );
            git(root, &["config", "commit.gpgsign", "false"]);
            std::fs::write(root.join("tracked.txt"), b"owned Git fixture\n").unwrap();
            git(root, &["add", "--", "tracked.txt"]);
            git(root, &["commit", "-m", "configuration isolation"]);
            assert_eq!(git(root, &["status", "--porcelain"]), "");
            assert_eq!(
                git(root, &["show", "HEAD:tracked.txt"]),
                "owned Git fixture\n"
            );
            assert!(!root.join(".git/template-marker.txt").exists());
        },
    );
}

/// A child-only override must fail at the actual Git helper before init can write any index/refs.
/// The parent retains both private paths and checks its sentinel; no process-global env is changed.
#[test]
fn git_fixture_rejects_repository_redirection_before_git_init() {
    const PROBE: &str = "NANOBUG_GIT_FIXTURE_ENV_PROBE";
    if let Some(root) = std::env::var_os(PROBE) {
        assert!(std::env::var_os("GIT_COMMON_DIR").is_some());
        git(Path::new(&root), &["init", "-b", "main"]);
        panic!("fixture Git accepted a repository override");
    }
    let directory = tempfile::tempdir().unwrap();
    let private = directory.path().join("private");
    let redirect = directory.path().join("redirect");
    std::fs::create_dir(&private).unwrap();
    std::fs::create_dir(&redirect).unwrap();
    let sentinel = redirect.join("sentinel");
    std::fs::write(&sentinel, b"retained caller data").unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap());
    child.args([
        "--exact",
        "environment::git_fixture_rejects_repository_redirection_before_git_init",
        "--nocapture",
        "--test-threads=1",
    ]);
    for name in REPOSITORY_OVERRIDES {
        child.env_remove(name);
    }
    let output = child
        .env(PROBE, &private)
        .env("GIT_COMMON_DIR", &redirect)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("requires inherited GIT_COMMON_DIR to be unset")
    );
    assert!(!private.join(".git").exists());
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"retained caller data");
    assert_eq!(std::fs::read_dir(&redirect).unwrap().count(), 1);
}
