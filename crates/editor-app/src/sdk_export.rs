//! Supply the editor's compiled-in contract to independent Cargo plugin projects.
//!
//! Only protocol Rust sources, Cargo metadata, WIT and JSON resources are embedded.
//! Usage, permissions and failure semantics live in source comments beside the public API.
use anyhow::Context;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const SDK_FILES: &[(&str, &[u8])] = &[
    // Files and recovery receipts use the same public contract in every independent consumer.
    (
        "src/workspace.rs",
        include_bytes!("../../plugin-protocol/src/workspace.rs"),
    ),
    // Independent projects receive the host's exact original-revision multi-range contract.
    (
        "src/api/edits.rs",
        include_bytes!("../../plugin-protocol/src/api/edits.rs"),
    ),
    // Document contracts ship in the exact same SDK used by the host consumer.
    (
        "src/api/documents.rs",
        include_bytes!("../../plugin-protocol/src/api/documents.rs"),
    ),
    // Typed commands and native menu declarations share the actual independent guest SDK.
    (
        "src/commands.rs",
        include_bytes!("../../plugin-protocol/src/commands.rs"),
    ),
    // Independent guests receive the identical native interaction types as the runtime.
    (
        "src/interaction.rs",
        include_bytes!("../../plugin-protocol/src/interaction.rs"),
    ),
    (
        "src/api/preference_binding.rs",
        include_bytes!("../../plugin-protocol/src/api/preference_binding.rs"),
    ),
    (
        "src/ui/layout_tests.rs",
        include_bytes!("../../plugin-protocol/src/ui/layout_tests.rs"),
    ),
    // Public configuration providers receive the identical template, native form and validation types.
    (
        "src/configurations.rs",
        include_bytes!("../../plugin-protocol/src/configurations.rs"),
    ),
    (
        "src/configurations/command_form.rs",
        include_bytes!("../../plugin-protocol/src/configurations/command_form.rs"),
    ),
    // Independent target providers use the same exact schemas as the host consumer.
    (
        "src/targets.rs",
        include_bytes!("../../plugin-protocol/src/targets.rs"),
    ),
    (
        "src/run-targets.json",
        include_bytes!("../../plugin-protocol/src/run-targets.json"),
    ),
    // The canonical optional debug signatures are packaged with their Rust SDK accessor.
    (
        "src/debug.rs",
        include_bytes!("../../plugin-protocol/src/debug.rs"),
    ),
    (
        "src/debug-session.json",
        include_bytes!("../../plugin-protocol/src/debug-session.json"),
    ),
    // Independent providers receive the same bounded observation contract as the host.
    (
        "src/execution.rs",
        include_bytes!("../../plugin-protocol/src/execution.rs"),
    ),
    (
        "src/api/viewport.rs",
        include_bytes!("../../plugin-protocol/src/api/viewport.rs"),
    ),
    (
        "src/api/navigation.rs",
        include_bytes!("../../plugin-protocol/src/api/navigation.rs"),
    ),
    (
        "src/service.rs",
        include_bytes!("../../plugin-protocol/src/service.rs"),
    ),
    (
        "src/ui/events.rs",
        include_bytes!("../../plugin-protocol/src/ui/events.rs"),
    ),
    (
        "src/ui/tools.rs",
        include_bytes!("../../plugin-protocol/src/ui/tools.rs"),
    ),
    (
        "src/ui/tools_tests.rs",
        include_bytes!("../../plugin-protocol/src/ui/tools_tests.rs"),
    ),
    (
        "src/ui/canvas.rs",
        include_bytes!("../../plugin-protocol/src/ui/canvas.rs"),
    ),
    (
        "src/ui/visual_viewport.rs",
        include_bytes!("../../plugin-protocol/src/ui/visual_viewport.rs"),
    ),
    (
        "src/dependencies.rs",
        include_bytes!("../../plugin-protocol/src/dependencies.rs"),
    ),
    (
        "src/language.rs",
        include_bytes!("../../plugin-protocol/src/language.rs"),
    ),
    // Pure structure providers build independently against the same versioned data types as the host.
    (
        "src/structure.rs",
        include_bytes!("../../plugin-protocol/src/structure.rs"),
    ),
    (
        "src/process.rs",
        include_bytes!("../../plugin-protocol/src/process.rs"),
    ),
    (
        "src/settings.rs",
        include_bytes!("../../plugin-protocol/src/settings.rs"),
    ),
    (
        "src/api.rs",
        include_bytes!("../../plugin-protocol/src/api.rs"),
    ),
    (
        "src/api/guest.rs",
        include_bytes!("../../plugin-protocol/src/api/guest.rs"),
    ),
    (
        "src/api/preferences.rs",
        include_bytes!("../../plugin-protocol/src/api/preferences.rs"),
    ),
    (
        "src/ui/controls.rs",
        include_bytes!("../../plugin-protocol/src/ui/controls.rs"),
    ),
    (
        "Cargo.toml",
        include_bytes!("../../plugin-protocol/Cargo.toml"),
    ),
    (
        "src/lib.rs",
        include_bytes!("../../plugin-protocol/src/lib.rs"),
    ),
    (
        "src/ui.rs",
        include_bytes!("../../plugin-protocol/src/ui.rs"),
    ),
    (
        "src/ui/validate.rs",
        include_bytes!("../../plugin-protocol/src/ui/validate.rs"),
    ),
    // Incremental native publication is part of the same independently exported public SDK.
    (
        "src/ui/incremental.rs",
        include_bytes!("../../plugin-protocol/src/ui/incremental.rs"),
    ),
    (
        "src/ui/tests.rs",
        include_bytes!("../../plugin-protocol/src/ui/tests.rs"),
    ),
    (
        "src/ui/images_tests.rs",
        include_bytes!("../../plugin-protocol/src/ui/images_tests.rs"),
    ),
    (
        "src/api/images_tests.rs",
        include_bytes!("../../plugin-protocol/src/api/images_tests.rs"),
    ),
    (
        "wit/plugin.wit",
        include_bytes!("../../plugin-protocol/wit/plugin.wit"),
    ),
];

