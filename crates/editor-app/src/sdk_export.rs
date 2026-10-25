//! Export the host's SDK sources and protocol resources for independent plugin builds.
//!
//! `--plugin-cargo <Cargo.toml> <build|check|test> [Cargo options]` prepares a
//! content-addressed source cache and supplies Cargo's dependency override automatically.
//! `--export-plugin-sdk <directory>` publishes the same inputs for other toolchains.
//! Only protocol Rust sources, Cargo metadata, WIT and JSON resources are embedded.
//! Interface guidance stays beside the public Rust types; Markdown is not embedded or exported.
use anyhow::Context;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const SDK_FILES: &[(&str, &[u8])] = &[
    // Public module declarations must remain independently buildable in the exported SDK.
    (
        "src/language_content.rs",
        include_bytes!("../../plugin-protocol/src/language_content.rs"),
    ),
    (
        "src/presentation.rs",
        include_bytes!("../../plugin-protocol/src/presentation.rs"),
    ),
    (
        "src/ui/menus.rs",
        include_bytes!("../../plugin-protocol/src/ui/menus.rs"),
    ),
    (
        "src/ui/workbench_tests.rs",
        include_bytes!("../../plugin-protocol/src/ui/workbench_tests.rs"),
    ),
    (
        "src/api/negotiation.rs",
        include_bytes!("../../plugin-protocol/src/api/negotiation.rs"),
    ),
    // Lifecycle owners are ordinary SDK source; no new imported capability or permission is needed.
    (
        "src/api/guest/resources.rs",
        include_bytes!("../../plugin-protocol/src/api/guest/resources.rs"),
    ),
    (
        "src/api/guest/intent.rs",
        include_bytes!("../../plugin-protocol/src/api/guest/intent.rs"),
    ),
    (
        "src/api/guest/lifecycle_tests.rs",
        include_bytes!("../../plugin-protocol/src/api/guest/lifecycle_tests.rs"),
    ),
    (
        "src/http/task.rs",
        include_bytes!("../../plugin-protocol/src/http/task.rs"),
    ),
    (
        "src/language_context.rs",
        include_bytes!("../../plugin-protocol/src/language_context.rs"),
    ),
    // Registered context references are part of the same independently buildable public module.
    (
        "src/language_context/registry.rs",
        include_bytes!("../../plugin-protocol/src/language_context/registry.rs"),
    ),
    // Independent language guests share the same bounded native control contract.
    (
        "src/language_control.rs",
        include_bytes!("../../plugin-protocol/src/language_control.rs"),
    ),
    // Old and new independent guests share the same documented demand/default lifecycle contract.
    (
        "src/activation.rs",
        include_bytes!("../../plugin-protocol/src/activation.rs"),
    ),
    // Stable diagnostics and experimental direct callbacks ship the identical versioned contracts.
    (
        "src/diagnostics.rs",
        include_bytes!("../../plugin-protocol/src/diagnostics.rs"),
    ),
    (
        "src/direct.rs",
        include_bytes!("../../plugin-protocol/src/direct.rs"),
    ),
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
    // General cloud and private-state contracts accompany every independent guest SDK.
    (
        "src/http.rs",
        include_bytes!("../../plugin-protocol/src/http.rs"),
    ),
    (
        "src/secrets.rs",
        include_bytes!("../../plugin-protocol/src/secrets.rs"),
    ),
    (
        "src/timers.rs",
        include_bytes!("../../plugin-protocol/src/timers.rs"),
    ),
    (
        "src/storage.rs",
        include_bytes!("../../plugin-protocol/src/storage.rs"),
    ),
    // Document contracts ship in the exact same SDK used by the host consumer.
    (
        "src/api/documents.rs",
        include_bytes!("../../plugin-protocol/src/api/documents.rs"),
    ),
    (
        "src/api/comparisons.rs",
        include_bytes!("../../plugin-protocol/src/api/comparisons.rs"),
    ),
    (
        "src/commands.rs",
        include_bytes!("../../plugin-protocol/src/commands.rs"),
    ),
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
    (
        "src/debug.rs",
        include_bytes!("../../plugin-protocol/src/debug.rs"),
    ),
    (
        "src/debug-session.json",
        include_bytes!("../../plugin-protocol/src/debug-session.json"),
    ),
    (
        "src/debug-inspection.json",
        include_bytes!("../../plugin-protocol/src/debug-inspection.json"),
    ),
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
        "src/ui/scroll_tests.rs",
        include_bytes!("../../plugin-protocol/src/ui/scroll_tests.rs"),
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
        "src/ui/canvas/columns.rs",
        include_bytes!("../../plugin-protocol/src/ui/canvas/columns.rs"),
    ),
    // Scene consumers receive the same bounded value contracts used by host admission.
    (
        "src/ui/canvas/scene.rs",
        include_bytes!("../../plugin-protocol/src/ui/canvas/scene.rs"),
    ),
    (
        "src/ui/canvas/scene/geometry.rs",
        include_bytes!("../../plugin-protocol/src/ui/canvas/scene/geometry.rs"),
    ),
    (
        "src/ui/canvas/scene/style.rs",
        include_bytes!("../../plugin-protocol/src/ui/canvas/scene/style.rs"),
    ),
    (
        "src/ui/canvas/scene_tests.rs",
        include_bytes!("../../plugin-protocol/src/ui/canvas/scene_tests.rs"),
    ),
    (
        "src/ui/canvas/text_measure.rs",
        include_bytes!("../../plugin-protocol/src/ui/canvas/text_measure.rs"),
    ),
    (
        "src/ui/text_measure_tests.rs",
        include_bytes!("../../plugin-protocol/src/ui/text_measure_tests.rs"),
    ),
    (
        "src/ui/text_box_tests.rs",
        include_bytes!("../../plugin-protocol/src/ui/text_box_tests.rs"),
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
        "src/ui/styles.rs",
        include_bytes!("../../plugin-protocol/src/ui/styles.rs"),
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
    // Trees and native marker consumers compile against the same bounded contract as this host.
    (
        "src/tree.rs",
        include_bytes!("../../plugin-protocol/src/tree.rs"),
    ),
    (
        "src/decorations.rs",
        include_bytes!("../../plugin-protocol/src/decorations.rs"),
    ),
    (
        "src/ui/presentation_tests.rs",
        include_bytes!("../../plugin-protocol/src/ui/presentation_tests.rs"),
    ),
];

