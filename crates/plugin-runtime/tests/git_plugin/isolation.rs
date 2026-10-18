//! Private Git candidate acceptance children isolate every Git invocation, including Manager's native children.
//! Included only by the runtime integration target and editor-app's cfg(test) fixture module.
use anyhow::{Context as _, ensure};
use std::{
    fs,
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::OnceLock,
    time::{Duration, Instant},
};

/// Git's repository-local routing names, plus namespace and interactive credential entry points.
pub(crate) const REPOSITORY_OVERRIDES: &[&str] = &[
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_OBJECT_DIRECTORY",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_GRAFT_FILE",
    "GIT_INDEX_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
    "GIT_PREFIX",
    "GIT_SHALLOW_FILE",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
    "GIT_ASKPASS",
    "SSH_ASKPASS",
];

/// The exact test and owned configuration identify a child; neither variable authorizes production Git.
const CHILD_TEST: &str = "NANOBUG_GIT_ISOLATED_TEST";
const CONFIGURATION: &str = "NANOBUG_GIT_TEST_CONFIGURATION";
/// A regression-only root carries observations across its hostile outer child and private inner child.
const PROBE_ROOT: &str = "NANOBUG_GIT_CONFIGURATION_PROBE_ROOT";
/// Only a child that consumed its parent's pipe handoff can authorize fixture Git in this process.
static ATTESTED_CONFIGURATION: OnceLock<PathBuf> = OnceLock::new();

/// Reject present routing overrides, including empty/non-Unicode values, without printing values.
pub(crate) fn reject_repository_overrides() -> anyhow::Result<()> {
    let inherited = REPOSITORY_OVERRIDES
        .iter()
        .copied()
        .filter(|name| std::env::var_os(name).is_some())
        .collect::<Vec<_>>();
    ensure!(
        inherited.is_empty(),
        "Git test fixture refuses repository-local environment overrides: {}",
        inherited.join(", ")
    );
    Ok(())
}

