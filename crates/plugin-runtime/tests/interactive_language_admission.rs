//! Frozen language packages exercise production admission without executing their components.
use plugin_runtime::{Package, validation};
use serde_json::{Value, json};
use std::path::PathBuf;

/// Preserve the original failure input: a matching target must accept its exact ZIP bytes.
#[test]
#[ignore = "set NANOBUG_INTERACTIVE_LANGUAGE_PACKAGE to the original frozen TypeScript 0.1.15 ZIP"]
fn original_interactive_language_package_requires_the_complete_public_target() {
    let path = PathBuf::from(std::env::var_os("NANOBUG_INTERACTIVE_LANGUAGE_PACKAGE").unwrap());
    let bytes = std::fs::read(path).unwrap();
    let report = validation::check_bytes(&bytes, Default::default());
    assert!(report.passed, "original ZIP: {:?}", report.diagnostics);
    let package = Package::from_bytes(&bytes).unwrap();
    let mut manifest: Value = serde_json::from_slice(&package.files["manifest.json"]).unwrap();
    let provider = manifest["language_servers"]
        .as_array()
        .unwrap()
        .iter()
        .position(|provider| provider["delegate"]["interactive"] == true)
        .expect("the original input must retain interactive preparation");

    // A known field cannot opt into 1.6 behavior while advertising a 1.5-compatible floor.
    manifest["api"]["required"]["language.lsp"] = json!(">=1.5,<2");
    assert!(with_manifest(&package, &manifest).is_err());
    manifest["api"]["required"]["language.lsp"] = json!(">=1.6,<2");
    assert!(with_manifest(&package, &manifest).is_ok());

    // Native preparation authorizes only the closed canonical profile, never arbitrary storage calls.
    let contract = manifest["language_servers"][provider]["delegate"]["contract"]
        .as_str()
        .unwrap()
        .to_owned();
    let method = manifest["language_servers"][provider]["delegate"]["method"]
        .as_str()
        .unwrap()
        .to_owned();
    manifest["plugin_services"]["provides"][&contract]["methods"][&method]["permissions"]
        .as_array_mut()
        .unwrap()
        .push(json!("editor.write"));
    assert!(with_manifest(&package, &manifest).is_err());

    let mut unknown: Value = serde_json::from_slice(&package.files["manifest.json"]).unwrap();
    unknown["language_servers"][provider]["delegate"]["invented"] = json!(true);
    assert!(
        with_manifest(&package, &unknown).is_err(),
        "unknown fields remain closed"
    );
}

/// Change only manifest bytes and use the same production Package validator as installation.
fn with_manifest(package: &Package, manifest: &Value) -> anyhow::Result<Package> {
    let mut files = package.files.clone();
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(manifest).unwrap(),
    );
    Package::from_files(files)
}

/// Static parsing cannot prove internal engines are demandable through their normal native role.
/// Use the unchanged original package and cancel before background preparation can start tools.
#[test]
#[ignore = "set NANOBUG_INTERACTIVE_LANGUAGE_PACKAGE to the original frozen TypeScript 0.1.15 ZIP"]
fn original_internal_engine_is_admitted_as_a_language_document_demand() {
    use plugin_runtime::plugin_protocol::{Environment, language};
    use plugin_runtime::{DemandReason, LanguageRole, Manager};
    let input = PathBuf::from(std::env::var_os("NANOBUG_INTERACTIVE_LANGUAGE_PACKAGE").unwrap());
    let package = Package::read(&input).unwrap();
    let engine = package
        .manifest
        .language_servers
        .iter()
        .find(|item| item.internal)
        .unwrap();
    assert!(
        !engine.primary,
        "the regression must exercise a hidden non-primary engine"
    );
    let profile = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(
        profile.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let availability = manager.availability(&package.manifest.id).unwrap();
    let demand = manager
        .begin_demand(
            &availability,
            DemandReason::LanguageDocument {
                provider: format!("{}/{}", package.manifest.id, engine.id),
                role: LanguageRole::Lsp,
                document: language::DocumentContext {
                    path: "source.ts".into(),
                    generation: 1,
                },
            },
            120_000,
        )
        .expect("a declared internal engine remains callable by public context control");
    demand.cancel().unwrap();
    manager.poll();
    manager.shutdown_checked().unwrap();
}