/// Handle SDK development commands before starting the GUI; Cargo owns build/test options.
///
/// Returns `true` after a handled command, or `false` for normal GUI startup. Missing
/// arguments, unavailable Cargo, failed builds and SDK publication errors are reported.
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
    let target = root.join(digest());
    export(&target)?;
    Ok(target)
}

/// Exact embedded SDK identity, available to static tools without creating a cache directory.
pub(crate) fn digest() -> String {
    let mut digest = Sha256::new();
    for (name, bytes) in SDK_FILES {
        digest.update((name.len() as u64).to_le_bytes());
        digest.update(name.as_bytes());
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
    }
    format!("{:x}", digest.finalize())
}

/// Publish Rust sources, Cargo manifests, WIT and JSON protocol resources into `target`.
///
/// Existing unrelated files are preserved. Unchanged sources retain Cargo freshness;
/// incomplete files are atomically repaired. Directory and write failures propagate.
pub fn export(target: &Path) -> anyhow::Result<()> {
    for (relative, bytes) in SDK_FILES {
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
            return Err(error.error).with_context(|| {
                format!(
                    "Could not publish SDK file {}",
                    editor_core::display_path(&path)
                )
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    /// The guest SDK consists solely of the protocol crate's buildable sources and schemas.
    #[test]
    fn sdk_contains_only_protocol_source_and_schemas() {
        assert!(SDK_FILES.iter().all(|(name, _)| *name == "Cargo.toml"
            || name.ends_with(".rs")
            || name.ends_with(".wit")
            || name.ends_with(".json")));
        assert!(SDK_FILES.iter().any(|(name, _)| *name == "src/tree.rs"));
        assert!(
            SDK_FILES
                .iter()
                .any(|(name, _)| *name == "src/decorations.rs")
        );
    }

    use super::*;

    /// Independent SDKs carry the guest contract as source comments, never embedded reader Markdown.
    #[test]
    fn independent_sdk_exports_only_protocol_source_resources() {
        fn inspect(path: &Path) {
            for entry in std::fs::read_dir(path).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    inspect(&path);
                } else {
                    assert!(
                        matches!(
                            path.extension().and_then(|value| value.to_str()),
                            Some("rs" | "toml" | "wit" | "json")
                        ),
                        "Independent SDK contains a non-source resource: {}",
                        path.display()
                    );
                }
            }
        }
        let root = tempfile::tempdir().unwrap();
        export(root.path()).unwrap();
        inspect(root.path());
        for required in [
            "Cargo.toml",
            "src/lib.rs",
            "src/activation.rs",
            "src/settings.rs",
            "src/language_context.rs",
            "src/language_context/registry.rs",
            "wit/plugin.wit",
            "src/http.rs",
            "src/secrets.rs",
            "src/storage.rs",
            "src/timers.rs",
            "src/diagnostics.rs",
            "src/direct.rs",
            "src/workspace.rs",
            "src/api/documents.rs",
            "src/api/comparisons.rs",
            "src/commands.rs",
            "src/interaction.rs",
            "src/configurations.rs",
            "src/configurations/command_form.rs",
            "src/debug.rs",
            "src/targets.rs",
            "src/debug-session.json",
            "src/run-targets.json",
        ] {
            assert!(
                root.path().join(required).is_file(),
                "Missing public SDK resource {required}"
            );
        }
    }

    /// Concurrent first-use exports never publish partial source bytes or divergent resources.
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

    /// A fresh export contains compilable sources and schemas, without bundled reader documents.
    #[test]
    fn fresh_sdk_export_contains_only_build_inputs() {
        let root = tempfile::tempdir().unwrap();
        export(root.path()).unwrap();
        let mut pending = vec![root.path().to_owned()];
        let mut files = Vec::new();
        while let Some(directory) = pending.pop() {
            for entry in std::fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    assert!(
                        matches!(
                            path.extension().and_then(|extension| extension.to_str()),
                            Some("rs" | "toml" | "json" | "wit")
                        ),
                        "unexpected exported build input: {}",
                        path.display()
                    );
                    files.push(path);
                }
            }
        }
        assert!(!files.is_empty());
        for required in [
            "Cargo.toml",
            "src/lib.rs",
            "wit/plugin.wit",
            "src/run-targets.json",
            "src/debug-session.json",
        ] {
            assert!(root.path().join(required).is_file(), "missing {required}");
        }
        let source = std::fs::read_to_string(root.path().join("src/lib.rs")).unwrap();
        assert!(
            source.contains("--plugin-cargo"),
            "SDK usage stays in source comments"
        );
    }

    /// Exporting into a caller-owned directory must not delete their existing notes.
    #[test]
    fn sdk_export_preserves_unrelated_caller_files() {
        let root = tempfile::tempdir().unwrap();
        let note = root.path().join("notes.md");
        std::fs::write(&note, b"caller-owned notes").unwrap();
        export(root.path()).unwrap();
        assert_eq!(std::fs::read(note).unwrap(), b"caller-owned notes");
    }

    /// Content-addressed caches retain build freshness and repair interrupted source/schema publication.
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
