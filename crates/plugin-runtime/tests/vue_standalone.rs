//! Actual independent packages admit language demands without another language plugin.
use plugin_runtime::{
    DemandReason, LanguageRole, Manager, Package,
    plugin_protocol::{Environment, api, language},
};
use std::time::{Duration, Instant};

/// Admission must reach Vue's instance; native completion is exercised in the editor's worker test.
#[test]
#[ignore = "set NANOBUG_VUE_PACKAGE to the built independent Vue archive"]
fn vue_language_admits_without_typescript_plugin() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    std::fs::write(
        workspace.path().join("package.json"),
        r#"{"dependencies":{"vue":"2.6.14"}}"#,
    )
    .unwrap();
    std::fs::write(workspace.path().join("App.vue"), "<template><p>{{ title }}</p></template><script>export default { data() { return { title: 'hello' }; } };</script>").unwrap();
    let archive = std::path::PathBuf::from(std::env::var_os("NANOBUG_VUE_PACKAGE").unwrap());
    let package = Package::read(&archive).unwrap();
    let mut manager = Manager::open(
        profile.path().to_owned(),
        Environment {
            workspace: workspace
                .path()
                .canonicalize()
                .unwrap()
                .display()
                .to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let availability = manager.availability("vue").unwrap();
    let request = manager
        .begin_demand(
            &availability,
            DemandReason::LanguageDocument {
                provider: "vue/vue".into(),
                role: LanguageRole::Lsp,
                document: language::DocumentContext {
                    path: "App.vue".into(),
                    generation: 1,
                },
            },
            120_000,
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    while manager.instance_id("vue").is_none() {
        manager.poll();
        match request.status() {
            api::RequestUpdate::Completed { result: Err(error) } => {
                panic!("Vue must admit without TypeScript: {error:?}")
            }
            api::RequestUpdate::Cancelled { reason, .. } => {
                panic!("Vue demand was cancelled: {reason:?}")
            }
            _ => {}
        }
        assert!(Instant::now() < deadline, "Vue admission timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
    // This harness has no editor document/control executor. Do not misreport admission as LSP readiness.
    request.cancel().unwrap();
    manager.shutdown();
}
