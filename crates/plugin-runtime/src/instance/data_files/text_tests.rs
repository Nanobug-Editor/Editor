//! Text reads retain disk/path quotas while avoiding the per-byte JSON wire representation.
use super::*;

/// A realistic lockfile-sized input crosses as one UTF-8 string, preserving exact bytes.
#[test]
fn workspace_text_preserves_large_utf8_and_rejects_invalid_inputs() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("metadata.json");
    let text = format!("{{\"note\":\"{}中文😀\"}}", "x".repeat(706_000));
    std::fs::write(&path, &text).unwrap();
    assert_eq!(read_text(directory.path(), "metadata.json").unwrap(), text);
    for (bytes, expected) in [
        (vec![0xff], ErrorCode::InvalidRequest),
        (vec![b'a'; 1024 * 1024 + 1], ErrorCode::LimitExceeded),
        (vec![0; 1024 * 1024], ErrorCode::LimitExceeded),
    ] {
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(
            read_text(directory.path(), "metadata.json")
                .unwrap_err()
                .code,
            expected
        );
    }
    assert_eq!(
        read_text(directory.path(), "missing").unwrap_err().code,
        ErrorCode::NotFound
    );
    assert_eq!(
        read_text(directory.path(), "../escape").unwrap_err().code,
        ErrorCode::InvalidPath
    );
}

/// The real guest instance's correlated transport preserves version, grant and handle boundaries.
#[test]
#[ignore = "set NANOBUG_VUE_PACKAGE to the independently built Vue package"]
fn workspace_text_transport_preserves_authority() {
    let package = crate::Package::read(&std::path::PathBuf::from(
        std::env::var_os("NANOBUG_VUE_PACKAGE").expect("built Vue package"),
    ))
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    std::fs::write(root.join("metadata.json"), "{\"label\":\"中文😀\"}").unwrap();
    let mut instance = Instance::prepare(
        &Instance::engine().unwrap(),
        package.component().unwrap(),
        &package.manifest,
        &package.manifest.permissions,
        Environment {
            workspace: root.display().to_string(),
            ..Default::default()
        },
        root.join("data"),
        root.clone(),
        None,
    )
    .unwrap();
    instance.activate().unwrap();
    let state = instance.store.data_mut();
    let api::Value::Resource(handle) = state
        .resource_request(api::Operation::OpenWorkspace)
        .unwrap()
    else {
        panic!("workspace handle")
    };
    let request = serde_json::json!({"id":17,"operation":{
        "method":"read_workspace_text","handle":handle,"path":"metadata.json"
    }})
    .to_string();
    let response = |state: &mut State| -> api::Response {
        serde_json::from_str(&state.capability_request(&request).unwrap()).unwrap()
    };
    let read = response(state);
    assert_eq!(read.id, 17);
    assert!(
        matches!(read.result.unwrap(), api::Value::Text(text) if text == "{\"label\":\"中文😀\"}")
    );
    state
        .api
        .capabilities
        .insert("workspace.files".into(), semver::Version::new(1, 4, 0));
    assert_eq!(
        response(state).result.unwrap_err().code,
        ErrorCode::CapabilityUnavailable
    );
    state
        .api
        .capabilities
        .insert("workspace.files".into(), semver::Version::new(1, 5, 0));
    state.permissions.remove("workspace.read");
    assert_eq!(
        response(state).result.unwrap_err().code,
        ErrorCode::PermissionDenied
    );
    state.permissions.insert("workspace.read".into());
    state
        .resource_request(api::Operation::CloseResource { handle })
        .unwrap();
    assert_eq!(
        response(state).result.unwrap_err().code,
        ErrorCode::InvalidHandle
    );
}