/// Handle development commands before starting the GUI. Cargo owns build/test options.
pub fn run_cli() -> anyhow::Result<bool> {
    let mut args = std::env::args_os().skip(1);
    match args.next().as_deref().and_then(|arg| arg.to_str()) {
        Some("--plugin-cargo") => {
            let manifest = args
                .next()
                .context("usage: --plugin-cargo <Cargo.toml> <build|check|test> [Cargo options]")?;
            let operation = args
                .next()
                .context("missing Cargo operation: build, check or test")?;
            anyhow::ensure!(
                matches!(operation.to_str(), Some("build" | "check" | "test")),
                "supported plugin Cargo operations: build, check, test"
            );
            let manifest = PathBuf::from(manifest)
                .canonicalize()
                .context("plugin Cargo.toml does not exist")?;
            anyhow::ensure!(manifest.is_file(), "plugin manifest must be a file");
            let status = Command::new("cargo")
                .args(cargo_args()?)
                .arg(operation)
                .arg("--manifest-path")
                .arg(manifest)
                .args(args)
                .status()
                .context("could not start Cargo; install the Rust toolchain")?;
            anyhow::ensure!(status.success(), "plugin Cargo command failed: {status}");
        }
        Some("--export-plugin-sdk") => {
            // Keep explicit exports available for other language toolchains and inspection.
            let target = args
                .next()
                .context("--export-plugin-sdk requires an output directory")?;
            anyhow::ensure!(args.next().is_none(), "unexpected SDK export argument");
            export(Path::new(&target))?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// Build and language analysis resolve the exact same host-owned, versioned SDK.
pub(crate) fn cargo_args() -> anyhow::Result<Vec<String>> {
    Ok(vec![
        "--config".into(),
        cargo_config()?.to_string_lossy().into_owned(),
    ])
}

/// Describe the same immutable public contract used by Cargo, without granting guests filesystem handles.
pub(crate) fn descriptor() -> anyhow::Result<plugin_runtime::plugin_protocol::api::SdkDescriptor> {
    let config = cargo_config()?;
    let root = config
        .parent()
        .context("SDK configuration has no directory")?;
    let digest = root
        .file_name()
        .and_then(|name| name.to_str())
        .context("SDK cache identity is not Unicode")?;
    Ok(plugin_runtime::plugin_protocol::api::SdkDescriptor {
        digest: digest.to_owned(),
        root: root.display().to_string(),
        cargo_config: config.display().to_string(),
    })
}

/// Keep Cargo configuration beside the host cache, shared by metadata, checks and builds.
pub(crate) fn cargo_config() -> anyhow::Result<PathBuf> {
    let cache_root = dirs::cache_dir()
        .context("no user cache directory available")?
        .join("MeEditor")
        .join("plugin-sdk");
    let sdk = prepare_cache(&cache_root)?;
    // JSON string quoting also escapes Windows paths in TOML configuration values.
    let contents = format!(
        "# Host-managed plugin protocol dependency.\n[patch.crates-io]\nplugin-protocol = {{ path = {} }}\n",
        serde_json::to_string(&sdk)?
    );
    let config = sdk.join("host-cargo.toml");
    publish(&config, contents.as_bytes())?;
    Ok(config)
}

/// Content-addressed caches let different editor versions build plugins independently.
fn prepare_cache(root: &Path) -> anyhow::Result<PathBuf> {
    let mut digest = Sha256::new();
    for (name, raw) in SDK_FILES {
        let bytes = *raw;
        digest.update((name.len() as u64).to_le_bytes());
        digest.update(name.as_bytes());
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
    }
    let target = root.join(format!("{:x}", digest.finalize()));
    export(&target)?;
    Ok(target)
}

/// Preserve unchanged files for Cargo freshness and atomically repair incomplete caches.
pub fn export(target: &Path) -> anyhow::Result<()> {
    for (relative, raw) in SDK_FILES {
        let bytes = *raw;
        let path = target.join(relative);
        publish(&path, bytes)?;
    }
    Ok(())
}

/// Concurrent first-use builds can prepare the identical content-addressed SDK. On Windows,
/// replacement may fail while another reader opens the winner; identical committed bytes are
/// already success. Other write failures still report the exact path and preserve existing data.
fn publish(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if std::fs::read(path).is_ok_and(|existing| existing == bytes) {
        return Ok(());
    }
    let parent = path.parent().context("SDK file needs a parent")?;
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    if let Err(error) = file.persist(path) {
        if !std::fs::read(path).is_ok_and(|existing| existing == bytes) {
            return Err(error.error)
                .with_context(|| format!("Could not publish SDK file {}", path.display()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two independent host invocations can publish a fresh SDK concurrently without partial files.
    #[test]
    fn concurrent_sdk_exports_converge() {
        let root = tempfile::tempdir().unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
        let workers = (0..4)
            .map(|_| {
                let path = root.path().to_owned();
                let ready = barrier.clone();
                std::thread::spawn(move || {
                    ready.wait();
                    export(&path)
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().unwrap().unwrap();
        }
        for (relative, bytes) in SDK_FILES {
            assert_eq!(std::fs::read(root.path().join(relative)).unwrap(), *bytes);
        }
    }

    #[test]
    fn cached_contract_is_complete_reused_and_repairs_corruption() {
        let root = tempfile::tempdir().unwrap();
        let sdk = prepare_cache(root.path()).unwrap();
        for (relative, bytes) in SDK_FILES {
            assert_eq!(std::fs::read(sdk.join(relative)).unwrap(), *bytes);
        }
        let manifest = sdk.join("Cargo.toml");
        let modified = manifest.metadata().unwrap().modified().unwrap();
        assert_eq!(prepare_cache(root.path()).unwrap(), sdk);
        assert_eq!(manifest.metadata().unwrap().modified().unwrap(), modified);
        std::fs::write(&manifest, b"interrupted or modified cache").unwrap();
        std::fs::remove_file(sdk.join("wit/plugin.wit")).unwrap();
        prepare_cache(root.path()).unwrap();
        for (relative, bytes) in SDK_FILES {
            assert_eq!(std::fs::read(sdk.join(relative)).unwrap(), *bytes);
        }
    }
}
