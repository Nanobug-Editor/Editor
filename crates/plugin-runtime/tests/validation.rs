//! Author-facing checks must diagnose production admission without running an untrusted component.
use plugin_runtime::validation::{self, Code, ToolIdentity};
use std::io::Write;

/// Build only a ZIP input: the component deliberately cannot execute if a verifier accidentally loads it.
fn archive(mutate: impl FnOnce(&mut serde_json::Value), extra: Option<&str>) -> Vec<u8> {
    let mut manifest = serde_json::json!({"id":"check","name":"Check","version":"1.0.0","protocol":7,
        "api":{"base":"^1"},"component":"component.wasm","permissions":[],"storage_limit":1024});
    mutate(&mut manifest);
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in [
        ("manifest.json", serde_json::to_vec(&manifest).unwrap()),
        (
            "component.wasm",
            b"\0asm\x0d\0\x01\0SECRET_COMPONENT_BODY".to_vec(),
        ),
    ] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&bytes).unwrap();
    }
    if let Some(name) = extra {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"SECRET_FILE_BODY").unwrap();
    }
    zip.finish().unwrap().into_inner()
}

/// Static admission cannot instantiate or infer dynamic compatibility from optional degradation.
#[test]
fn package_check_static_success_does_not_load_component_and_reports_optional_absence() {
    let bytes = archive(
        |manifest| manifest["api"]["optional"] = serde_json::json!({"future.optional":"^1"}),
        None,
    );
    let admitted = plugin_runtime::Package::from_bytes(&bytes);
    assert!(
        admitted.is_ok(),
        "production admission: {:#}",
        admitted.err().unwrap()
    );
    let report = validation::check_bytes(&bytes, ToolIdentity::default());
    assert!(report.passed);
    assert!(!report.executed);
    assert_eq!(report.diagnostics[0].code, Code::OptionalUnavailable);
    assert!(
        report
            .coverage
            .iter()
            .any(|entry| entry == "dynamic_calls_not_exercised")
    );
    assert!(
        !serde_json::to_string(&report)
            .unwrap()
            .contains("SECRET_COMPONENT_BODY")
    );
}

/// Production reject points carry stable typed codes, without leaking attacker-controlled filenames.
#[test]
fn package_check_protocol_permissions_host_floor_and_paths_have_stable_codes() {
    for (bytes, code) in [
        (
            archive(|manifest| manifest["protocol"] = 6.into(), None),
            Code::Protocol,
        ),
        (
            archive(|manifest| manifest["minimum_host"] = "99.0.0".into(), None),
            Code::HostVersion,
        ),
        (
            archive(
                |manifest| manifest["permissions"] = serde_json::json!(["forbidden.secret-token"]),
                None,
            ),
            Code::Permission,
        ),
        (archive(|_| {}, Some("../SECRET_PATH_TOKEN")), Code::Path),
    ] {
        let report = validation::check_bytes(&bytes, ToolIdentity::default());
        assert!(!report.passed);
        assert_eq!(report.diagnostics[0].code, code);
        let text = serde_json::to_string(&report).unwrap();
        assert!(!text.contains("SECRET_PATH_TOKEN"));
        assert!(!text.contains("forbidden.secret-token"));
    }
}

/// Future required capabilities produce a typed field diagnosis, without parsing an error string.
#[test]
fn package_check_reports_required_capability_without_running_wasm() {
    let manifest = serde_json::json!({"id":"future-check","name":"Check","version":"1.0.0","protocol":7,
        "api":{"base":"^1","required":{"future.capability":"^3"}},"component":"component.wasm","permissions":[],"storage_limit":1024});
    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    archive
        .start_file("manifest.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    archive
        .write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    archive
        .start_file("component.wasm", zip::write::SimpleFileOptions::default())
        .unwrap();
    archive.write_all(b"deliberately not executable").unwrap();
    let bytes = archive.finish().unwrap().into_inner();
    let report = validation::check_bytes(&bytes, ToolIdentity::default());
    assert!(!report.passed);
    assert_eq!(report.diagnostics[0].code, Code::RequiredCapability);
    assert_eq!(
        report.diagnostics[0].pointer.as_deref(),
        Some("/api/required/future.capability")
    );
    assert!(!report.executed);
    assert!(report.archive_sha256.is_some());
}

/// Package admission accepts valid long SemVer while report identity remains small and explicit.
#[test]
fn package_check_bounds_version_and_negotiation_metadata_without_rejecting_valid_packages() {
    let version = format!("1.0.0-{}", "a".repeat(1024 * 1024));
    let accepted = validation::check_bytes(
        &archive(
            |manifest| manifest["version"] = version.clone().into(),
            None,
        ),
        ToolIdentity::default(),
    );
    assert!(accepted.passed);
    assert!(accepted.metadata_partial);
    assert!(accepted.version.is_none());
    assert_eq!(accepted.version_sha256.as_deref().map(str::len), Some(64));
    assert!(serde_json::to_vec(&accepted).unwrap().len() < 8192);
    let rejected = validation::check_bytes(
        &archive(
            |manifest| {
                manifest["api"]["base"] = format!(">=99.0.0-{}", "a".repeat(1024 * 1024)).into()
            },
            None,
        ),
        ToolIdentity::default(),
    );
    assert!(!rejected.passed);
    assert!(rejected.metadata_partial);
    assert!(rejected.diagnostics[0].required.is_none());
    assert_eq!(
        rejected.diagnostics[0]
            .required_sha256
            .as_deref()
            .map(str::len),
        Some(64)
    );
    assert!(serde_json::to_vec(&rejected).unwrap().len() < 8192);
}

/// A validator without a field-specific code still retains its admission phase for development exits.
#[test]
fn unclassified_package_rejection_has_a_typed_admission_boundary_and_original_human_message() {
    let bytes = archive(|manifest| manifest["id"] = "INVALID".into(), None);
    let error = plugin_runtime::Package::from_bytes(&bytes).err().unwrap();
    let issue = error.downcast_ref::<validation::Diagnostic>().unwrap();
    assert_eq!(issue.code, Code::Unclassified);
    assert_eq!(error.to_string(), "Invalid plugin identity");
    assert_eq!(
        validation::check_bytes(&bytes, ToolIdentity::default()).diagnostics[0].code,
        Code::Unclassified
    );
    let mut files = std::collections::BTreeMap::new();
    files.insert(
        "manifest.json".into(),
        br#"{"id":"INVALID","name":"Invalid","version":"1.0.0","protocol":7,"api":{"base":"^1"}}"#
            .to_vec(),
    );
    let error = plugin_runtime::Package::from_files(files).err().unwrap();
    assert!(error.downcast_ref::<validation::Diagnostic>().is_some());
}
