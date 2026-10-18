//! Native fixture preparation must not execute inherited hooks or copy external init templates.
use super::*;
use std::time::{Duration, Instant};

/// Drive the existing real child entry, stopping at a deliberately invalid Package before GPUI.
/// All injected configuration and executable sentinels belong to this test's temporary directory.
#[test]
fn native_fixture_git_ignores_inherited_hooks_and_templates() {
    super::super::super::super::git_fixture_environment::reject_repository_overrides().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("native-root");
    let hooks = directory.path().join("inherited-hooks");
    let template = directory.path().join("inherited-template");
    let marker = directory.path().join("hook-must-not-run.txt");
    fs::create_dir_all(&hooks).unwrap();
    fs::create_dir_all(template.join("hooks")).unwrap();
    let hook = format!(
        "#!/bin/sh\nprintf '%s\\n' 'owned hook executed' > '{}'\nexit 73\n",
        git_path(&marker).replace('\'', "'\\''")
    );
    fs::write(hooks.join("pre-commit"), &hook).unwrap();
    fs::write(hooks.join("fsmonitor"), &hook).unwrap();
    fs::write(template.join("hooks/pre-commit"), &hook).unwrap();
    let template_marker = b"inherited template must not enter the fixture\n";
    fs::write(template.join("template-marker.txt"), template_marker).unwrap();
    let config = directory.path().join("inherited.config");
    let config_text = format!(
        "[core]\n\thooksPath = \"{}\"\n\tfsmonitor = \"{}\"\n[init]\n\ttemplateDir = \"{}\"\n",
        git_path(&hooks).replace('"', "\\\""),
        // A monitor remains executable even after repository-local hooksPath is safely set.
        // Quote the owned command for Git's hook shell and then for its config parser.
        format!(
            "'{}'",
            git_path(&hooks.join("fsmonitor")).replace('\'', "'\\''")
        )
        .replace('\\', "\\\\")
        .replace('"', "\\\""),
        git_path(&template).replace('"', "\\\"")
    );
    fs::write(&config, &config_text).unwrap();
    let candidate = directory.path().join("invalid-candidate.zip");
    // A fixed invalid ZIP guarantees even a broken assertion can never start the real desktop.
    fs::write(&candidate, b"deliberately invalid Package before GUI").unwrap();
    let diagnostic = rejected_before_gui(directory.path(), &root, &config, &template, &candidate);
    assert!(
        diagnostic.contains("native fixture package validation after Git preparation"),
        "fixture did not complete Git preparation before Package rejection: {diagnostic}"
    );
    assert!(!marker.exists(), "fixture executed an inherited hook");
    let workspace = root.join("workspace");
    assert!(
        !workspace.join(".git/template-marker.txt").exists(),
        "init copied the inherited template"
    );
    assert_eq!(
        git_text(&workspace, &["log", "-1", "--format=%s"])
            .unwrap()
            .trim(),
        "Review history 35"
    );
    let empty_hooks = root.join("git-configuration").join("empty-hooks");
    assert_eq!(
        Path::new(
            git_text(
                &workspace,
                &["config", "--local", "--get", "core.hooksPath"]
            )
            .unwrap()
            .trim()
        ),
        empty_hooks
    );
    assert_eq!(fs::read_dir(&empty_hooks).unwrap().count(), 0);
    assert_eq!(
        git_text(
            &workspace,
            &["config", "--local", "--get", "commit.gpgsign"]
        )
        .unwrap()
        .trim(),
        "false"
    );
    // The other factory process entry must still establish genuine conflict stages, using the
    // same isolated defaults rather than masking a failed merge behind a synthetic index.
    Fixture::enter_conflict(&workspace).unwrap();
    assert!(!marker.exists(), "merge executed an inherited hook");
    assert_eq!(fs::read(&config).unwrap(), config_text.as_bytes());
    assert_eq!(fs::read(hooks.join("pre-commit")).unwrap(), hook.as_bytes());
    assert_eq!(fs::read(hooks.join("fsmonitor")).unwrap(), hook.as_bytes());
    assert_eq!(
        fs::read(template.join("hooks/pre-commit")).unwrap(),
        hook.as_bytes()
    );
    assert_eq!(
        fs::read(template.join("template-marker.txt")).unwrap(),
        template_marker
    );
}

/// Own the ordinary child handle and its bounded retirement. Redirect to a file so diagnostic
/// pipes cannot block the child; an invalid Package stops this route before WindowsPlatform starts.
fn rejected_before_gui(
    directory: &Path,
    root: &Path,
    config: &Path,
    template: &Path,
    candidate: &Path,
) -> String {
    let log_path = directory.join("native-child.log");
    let log = fs::File::create(&log_path).unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "extensions::worker::native_acceptance::git_native_matrix_child",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .current_dir(directory)
        // Inject only this child's owned settings; never mutate the test process environment.
        .env("GIT_CONFIG_GLOBAL", config)
        .env("GIT_CONFIG_SYSTEM", config)
        .env_remove("GIT_CONFIG_NOSYSTEM")
        .env("GIT_TEMPLATE_DIR", template)
        .env("NANOBUG_GIT_NATIVE_CASE", "dark-zh-CN-100")
        .env("NANOBUG_GIT_NATIVE_ROOT", root)
        .env("NANOBUG_GIT_NATIVE_OUTPUT", directory.join("output"))
        .env("NANOBUG_GIT_CANDIDATE", candidate)
        .env("ME_EDITOR_PROFILE_HOME", root.join("profile"))
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            // Terminate only the Child this test started, then release its temporary storage.
            let _ = child.kill();
            let _ = child.wait();
            panic!("native fixture configuration regression exceeded its 30-second bound");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(!status.success(), "invalid Package unexpectedly passed");
    fs::read_to_string(log_path).unwrap()
}

/// Git's config and its POSIX hook interpreter accept slash-separated owned Windows paths.
fn git_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
