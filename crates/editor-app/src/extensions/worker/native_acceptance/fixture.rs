//! Fixed Git histories and conflicts remain entirely inside a parent-owned temporary workspace.
use super::*;
use std::{
    io::Write as _,
    process::{Command, Stdio},
};

mod isolation_tests;
mod topology;

pub(super) const REVIEW_PATH: &str = "src/a-review.txt";
pub(super) const CONFLICT_PATH: &str = "conflicts/merge.txt";
const BASE: &str =
    "heading\nbase conflict\nkeep one\nkeep two\nbase left\nkeep three\nbase right\nend\n";

/// Source strings are fixture facts; all mutable UI text remains owned by native EditorState.
pub(super) struct Fixture {
    pub workspace: PathBuf,
    pub profile: PathBuf,
    pub output: PathBuf,
    pub package_digest: String,
    pub review: String,
}
impl Fixture {
    /// Prepare local branches and a scrollable history without a remote, credentials or global Git writes.
    pub fn prepare(case: Case) -> anyhow::Result<Self> {
        super::super::super::git_fixture_environment::reject_repository_overrides()?;
        let root = PathBuf::from(
            std::env::var_os("NANOBUG_GIT_NATIVE_ROOT").context("parent-owned root required")?,
        );
        let output = PathBuf::from(
            std::env::var_os("NANOBUG_GIT_NATIVE_OUTPUT").context("native output required")?,
        );
        let candidate = PathBuf::from(
            std::env::var_os("NANOBUG_GIT_CANDIDATE").context("independent Git ZIP required")?,
        );
        let workspace = root.join("workspace");
        let profile = root.join("profile");
        let git_configuration = prepare_git_configuration(&root)?;
        fs::create_dir_all(workspace.join("src"))?;
        fs::create_dir_all(workspace.join("conflicts"))?;
        fs::create_dir_all(&profile)?;
        fs::create_dir_all(&output)?;
        // Even the first init must not copy an inherited template's hooks or attributes. The
        // repository-local hook path also protects later Git calls made through the real plugin.
        let template = git_configuration.join("empty-template");
        let hooks = git_configuration.join("empty-hooks");
        git(
            &workspace,
            &[
                "init",
                "--template",
                template
                    .to_str()
                    .context("fixture template path is not UTF-8")?,
                "-b",
                "main",
            ],
        )?;
        git(
            &workspace,
            &[
                "config",
                "--local",
                "core.hooksPath",
                hooks.to_str().context("fixture hook path is not UTF-8")?,
            ],
        )?;
        for args in [
            &["config", "--local", "core.autocrlf", "false"][..],
            &["config", "--local", "commit.gpgsign", "false"],
            &["config", "--local", "tag.gpgsign", "false"],
            &["config", "--local", "user.name", "Native acceptance"],
            &["config", "--local", "user.email", "native@example.invalid"],
        ] {
            git(&workspace, args)?;
        }
        let original = (0..80)
            .map(|i| format!("review line {i:02}\n"))
            .collect::<String>();
        fs::write(workspace.join(REVIEW_PATH), &original)?;
        fs::write(workspace.join(CONFLICT_PATH), BASE)?;
        git(&workspace, &["add", "--", "."])?;
        git(&workspace, &["commit", "-m", "Initial native review"])?;
        git(&workspace, &["switch", "-c", "incoming"])?;
        fs::write(
            workspace.join(CONFLICT_PATH),
            BASE.replace("base conflict", "incoming block")
                .replace("base right", "incoming only"),
        )?;
        git(&workspace, &["commit", "-am", "Incoming native changes"])?;
        git(&workspace, &["switch", "main"])?;
        fs::write(
            workspace.join(CONFLICT_PATH),
            BASE.replace("base conflict", "local block")
                .replace("base left", "local only"),
        )?;
        git(&workspace, &["commit", "-am", "Local native changes"])?;
        history(&workspace)?;
        if std::env::var_os("NANOBUG_GIT_NATIVE_GRAPH_COLORS").is_some() {
            topology::prepare(&workspace)?;
        }
        if std::env::var_os("NANOBUG_GIT_NATIVE_BRANCH_COMPACT").is_some() {
            // Sibling directories sort before ordinary leaves, so the current marker must explicitly
            // lead this real lazy page. All refs share HEAD and preserve the fixture's source tree.
            for branch in ["aa-folder/a-first", "aa-folder/z-topic", "z-last"] {
                git(
                    &workspace,
                    &["update-ref", &format!("refs/heads/{branch}"), "HEAD"],
                )?;
            }
        }
        // A private remote-tracking ref exercises grouping without a URL, transport or fetch.
        // The common helper removes caller-selected Git directories and indexes for every command.
        git(
            &workspace,
            &["update-ref", "refs/remotes/fixture/main", "HEAD"],
        )?;
        let review = original
            .replace(
                "review line 02\n",
                "review line 02 changed 中😀\n新增一行\n",
            )
            .replace("review line 65\n", "review line 65 changed\n");
        fs::write(workspace.join(REVIEW_PATH), &review)?;
        for i in 0..28 {
            fs::write(
                workspace.join(format!("src/z-note-{i:02}.txt")),
                "untracked review item\n",
            )?;
        }
        let package = Package::read(&candidate)
            .context("native fixture package validation after Git preparation")?;
        let package_digest = format!("{:x}", Sha256::digest(fs::read(&candidate)?));
        let mut manager = Manager::open(
            extensions::profile_runtime_root(&profile),
            plugin_runtime::plugin_protocol::Environment {
                workspace: workspace.display().to_string(),
                os: "windows".into(),
                locale: case.locale.into(),
                ..Default::default()
            },
        )?;
        manager.install(&package, package.manifest.permissions.clone())?;
        if comparison_timing::requested() {
            // Exercise real dynamic grammar initialization in both measured binaries.
            let language = if let Some(path) = std::env::var_os("NANOBUG_DIFF_TIMING_LANGUAGE") {
                Package::read(Path::new(&path))?
            } else {
                crate::extensions::language_tests::packages::language_package("snapshot-language")
            };
            manager.install(&language, language.manifest.permissions.clone())?;
        }
        manager.shutdown_checked()?;
        drop(manager);
        Ok(Self {
            workspace,
            profile,
            output,
            package_digest,
            review,
        })
    }

