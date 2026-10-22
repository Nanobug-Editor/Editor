//! Opt-in public CLI regressions use an isolated native child, not a simulated shutdown receipt.
use plugin_runtime::development::report::{CheckStatus, DevelopmentReport, Termination};
use std::{
    io::Write,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

/// The controller's OS job owns its GUI child; panic cleanup cannot leave that test instance alive.
struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

/// Atomic report replacement permits polling without interpreting stdout as lifecycle evidence.
fn read(path: &Path) -> Option<DevelopmentReport> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// Bound every external wait, including startup failure and the controller's force grace.
fn until(label: &str, log: &Path, mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !predicate() {
        assert!(
            Instant::now() < deadline,
            "native CLI {label} timed out: {}",
            std::fs::read_to_string(log).unwrap_or_default()
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// A resource-only package keeps this close-routing regression independent of a compiler/WASM cache.
/// Executable activation is covered separately by the frozen lifecycle consumers and native H01.
#[test]
#[ignore = "opens an isolated Windows GUI; set NANOBUG_TOOLING_HOST to the exact built editor-app.exe"]
fn developer_tooling_native_report_stop_cancel_and_reload_failure() {
    let executable = std::env::var_os("NANOBUG_TOOLING_HOST")
        .expect("NANOBUG_TOOLING_HOST must identify the host under test");
    for mode in ["stop", "cancel", "reload"] {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("project");
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::write(
            project.join("nanobug-plugin.json"),
            r#"{"version":1,"manifest":"manifest.json","output":".","assets":[{"source":"plugin.toml","destination":"plugin.toml"}]}"#,
        )
        .unwrap();
        let mut manifest = serde_json::json!({
            "id":"report-regression", "name":"Report regression", "version":"0.1.0",
            "protocol":7, "api":{"base":"^1"}, "permissions":[], "storage_limit":1024,
            "contributions":"plugin.toml"
        });
        std::fs::write(project.join("plugin.toml"), "[plugin]\nid='report-regression'\nname='Report regression'\nversion='0.1.0'\nhost_version='>=0.1.0'\n").unwrap();
        std::fs::write(
            project.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let report = root.path().join("report.json");
        let log_path = root.path().join("controller.log");
        let log = std::fs::File::create(&log_path).unwrap();
        let mut command = Command::new(&executable);
        command
            .arg("--plugin-dev")
            .arg(&project)
            .arg("--profile")
            .arg(root.path().join("profile"))
            .arg("--workspace")
            .arg(&workspace)
            .arg("--report")
            .arg(&report)
            .stdin(Stdio::piped())
            .stdout(log.try_clone().unwrap())
            .stderr(log);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000); // Keep the controller's console hidden.
        }
        let mut child = Running(command.spawn().unwrap());
        until("startup", &log_path, || {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "controller exited before startup: {}",
                std::fs::read_to_string(&log_path).unwrap_or_default()
            );
            read(&report).is_some_and(|report| report.startup == CheckStatus::Passed)
        });
        if mode == "reload" {
            manifest["protocol"] = 999.into();
            std::fs::write(
                project.join("manifest.json"),
                serde_json::to_vec(&manifest).unwrap(),
            )
            .unwrap();
            child
                .0
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"reload\n")
                .unwrap();
            until("reload", &log_path, || {
                read(&report).is_some_and(|report| report.admission == CheckStatus::Failed)
            });
        }
        let input = if mode == "cancel" {
            b"cancel\n".as_slice()
        } else {
            b"stop\n".as_slice()
        };
        child.0.stdin.as_mut().unwrap().write_all(input).unwrap();
        let mut status = None;
        until("exit", &log_path, || {
            status = child.0.try_wait().unwrap();
            status.is_some()
        });
        let report = read(&report).unwrap();
        let expected = match mode {
            "cancel" => 3,
            "reload" => 1,
            _ => 0,
        };
        assert_eq!(status.unwrap().code(), Some(expected), "{mode}: {report:?}");
        assert_eq!(report.cleanup, CheckStatus::Passed, "{mode}: {report:?}");
        assert_eq!(report.business, CheckStatus::NotExercised);
        assert_eq!(
            report.termination,
            Some(if mode == "cancel" {
                Termination::Cancelled
            } else {
                Termination::Normal
            })
        );
        if mode == "stop" {
            assert_eq!(report.status, CheckStatus::Passed);
        }
    }
}
