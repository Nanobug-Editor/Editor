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
    ensure!(
        !sidebar::title_actions::requested()
            || std::env::var_os("NANOBUG_GIT_NATIVE_SHARED_SIDEBAR").is_none(),
        "title actions and full sidebar workflows are mutually exclusive"
    );
    // Each narrow receipt has one scope; inherited diagnostic flags cannot shorten another workflow.
    let narrow = [
        commit_panel::requested(),
        workspace_presentation::requested(),
        exclusive::requested(),
        sidebar::requested(),
        caption_controls::requested(),
        std::env::var_os("NANOBUG_GIT_NATIVE_IDLE_RETENTION").is_some(),
        std::env::var_os("NANOBUG_GIT_NATIVE_HISTORY_ONLY").is_some(),
        std::env::var_os("NANOBUG_GIT_NATIVE_COMPARISON_ONLY").is_some(),
    ];
    ensure!(
        narrow.into_iter().filter(|active| *active).count() <= 1,
        "native diagnostic workflows are mutually exclusive"
    );
    ensure!(
        std::env::var_os("NANOBUG_GIT_NATIVE_HISTORY_PHASES").is_none()
            || std::env::var_os("NANOBUG_GIT_NATIVE_HISTORY_ONLY").is_some(),
        "history phases require the history-only workflow"
    );
    let candidate = std::env::var_os("NANOBUG_GIT_CANDIDATE").context("candidate required")?;
    ensure!(
        !history_files::requested()
            || (std::env::var_os("NANOBUG_GIT_NATIVE_HISTORY_ONLY").is_some()
                && std::env::var_os("NANOBUG_GIT_NATIVE_SCROLL_ONLY").is_none()
                && std::env::var_os("NANOBUG_GIT_NATIVE_HISTORY_PHASES").is_none()),
        "history files require their own history-only receipt"
    );
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
    let deadline = Instant::now()
        + Duration::from_secs(
            if caption_controls::requested() || sidebar::requested() || exclusive::requested() {
                180
            } else {
                480
            },
        );
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
    let images: &[&str] = if commit_panel::requested() {
        ensure!(
            evidence["workflow"] == "commit-panel-only",
            "commit receipt has wrong scope"
        );
        ensure!(
            evidence["observations"]["checkbox_partial_stage_unstage_and_draft"] == true,
            "commit workflow incomplete"
        );
        &[
            "commit-default.png",
            "commit-checked.png",
            "commit-final.png",
        ]
    } else if workspace_presentation::requested() {
        ensure!(
            evidence["workflow"] == "workspace-presentation-only",
            "presentation receipt has wrong scope"
        );
        ensure!(
            evidence["observations"]["workspace_presentation_complete"] == true,
            "presentation workflow incomplete"
        );
        &[
            "presentation-files.png",
            "presentation-head-menu.png",
            "presentation-overflow.png",
            "presentation-withdrawn.png",
        ]
    } else if exclusive::replacement::requested() {
        ensure!(
            evidence["workflow"] == "exclusive-full-replacement-only"
                && evidence["observations"]["exclusive_full_replacement_complete"] == true,
            "full replacement workflow incomplete or wrong receipt scope"
        );
        &[
            "replacement-defaults.png",
            "replacement-ordinary-preview.png",
            "replacement-ordinary-dropped.png",
            "replacement-terminal-preview.png",
            "replacement-terminal-dropped.png",
            "replacement-restored.png",
        ]
    } else if exclusive::requested() {
        ensure!(
            evidence["workflow"] == "exclusive-dock-only",
            "exclusive receipt has wrong scope"
        );
        ensure!(
            evidence["observations"]["exclusive_complete"] == true,
            "exclusive workflow incomplete"
        );
        &[
            "exclusive-open.png",
            "exclusive-shared.png",
            "exclusive-reopened.png",
        ]
    } else if sidebar::title_actions::requested() {
        ensure!(
            evidence["workflow"] == "title-actions-only"
                && evidence["observations"]["title_actions_hover_and_focus"]["explorer"] == true
                && evidence["observations"]["title_actions_hover_and_focus"]["hover_preserves_editor_focus"]
                    == true
                && evidence["observations"]["title_actions_hover_and_focus"]["focus_retains_after_pointer_exit"]
                    == true
                && evidence["observations"]["title_actions_hover_and_focus"]["hidden_after_both_leave"]
                    == true,
            "title actions workflow incomplete or wrong receipt scope"
        );
        &[
            "title-actions-explorer-hover.png",
            "title-actions-plugin-hover.png",
        ]
    } else if sidebar::requested() {
        ensure!(
            evidence["workflow"] == "shared-sidebar-only",
            "sidebar receipt has wrong scope"
        );
        ensure!(
            evidence["observations"]["shared_sidebar_complete"]["three_independent_peers"] == true,
            "shared-sidebar workflow incomplete"
        );
        &[
            "sidebar-before.png",
            "sidebar-after-drag.png",
            "sidebar-after-reopen.png",
        ]
    } else if caption_controls::requested() {
        ensure!(
            evidence["workflow"] == "caption-controls-loaded-commit",
            "caption receipt has wrong scope"
        );
        ensure!(
            evidence["observations"]["caption_controls_complete"]["actual_os_caption_mouse"]
                == true,
            "loaded caption workflow incomplete"
        );
        &[
            "caption-loaded-before.png",
            "caption-loaded-after-restore.png",
        ]
    } else if std::env::var_os("NANOBUG_GIT_NATIVE_IDLE_RETENTION").is_some() {
        &[
            "idle-state-before.png",
            "idle-state-after.png",
            "idle-menu-before.png",
            "idle-menu-after.png",
        ]
    } else if std::env::var_os("NANOBUG_GIT_NATIVE_COMPARISON_ONLY").is_some() {
        ensure!(
            evidence["workflow"] == "comparison-only",
            "comparison receipt has wrong scope"
        );
        ensure!(
            evidence["observations"]["comparison_preview_without_file_tab"].is_object(),
            "preview-only tab observation missing"
        );
        ensure!(
            evidence["observations"]["saved_and_refreshed_before_staging"] == true,
            "native comparison review did not finish"
        );
        &["workbench.png", "workbench-ime.png"]
    } else if history_files::requested() {
        let receipt = &evidence["observations"]["history_files"];
        ensure!(
            evidence["workflow"] == "history-files-only",
            "history file receipt has wrong scope"
        );
        ensure!(
            receipt["total"] == case.label("33 个文件", "33 files")
                && receipt["count_includes_unloaded_files"] == true
                && receipt["keyboard_disclosure"] == true
                && receipt["collapsed_root_retained"] == true
                && receipt["selection_retained"] == true
                && receipt["scroll_retained"] == true,
            "counted history tree workflow incomplete"
        );
        &[
            "history-files-expanded.png",
            "history-files-exact-source.png",
            "history-files-refreshed.png",
        ]
    } else if std::env::var_os("NANOBUG_GIT_NATIVE_SCROLL_ONLY").is_some() {
        &["history-scroll.png"]
    } else if std::env::var_os("NANOBUG_GIT_NATIVE_HISTORY_ONLY").is_some() {
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
