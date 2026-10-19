//! Architectural regression: production and test feature resolution must keep styled runtimes absent.

/// Cargo's selected tree includes test features; metadata can retain inactive weak optional nodes.
#[test]
fn local_ui_dependency_graph_excludes_component_runtime() {
    let output = std::process::Command::new(env!("CARGO"))
        .args([
            "tree",
            "--offline",
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
