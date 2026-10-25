//! Selected project preparation reports real tool errors without stranding the original native waiter.
use super::*;
use plugin_runtime::{DemandReason, LanguageRole, plugin_protocol::language::DocumentContext};

/// A valid Maven model requesting an unsupported JDK fails Java preparation as a completed error.
#[test]
#[ignore = "Build Maven and Java; set NANOBUG_MAVEN_PACKAGE and NANOBUG_JAVA_PACKAGE"]
fn an_invalid_imported_jdk_finishes_the_selected_context_with_its_actual_error() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(workspace.path().join("src/main/java")).unwrap();
    std::fs::write(
        workspace.path().join("src/main/java/Main.java"),
        "public class Main {}",
    )
    .unwrap();
    std::fs::write(workspace.path().join("pom.xml"),r#"<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><groupId>sample</groupId><artifactId>invalid-jdk</artifactId><version>1</version><properties><maven.compiler.release>26</maven.compiler.release></properties></project>"#).unwrap();
    let mut manager = Manager::open(
        profile.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    for variable in ["NANOBUG_MAVEN_PACKAGE", "NANOBUG_JAVA_PACKAGE"] {
        let package =
            Package::read(std::path::Path::new(&std::env::var_os(variable).unwrap())).unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
    }
    manager
        .update_setting(
            "maven",
            Scope::Project,
            "maven.repository",
            Some(json!(profile.path().join("m2").display().to_string())),
        )
        .unwrap();
    demand_support::ready(
        &mut manager,
        "maven",
        DemandReason::Command("import".into()),
    );
    maven_command(
        &mut manager,
        "import",
        json!({"pom":"pom.xml","profiles":[],"offline":true}),
    )
    .unwrap();
    let availability = manager.availability("maven").unwrap();
    let source = DocumentContext {
        path: "src/main/java/Main.java".into(),
        generation: availability.configuration_generation(),
    };
    let original = manager
        .begin_demand(
            &availability,
            DemandReason::LanguageDocument {
                provider: "maven/analysis".into(),
                role: LanguageRole::Lsp,
                document: source.clone(),
            },
            120_000,
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    let control = loop {
        manager.poll();
        if let Some((_, request)) = manager.take_editor_requests().pop() {
            assert!(request.begin());
            break request;
        }
        assert!(!original.status().is_terminal(), "{:?}", original.status());
        assert!(
            Instant::now() < deadline,
            "Original native preparation deadline"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    let java = manager.availability("java").unwrap();
    let source = DocumentContext {
        generation: java.configuration_generation(),
        ..source
    };
    let native = manager
        .begin_demand(
            &java,
            DemandReason::LanguageDocument {
                provider: "java/project-analysis".into(),
                role: LanguageRole::Lsp,
                document: source,
            },
            15_000,
        )
        .unwrap();
    let error = loop {
        manager.poll();
        match native.status() {
            RequestUpdate::Completed { result: Err(error) } => break error,
            RequestUpdate::Completed { result: Ok(_) } => {
                panic!("An unsupported JDK cannot yield a ready native plan")
            }
            RequestUpdate::Cancelled { reason, .. } => {
                panic!("The actual preparation error was stranded until cancellation: {reason:?}")
            }
            _ => {}
        }
        assert!(
            Instant::now() < deadline,
            "Selected native preparation deadline"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(
        error.message.contains("Unsupported project JDK"),
        "{error:?}"
    );
    control.finish(Err(error));
    loop {
        manager.poll();
        match original.status() {
            RequestUpdate::Completed { result: Err(error) } => {
                assert!(
                    error.message.contains("Unsupported project JDK"),
                    "{error:?}"
                );
                break;
            }
            RequestUpdate::Cancelled { reason, .. } => {
                panic!("Original preparation timed out instead of returning the error: {reason:?}")
            }
            _ => {}
        }
        assert!(Instant::now() < deadline, "Error forwarding deadline");
        std::thread::sleep(Duration::from_millis(10));
    }
    manager.shutdown_checked().unwrap();
}