/// Run `test` once in a same-name libtest child and return false in its successful parent.
/// The child inherits private defaults before its first init and before any Manager/Worker starts.
/// Routing overrides still fail before temporary setup; failures and a 600-second timeout panic.
pub(crate) fn run_test(test: &str) -> bool {
    reject_repository_overrides().expect("fixture environment cannot redirect repository state");
    if let Some(child) = std::env::var_os(CHILD_TEST) {
        assert_eq!(child, test, "private Git child must run its exact owner");
        let root = PathBuf::from(std::env::var_os(CONFIGURATION).expect("private Git child root"));
        let mut handoff = Vec::new();
        std::io::stdin()
            .take(8193)
            .read_to_end(&mut handoff)
            .unwrap();
        assert!(
            handoff.len() <= 8192
                && !handoff.is_empty()
                && fs::read(root.join("child-handoff")).is_ok_and(|expected| expected == handoff),
            "inherited environment cannot impersonate a private Git parent handoff"
        );
        ATTESTED_CONFIGURATION
            .set(root)
            .expect("one exact Git child per process");
        require_private_configuration().expect("private Git child configuration changed");
        return true;
    }
    let directory = tempfile::tempdir().expect("private Git child directory");
    let configuration = directory.path().join("git-configuration");
    prepare_configuration(&configuration).expect("private Git defaults");
    let handoff = format!(
        "Git candidate acceptance\n{test}\n{}\n{}\n",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    fs::write(configuration.join("child-handoff"), &handoff).expect("private Git pipe handoff");
    let mut child = exact_test(test).expect("current test executable");
    child
        .env(CHILD_TEST, test)
        .env(CONFIGURATION, &configuration)
        .env("GIT_CONFIG_GLOBAL", configuration.join("global.config"))
        .env("GIT_CONFIG_SYSTEM", configuration.join("system.config"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_TEMPLATE_DIR", configuration.join("empty-template"));
    let diagnostic = run_exact(
        child,
        directory.path(),
        Duration::from_secs(600),
        Some(handoff.as_bytes()),
    )
    .expect("isolated Git test child failed");
    eprintln!("isolated Git test completed: {test}; {diagnostic}");
    false
}

/// Reject a factory called outside its configured child, so a future missing entry wrapper fails closed.
/// Check environment bindings and owned files rather than assuming a test marker implies isolation.
pub(crate) fn require_private_configuration() -> anyhow::Result<()> {
    let root =
        PathBuf::from(std::env::var_os(CONFIGURATION).context("private Git child required")?);
    ensure!(
        ATTESTED_CONFIGURATION.get() == Some(&root),
        "Git fixture needs its parent's private pipe handoff"
    );
    ensure!(
        root.is_absolute(),
        "private Git configuration must be absolute"
    );
    for (name, path) in [
        ("GIT_CONFIG_GLOBAL", root.join("global.config")),
        ("GIT_CONFIG_SYSTEM", root.join("system.config")),
        ("GIT_TEMPLATE_DIR", root.join("empty-template")),
    ] {
        ensure!(
            std::env::var_os(name).is_some_and(|value| Path::new(&value) == path),
            "private Git child lost {name}"
        );
    }
    ensure!(
        std::env::var_os("GIT_CONFIG_NOSYSTEM").is_some_and(|value| value == "1"),
        "private Git child must suppress system defaults"
    );
    ensure!(
        fs::read(root.join("global.config"))? == configuration_text(&root).as_bytes()
            && fs::read(root.join("system.config"))?.is_empty(),
        "private Git defaults changed"
    );
    for directory in ["empty-template", "empty-hooks"] {
        ensure!(
            fs::read_dir(root.join(directory))?.next().is_none(),
            "private Git {directory} must remain empty"
        );
    }
    Ok(())
}

/// Create only `root`'s private config, empty template and hooks; callers own its temporary lifetime.
fn prepare_configuration(root: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(root.join("empty-template"))?;
    fs::create_dir_all(root.join("empty-hooks"))?;
    fs::write(root.join("global.config"), configuration_text(root))?;
    fs::write(root.join("system.config"), b"")?;
    Ok(())
}

/// Empty hooks apply even to bare/linked repositories; no user global/system filter or monitor is read.
fn configuration_text(root: &Path) -> String {
    format!(
        "[core]\n\thooksPath = \"{}\"\n\tfsmonitor = false\n[init]\n\ttemplateDir = \"{}\"\n",
        config_path(&root.join("empty-hooks")),
        config_path(&root.join("empty-template"))
    )
}

/// Quote an owned path for Git's double-quoted config syntax, independent from shell parsing.
fn config_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace('"', "\\\"")
}

/// Preserve libtest's exact identity and include ignored package cases without changing their body.
fn exact_test(test: &str) -> anyhow::Result<Command> {
    let mut child = Command::new(std::env::current_exe()?);
    child.args([
        "--exact",
        test,
        "--include-ignored",
        "--nocapture",
        "--test-threads=1",
        "--color=never",
    ]);
    Ok(child)
}

/// Capture to an owned file so pipes cannot block; kill/wait only this Child at `timeout`.
/// A successful process must also report exactly one passed libtest case, preventing empty filters.
fn run_exact(
    mut command: Command,
    directory: &Path,
    timeout: Duration,
    handoff: Option<&[u8]>,
) -> anyhow::Result<String> {
    let path = directory.join("child.log");
    let log = fs::File::create(&path)?;
    let mut child = command
        .stdin(if handoff.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?;
    if let Some(handoff) = handoff {
        let sent = child
            .stdin
            .take()
            .context("private Git pipe unavailable")?
            .write_all(handoff);
        if let Err(error) = sent {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error.into());
        }
    }
    let deadline = Instant::now() + timeout;
    let status: ExitStatus = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!(
                "isolated Git child exceeded {timeout:?}:\n{}",
                fs::read_to_string(path)?
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let diagnostic = fs::read_to_string(path)?;
    ensure!(
        status.success(),
        "isolated Git child exited {status}:\n{diagnostic}"
    );
    ensure!(
        diagnostic.contains("running 1 test")
            && diagnostic.contains("test result: ok. 1 passed; 0 failed;"),
        "isolated Git child did not execute exactly one case:\n{diagnostic}"
    );
    Ok(format!("exit {status}, exactly one test passed"))
}

/// Inject only owned malicious defaults into a same-name child, then exercise its real fixture.
/// `exercise` receives an owned observation root; hooks/monitor execution, copied templates, changed
/// sentinel bytes, failed exits and a 30-second timeout all fail without mutating parent environment.
pub(crate) fn check_external_configuration(test: &str, exercise: impl FnOnce(&Path)) {
    reject_repository_overrides().expect("configuration regression cannot redirect repositories");
    if let Some(root) = std::env::var_os(PROBE_ROOT) {
        if !run_test(test) {
            return;
        }
        let root = Path::new(&root);
        exercise(root);
        fs::write(
            root.join("fixture-completed.txt"),
            b"real Git fixture completed\n",
        )
        .unwrap();
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let hooks = root.join("external-hooks");
    let template = root.join("external-template");
    fs::create_dir_all(&hooks).unwrap();
    fs::create_dir_all(template.join("hooks")).unwrap();
    let marker = root.join("external-program-ran.txt");
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' 'external fixture program executed' > '{}'\nexit 73\n",
        marker
            .to_string_lossy()
            .replace('\\', "/")
            .replace('\'', "'\\''")
    );
    let config = format!(
        "[core]\n\thooksPath = \"{}\"\n\tfsmonitor = \"'{}'\"\n[init]\n\ttemplateDir = \"{}\"\n",
        config_path(&hooks),
        config_path(&hooks.join("fsmonitor")).replace('\'', "'\\''"),
        config_path(&template)
    );
    let retained = [
        (hooks.join("pre-commit"), script.clone()),
        (hooks.join("fsmonitor"), script.clone()),
        (template.join("hooks/pre-commit"), script),
        (
            template.join("template-marker.txt"),
            "external template\n".into(),
        ),
        (root.join("external.config"), config),
    ];
    for (path, value) in &retained {
        fs::write(path, value).unwrap();
        #[cfg(unix)]
        if path
            .file_name()
            .is_some_and(|name| name == "pre-commit" || name == "fsmonitor")
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
    }
    let inherited = ["GIT_CONFIG_GLOBAL", "GIT_CONFIG_SYSTEM", "GIT_TEMPLATE_DIR"]
        .map(|name| (name, std::env::var_os(name)));
    let mut child = exact_test(test).unwrap();
    child
        .env_remove(CHILD_TEST)
        .env_remove(CONFIGURATION)
        .env_remove("GIT_CONFIG_NOSYSTEM")
        .env("GIT_CONFIG_GLOBAL", root.join("external.config"))
        .env("GIT_CONFIG_SYSTEM", root.join("external.config"))
        .env("GIT_TEMPLATE_DIR", &template)
        .env(PROBE_ROOT, root);
    run_exact(child, root, Duration::from_secs(30), None).unwrap();
    assert!(
        root.join("fixture-completed.txt").is_file(),
        "fixture was not exercised"
    );
    assert!(!marker.exists(), "an inherited hook or monitor executed");
    for (path, value) in retained {
        assert_eq!(fs::read(path).unwrap(), value.as_bytes());
    }
    for (name, value) in inherited {
        assert!(
            std::env::var_os(name) == value,
            "parent environment changed: {name}"
        );
    }
}