    /// An external merge creates genuine stage-1/2/3 entries for the plugin to discover and resolve.
    pub fn enter_conflict(root: &Path) -> anyhow::Result<()> {
        let output = git_command(root)
            .args(["merge", "--no-edit", "incoming"])
            .output()?;
        ensure!(
            !output.status.success() && root.join(".git/MERGE_HEAD").is_file(),
            "fixture merge did not create a real conflict"
        );
        ensure!(
            !git_text(root, &["ls-files", "-u", "--", CONFLICT_PATH])?.is_empty(),
            "missing unmerged index entries"
        );
        Ok(())
    }

    pub fn merged_text() -> String {
        BASE.replace("base conflict", "local block")
            .replace("base left", "local only")
            .replace("base right", "incoming only")
    }
}

/// Fast-import adds a bounded visible log in one process; every row inherits the same reviewed source tree.
fn history(root: &Path) -> anyhow::Result<()> {
    let parent = git_text(root, &["rev-parse", "HEAD"])?;
    let mut input = String::new();
    // Only the pagination diagnostic needs several complete 128-row pages; keep other fixtures small.
    let count = if std::env::var_os("NANOBUG_GIT_NATIVE_HISTORY_PAGINATION").is_some() {
        385
    } else {
        36
    };
    for index in 0..count {
        let title = format!("Review history {index:02}");
        input.push_str(&format!("commit refs/heads/main\ncommitter Native acceptance <native@example.invalid> {} +0000\ndata {}\n{title}\n", 1_800_000_000 + index, title.len()));
        if index == 0 {
            input.push_str(&format!("from {}\n", parent.trim()));
        }
        input.push('\n');
    }
    input.push_str("done\n");
    let mut child = git_command(root)
        .args(["fast-import", "--quiet"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .context("fast-import stdin unavailable")?
        .write_all(input.as_bytes())?;
    let result = child.wait_with_output()?;
    ensure!(
        result.status.success(),
        "fixture fast-import failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(())
}

/// Commands are static argument arrays and use only the selected private repository.
pub(super) fn git(root: &Path, args: &[&str]) -> anyhow::Result<()> {
    git_text(root, args).map(|_| ())
}

/// Read Git's observable result, preserving stderr on failure without printing environment or credentials.
pub(super) fn git_text(root: &Path, args: &[&str]) -> anyhow::Result<String> {
    let output = git_command(root).args(args).output()?;
    ensure!(
        output.status.success(),
        "fixture Git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?)
}

/// Prepare only parent-owned Git defaults. The native parent applies the same empty file to its
/// child's whole environment, so later Manager-launched Git cannot read user/system executable
/// configuration such as hooks, filesystem monitors, attribute filters or signing helpers.
/// `root` must belong to the disposable native parent; return its config directory or the I/O error.
pub(super) fn prepare_git_configuration(root: &Path) -> anyhow::Result<PathBuf> {
    let configuration = root.join("git-configuration");
    fs::create_dir_all(configuration.join("empty-template"))?;
    fs::create_dir_all(configuration.join("empty-hooks"))?;
    fs::write(configuration.join("empty.config"), b"")?;
    Ok(configuration)
}

/// Direct child-entry regressions bypass the native parent, so every factory command also uses
/// the owned empty configuration from its first init. Repository-routing variables remain an
/// independently rejected precondition; these process-local settings never alter global state.
fn git_command(root: &Path) -> Command {
    let configuration = root
        .parent()
        .unwrap_or(root)
        .join("git-configuration/empty.config");
    let mut command = Command::new("git");
    command
        .current_dir(root)
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env("GIT_CONFIG_GLOBAL", &configuration)
        .env("GIT_CONFIG_SYSTEM", &configuration)
        .env("GIT_CONFIG_NOSYSTEM", "1");
    command
}
