//! Real package grants and retirement govern native file proposals through the public Manager.
use super::*;
use plugin_runtime::plugin_protocol::api::ErrorCode;

/// File proposals cannot borrow a missing grant, escape their workspace or survive owner retirement.
#[test]
#[ignore = "Build Java with --plugin-package and set NANOBUG_JAVA_PACKAGE to the resulting ZIP"]
fn java_package_file_proposals_preserve_grants_paths_and_retirement() {
    let archive = std::path::PathBuf::from(std::env::var_os("NANOBUG_JAVA_PACKAGE").unwrap());
    let original = Package::read(&archive).unwrap();
    for granted in [false, true] {
        let mut package_files = original.files.clone();
        if !granted {
            let mut manifest: Value =
                serde_json::from_slice(&package_files["manifest.json"]).unwrap();
            manifest["permissions"]
                .as_array_mut()
                .unwrap()
                .retain(|permission| permission != "workspace.write");
            package_files.insert(
                "manifest.json".into(),
                serde_json::to_vec(&manifest).unwrap(),
            );
        }
        let package = Package::from_files(package_files).unwrap();
        let workspace = tempfile::tempdir().unwrap();
        let profile = tempfile::tempdir().unwrap();
        std::fs::write(workspace.path().join("Main.java"), "public class Main {}\n").unwrap();
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
        demand_support::ready(
            &mut manager,
            "java",
            DemandReason::Language {
                provider: "java/analysis".into(),
                role: LanguageRole::Lsp,
            },
        );
        let service = manager
            .language_services()
            .remove("java/analysis")
            .unwrap()
            .unwrap();
        let proposal = service.refactor_file("Main.java");
        if !granted {
            assert_eq!(proposal.err().unwrap().code, ErrorCode::PermissionDenied);
        } else {
            let file = proposal.unwrap();
            let (metadata, _) = plugin_runtime::workspace_io::read(&file, false).unwrap();
            assert!(matches!(
                metadata.stamp,
                plugin_runtime::plugin_protocol::workspace::FileStamp::File { .. }
            ));
            assert_eq!(metadata.target.path, "Main.java");
            assert_eq!(
                service.refactor_file("../outside.java").err().unwrap().code,
                ErrorCode::InvalidPath
            );
            manager.disable("java").unwrap();
            assert!(!file.is_live());
            assert_eq!(file.guard().err().unwrap().code, ErrorCode::InvalidHandle);
            assert_eq!(
                service.refactor_file("Main.java").err().unwrap().code,
                ErrorCode::InvalidHandle
            );
        }
        manager.shutdown_checked().unwrap();
        assert_eq!(
            std::fs::read_to_string(workspace.path().join("Main.java")).unwrap(),
            "public class Main {}\n"
        );
    }
}
