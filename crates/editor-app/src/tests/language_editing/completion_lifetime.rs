//! Unknown language resolve replies cross a real Windows scheduler before native atomic application.
use super::*;
use sha2::{Digest as _, Sha256};

/// The child owns one private window/store; the parent admits the ordinary Package and bounds exit.
#[cfg(windows)]
#[test]
#[ignore = "requires Windows/D3D11 and the current javascript fixture archive for its approved Node runtime"]
fn selected_unknown_completion_resolve_rejects_stale_sources_and_revoked_owners_whole() {
    use std::{fs, process::Command, time::Instant};
    const CHILD: &str =
        "extensions::worker::native_selection_formatting_tests::completion_resolve_lifetime_child";
    const RECEIPT: &str =
        "native completion: selected resolve, whole refusals, Undo and cleanup verified";
    let directory = tempfile::tempdir().unwrap();
    let workspace = directory.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let workspace = workspace.canonicalize().unwrap();
    fs::write(
        workspace.join("sample.linked"),
        "// saved 中文😀e\u{301}\r\n",
    )
    .unwrap();
    fs::write(
        workspace.join("related.linked"),
        "export const helper = 1;\r\n",
    )
    .unwrap();
    let profile = directory.path().join("profile");
    let package = editing_fixture::package(
        "delayed-resolve",
        "resolve-language",
        true,
        true,
        &directory.path().join("completion-resolve.jsonl"),
        350,
        "completion-resolve",
    );
    let mut files = package.files;
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    let source = include_bytes!("../fixtures/completion-service.cjs");
    manifest["services"]["analysis"]["installation"]["artifacts"][1]["sha256"] =
        serde_json::json!(format!("{:x}", Sha256::digest(source)));
    manifest["api"]["required"]["language.lsp"] = serde_json::json!("^1.5");
    manifest["api"]["required"]["language.control"] = serde_json::json!("^1");
    manifest["permissions"].as_array_mut().unwrap().extend([
        serde_json::json!("editor.read"),
        serde_json::json!("workspace.read"),
    ]);
    manifest["language_servers"][0]["related_documents"] =
        serde_json::json!("fixture/context/documents");
    manifest["language_servers"][0]["control_methods"] = serde_json::json!({
        "fixture/context/documents": protocol::language_context::related_documents_method()
    });
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    files.insert("native/server.cjs".into(), source.to_vec());
    let package = Package::from_files(files).unwrap();
    let mut manager = Manager::open(
        crate::extensions::profile_runtime_root(&profile),
        protocol::Environment {
            workspace: workspace.display().to_string(),
            os: "windows".into(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    manager.shutdown_checked().unwrap();
    drop(manager);
    let log_path = directory.path().join("native-completion-resolve.log");
    let log = fs::File::create(&log_path).unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            CHILD,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("NANOBUG_NATIVE_COMPLETION_ROOT", directory.path())
        .env("ME_EDITOR_PROFILE_HOME", &profile)
        .env("RUST_BACKTRACE", "1")
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(180);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let log = fs::read_to_string(log_path).unwrap();
    assert!(
        status.is_some_and(|status| status.success()),
        "native completion child failed or stalled: {status:?}\n{log}"
    );
    assert!(
        log.contains(RECEIPT),
        "missing checked native completion receipt:\n{log}"
    );
    eprintln!("{log}");
}
