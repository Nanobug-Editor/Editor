//! Actual SDK consumers observe approved tools through the public package/manager preparation path.
#[path = "support/demand.rs"]
mod demand_support;
use plugin_runtime::{DemandReason, LanguageRole, Manager, Package, plugin_protocol::Environment};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Observing metadata never executes a declaration's failing version-probe arguments.
#[test]
#[ignore = "Build fixtures/tool-metadata with --plugin-cargo and set NANOBUG_TOOL_METADATA_COMPONENT"]
fn declared_tool_metadata_uses_owned_bounded_files_without_running_a_probe() {
    let root = tempfile::tempdir().unwrap();
    let tools = root.path().join("tool");
    std::fs::create_dir_all(tools.join("bin")).unwrap();
    let program = tools
        .join("bin")
        .join(format!("metadata-tool{}", std::env::consts::EXE_SUFFIX));
    std::fs::copy(std::env::current_exe().unwrap(), &program).unwrap();
    std::fs::write(tools.join("metadata.txt"), "VERSION=7.3\n").unwrap();
    let package = fixture(&program, json!({}));
    let mut manager = Manager::open(
        root.path().join("profile"),
        Environment {
            workspace: root.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    demand_support::ready(
        &mut manager,
        "metadata-reader",
        DemandReason::Language {
            provider: "metadata-reader/analysis".into(),
            role: LanguageRole::Lsp,
        },
    );
    let service = manager
        .language_services()
        .remove("metadata-reader/analysis")
        .unwrap()
        .unwrap();
    assert_eq!(
        service.provider.initialization_options["files"]["metadata.txt"],
        "VERSION=7.3\n"
    );
    assert_eq!(
        std::path::Path::new(
            service.provider.initialization_options["root"]
                .as_str()
                .unwrap()
        ),
        tools.canonicalize().unwrap()
    );
    assert_eq!(manager.resource_count(), 0);
    assert_eq!(service.process_count(), 0);
}

/// A readonly hook cannot turn tool metadata into traversal or an unbounded file read.
#[test]
#[ignore = "Build fixtures/tool-metadata with --plugin-cargo and set NANOBUG_TOOL_METADATA_COMPONENT"]
fn tool_metadata_rejects_escaping_paths_and_oversized_contents() {
    use plugin_runtime::plugin_protocol::api::ErrorCode;
    for (relative, contents, expected) in [
        (
            "../outside.txt",
            "private".to_owned(),
            ErrorCode::InvalidPath,
        ),
        (
            "metadata.txt",
            "x".repeat(16 * 1024 + 1),
            ErrorCode::LimitExceeded,
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        let tools = root.path().join("tool");
        std::fs::create_dir_all(tools.join("bin")).unwrap();
        let program = tools
            .join("bin")
            .join(format!("metadata-tool{}", std::env::consts::EXE_SUFFIX));
        std::fs::copy(std::env::current_exe().unwrap(), &program).unwrap();
        std::fs::write(tools.join("metadata.txt"), contents).unwrap();
        std::fs::write(root.path().join("outside.txt"), "must not be observed").unwrap();
        let package = fixture(&program, json!({"files":[relative],"capture_failure":true}));
        let mut manager = Manager::open(
            root.path().join("profile"),
            Environment {
                workspace: root.path().display().to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        demand_support::ready(
            &mut manager,
            "metadata-reader",
            DemandReason::Language {
                provider: "metadata-reader/analysis".into(),
                role: LanguageRole::Lsp,
            },
        );
        let service = manager
            .language_services()
            .remove("metadata-reader/analysis")
            .unwrap()
            .unwrap();
        assert_eq!(
            service.provider.initialization_options["failure"]["code"],
            serde_json::to_value(expected).unwrap(),
            "{relative}"
        );
        assert_eq!(service.process_count(), 0);
        assert_eq!(manager.resource_count(), 0);
    }
}

/// Assemble a real independent guest with caller-owned native declarations and a fixed inspection policy.
fn fixture(program: &std::path::Path, policy: Value) -> Package {
    let component = std::fs::read(
        std::env::var_os("NANOBUG_TOOL_METADATA_COMPONENT")
            .expect("NANOBUG_TOOL_METADATA_COMPONENT"),
    )
    .unwrap();
    let manifest = json!({
        "id":"metadata-reader", "name":"Metadata reader", "version":"0.1.0", "protocol":7,
        "component":"fixture.wasm", "contributions":"plugin.toml", "storage_limit":1024,
        "permissions":["process.exec", "process.service.analysis", "process.service.inspected"],
        "api":{"base":"^1","required":{"language.lsp":">=1.6,<2","process":"^1","tools.inspect":"^1","configuration":"^1"}},
        "settings":{"fixture.inspection":{"title":"Inspection policy","value_type":{"kind":"string","max_length":4096},"default":policy.to_string()}},
        "services":{
            "analysis":{"program":program,"args":[]},
            "inspected":{"program":program,"check_args":["--not-a-valid-test-harness-option"]}
        },
        "language_servers":[{"id":"analysis","language":"metadata-fixture","service":"analysis","hook":true}]
    });
    Package::from_files(BTreeMap::from([
        ("manifest.json".into(), serde_json::to_vec(&manifest).unwrap()),
        ("fixture.wasm".into(), component),
        ("plugin.toml".into(), b"[plugin]\nid=\"metadata-reader\"\nname=\"Metadata reader\"\nversion=\"0.1.0\"\nhost_version=\"^0.1\"\n".to_vec()),
    ])).unwrap()
}
