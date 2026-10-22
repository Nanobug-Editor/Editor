//! Architectural regression: production and test feature resolution must keep styled runtimes absent.

/// Cargo's selected tree must retain Base and Kit without the styled runtime, including tests.
#[test]
fn local_ui_dependency_graph_excludes_component_runtime() {
    let output = std::process::Command::new(env!("CARGO"))
        .args([
            "tree",
            "--offline",
            "--locked",
            "-p",
            "editor-app",
            "--all-features",
            "--target",
            "all",
            "--edges",
            "normal,build,dev",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let graph = String::from_utf8(output.stdout).unwrap();
    assert!(
        graph.lines().any(|line| line.starts_with("gpui-base ")),
        "the selected graph must include Base"
    );
    for name in graph
        .lines()
        .filter_map(|line| line.split_whitespace().next())
    {
        assert!(
            !matches!(name, "gpui-component" | "gpui-component-macros"),
            "UI runtime boundary violated by {name}; keep Kit defaults and component features disabled"
        );
    }
}

/// Reject weak optional component entries even when Cargo would omit them from compilation.
#[test]
fn local_ui_lockfile_excludes_component_packages() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock");
    let lockfile = std::fs::read_to_string(path).expect("read the workspace lockfile");
    let lockfile: toml::Value = toml::from_str(&lockfile).expect("parse the workspace lockfile");
    for package in lockfile["package"].as_array().expect("locked packages") {
        let name = package["name"].as_str().expect("locked package name");
        assert!(
            !matches!(name, "gpui-component" | "gpui-component-macros"),
            "lockfile retains {name}; enable test support on the GPUI foundations, not Kit"
        );
    }
}

/// Every shared module has a co-located entry; the host cannot grow a second control directory.
#[test]
fn local_ui_modules_use_one_colocated_entry() {
    let app = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources = app.join("../editor-ui/src");
    for entry in std::fs::read_dir(sources).expect("shared UI source directory") {
        let path = entry.expect("UI source entry").path();
        if path.is_dir() {
            assert!(
                path.join("mod.rs").is_file(),
                "missing entry in {}",
                path.display()
            );
        } else if path.extension().is_some_and(|value| value == "rs") {
            assert_eq!(
                path.file_name().unwrap(),
                "lib.rs",
                "component entries belong in their own directory"
            );
        }
    }
    assert!(
        !app.join("src/ui/controls.rs").exists(),
        "host controls must use editor-ui"
    );
}
