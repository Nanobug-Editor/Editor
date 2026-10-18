//! Native combinations are isolated processes, so global locale, thread-local UI and HWND lifetimes cannot leak.
use super::*;
use std::process::Command;

const CHILD: &str = "extensions::worker::native_acceptance::git_native_matrix_child";

/// One explicit theme/locale/DPI request; no case changes operating-system display settings.
#[derive(Clone, Copy)]
pub(super) struct Case {
    pub name: &'static str,
    pub dark: bool,
    pub locale: &'static str,
    pub percent: u32,
}
impl Case {
    pub const SMOKE: Self = Self {
        name: "dark-zh-CN-100",
        dark: true,
        locale: "zh-CN",
        percent: 100,
    };
    pub const ALL: [Self; 8] = [
        Self::SMOKE,
        Self {
            name: "dark-zh-CN-150",
            percent: 150,
            ..Self::SMOKE
        },
        Self {
            name: "dark-en-100",
            dark: true,
            locale: "en",
            percent: 100,
        },
        Self {
            name: "dark-en-150",
            dark: true,
            locale: "en",
            percent: 150,
        },
        Self {
            name: "light-zh-CN-100",
            dark: false,
            locale: "zh-CN",
            percent: 100,
        },
        Self {
            name: "light-zh-CN-150",
            dark: false,
            locale: "zh-CN",
            percent: 150,
        },
        Self {
            name: "light-en-100",
            dark: false,
            locale: "en",
            percent: 100,
        },
        Self {
            name: "light-en-150",
            dark: false,
            locale: "en",
            percent: 150,
        },
    ];

    /// Accept only the bounded parent matrix, never reinterpret arbitrary strings as settings or paths.
    pub fn from_environment() -> anyhow::Result<Self> {
        let requested = std::env::var("NANOBUG_GIT_NATIVE_CASE")?;
        Self::ALL
            .into_iter()
            .find(|case| case.name == requested)
            .context("unknown native acceptance case")
    }
    pub fn label(self, zh: &'static str, en: &'static str) -> &'static str {
        if self.locale == "en" { en } else { zh }
    }
}

/// Only this returned Child handle can be terminated; there is no process-name or user-window search.
pub(super) fn run(case: Case) -> anyhow::Result<()> {
    let candidate = std::env::var_os("NANOBUG_GIT_CANDIDATE").context("candidate required")?;
    let output = PathBuf::from(
        std::env::var_os("NANOBUG_GIT_NATIVE_OUTPUT").context("native output required")?,
    )
    .join(case.name);
    fs::create_dir_all(&output)?;
    let prior_log = output.join("native.log");
    if prior_log.exists() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis();
        fs::rename(&prior_log, output.join(format!("native-{stamp}.log")))?;
    }
    // An earlier successful image can never turn a failed new run into a successful receipt.
    fs::write(
        output.join("evidence.json"),
        br#"{"shutdown_complete":false,"state":"running"}"#,
    )?;
    let isolated = tempfile::tempdir()?;
    let git_configuration = fixture::prepare_git_configuration(isolated.path())?;
    let log = fs::File::create(prior_log)?;
    let mut child = Command::new(std::env::current_exe()?)
        .args([
            "--exact",
            CHILD,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("RUST_MIN_STACK", "8388608")
        // Scope all subsequent real plugin/Manager Git children to these owned defaults too.
        // Per-command fixture isolation alone would leave user hooks/filters reachable later.
        .env("GIT_CONFIG_GLOBAL", git_configuration.join("empty.config"))
        .env("GIT_CONFIG_SYSTEM", git_configuration.join("empty.config"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("NANOBUG_GIT_NATIVE_CASE", case.name)
        .env("NANOBUG_GIT_NATIVE_ROOT", isolated.path())
        .env("ME_EDITOR_PROFILE_HOME", isolated.path().join("profile"))
        .env("NANOBUG_GIT_NATIVE_OUTPUT", &output)
        .env("NANOBUG_GIT_CANDIDATE", candidate)
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(480);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!(
                "native {} timed out; see {}",
                case.name,
                output.join("native.log").display()
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    ensure!(
        status.success(),
        "native {} failed; see {}",
        case.name,
        output.join("native.log").display()
    );
    let evidence: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("evidence.json"))?)?;
    ensure!(
        evidence["shutdown_complete"] == true,
        "native shutdown receipt missing"
    );
    ensure!(
        evidence["case"] == case.name,
        "receipt belongs to a different case"
    );
    // The separately named keyboard diagnostic has an explicit, narrower evidence contract.
    let images: &[&str] = if std::env::var_os("NANOBUG_GIT_NATIVE_HISTORY_ONLY").is_some() {
        &["history-keyboard-probe.png"]
    } else {
        &[
            "workbench.png",
            "workbench-ime.png",
            "history.png",
            "conflicts.png",
        ]
    };
    for image in images {
        ensure!(output.join(image).is_file(), "missing native image {image}");
    }
    Ok(())
}
