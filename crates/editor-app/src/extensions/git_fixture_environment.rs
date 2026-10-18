//! Shared test-only Git isolation protects fixture factories and their actual Manager/Worker children.
// The common source is a private test module, not an editor/runtime dependency or public host API.
#[path = "../../../plugin-runtime/tests/git_plugin/isolation.rs"]
mod isolation;
use isolation::REPOSITORY_OVERRIDES;
pub(super) use isolation::{
    check_external_configuration, reject_repository_overrides, require_private_configuration,
    run_test as run_isolated_test,
};

/// Exercise the existing child entry in a separate environment. Even a missing guard reaches an
/// unreadable dummy package before GPUI, so this regression never opens a desktop or user workspace.
#[cfg(windows)]
#[test]
fn inherited_common_dir_is_rejected_before_native_fixture_setup() {
    use std::{fs, process::Command};

    let directory = tempfile::tempdir().unwrap();
    let sentinel = directory.path().join("external-common-directory");
    fs::create_dir(&sentinel).unwrap();
    let marker = sentinel.join("sentinel.txt");
    let original = b"parent-owned sentinel must remain unchanged\n";
    fs::write(&marker, original).unwrap();
    let root = directory.path().join("native-root");
    let output = directory.path().join("native-output");
    let candidate = directory.path().join("not-a-readable-candidate.zip");
    let mut child = Command::new(std::env::current_exe().unwrap());
    child.args([
        "--exact",
        "extensions::worker::native_acceptance::git_native_matrix_child",
        "--ignored",
        "--nocapture",
        "--test-threads=1",
    ]);
    // Only this child receives the injected override. Existing caller values cannot change the
    // expected failure or reach its temporary repository; the process-wide environment is intact.
    for name in REPOSITORY_OVERRIDES {
        child.env_remove(name);
    }
    let result = child
        .env("GIT_COMMON_DIR", &sentinel)
        .env("NANOBUG_GIT_NATIVE_CASE", "dark-zh-CN-100")
        .env("NANOBUG_GIT_NATIVE_ROOT", &root)
        .env("NANOBUG_GIT_NATIVE_OUTPUT", &output)
        .env("NANOBUG_GIT_CANDIDATE", &candidate)
        .output()
        .unwrap();
    assert!(
        !result.status.success(),
        "unsafe fixture child unexpectedly succeeded"
    );
    let diagnostic = format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        diagnostic.contains(
            "Git test fixture refuses repository-local environment overrides: GIT_COMMON_DIR"
        ),
        "native child failed for a reason other than the inherited repository override"
    );
    assert!(
        !diagnostic.contains(sentinel.to_string_lossy().as_ref()),
        "fixture rejection disclosed the environment value"
    );
    assert!(
        !root.join("workspace").exists(),
        "workspace was created before rejection"
    );
    assert!(
        !root.join("profile").exists(),
        "profile was created before rejection"
    );
    assert!(!output.exists(), "output was created before rejection");
    assert!(
        !candidate.exists(),
        "dummy candidate must never be prepared"
    );
    assert_eq!(fs::read(&marker).unwrap(), original);
    let mut remaining = fs::read_dir(&sentinel)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    remaining.sort();
    assert_eq!(remaining, [std::ffi::OsString::from("sentinel.txt")]);
}
