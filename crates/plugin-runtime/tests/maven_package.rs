//! A real independent Maven package is tested through public installation and native command requests.
#[path = "support/demand.rs"]
mod demand_support;
use plugin_runtime::{
    Manager, Package,
    plugin_protocol::{Environment, api::RequestUpdate, settings::Scope},
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
#[path = "maven_package/application.rs"]
mod application;
#[path = "maven_package/configurations.rs"]
mod configurations;
#[path = "maven_package/failures.rs"]
mod failures;
#[path = "maven_package/java.rs"]
mod java;
#[path = "maven_package/lsp.rs"]
mod lsp;
#[path = "maven_package/panel.rs"]
mod panel;
#[path = "maven_package/repository.rs"]
mod repository;

/// Effective Maven source/output/JDK facts are returned without compiling the user's project.
#[test]
#[ignore = "Build Maven with --plugin-package and set NANOBUG_MAVEN_PACKAGE to its ZIP"]
fn actual_maven_package_imports_a_project_without_compiling_it() {
    let package = Package::read(std::path::Path::new(
        &std::env::var_os("NANOBUG_MAVEN_PACKAGE").unwrap(),
    ))
    .unwrap();
    let workspace = tempfile::Builder::new()
        .prefix("nanobug-maven-中文 ")
        .tempdir()
        .unwrap();
    let profile = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(workspace.path().join("src/main/java")).unwrap();
    std::fs::write(workspace.path().join("pom.xml"), r#"<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><groupId>sample</groupId><artifactId>demo</artifactId><version>1</version><properties><maven.compiler.release>11</maven.compiler.release></properties></project>"#).unwrap();
    std::fs::write(workspace.path().join("src/main/java/Main.java"), "public class Main { public static void main(String[] args) { throw new AssertionError(\"Import must not launch\"); } }").unwrap();
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
        plugin_runtime::DemandReason::Command("import".into()),
    );
    let pending = manager
        .invoke_typed_command(
            "maven",
            "import",
            json!({"pom":"pom.xml","profiles":[],"offline":true}),
            180_000,
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(180);
    let result = loop {
        manager.poll();
        match pending.status() {
            RequestUpdate::Completed { result } => break result,
            RequestUpdate::Cancelled { reason, .. } => panic!("Maven import cancelled: {reason:?}"),
            _ => {}
        }
        assert!(Instant::now() < deadline, "Maven import deadline");
        std::thread::sleep(Duration::from_millis(15));
    }
    .expect("The actual Maven package must import an effective project model");
    let report: Value = serde_json::from_str(result["payload"].as_str().unwrap()).unwrap();
    assert_eq!(report["modules"][0]["id"], "sample:demo:1");
    assert_eq!(
        report["modules"][0]["source_roots"],
        json!(["src/main/java"])
    );
    assert_eq!(report["modules"][0]["output"], "target/classes");
    assert_eq!(report["modules"][0]["jdk"], "11");
    assert!(
        !workspace.path().join("target/classes/Main.class").exists(),
        "Import cannot compile the application"
    );
    let pom = workspace.path().join("pom.xml");
    let configured=std::fs::read_to_string(&pom).unwrap().replace("</project>","<build><plugins><plugin><artifactId>maven-compiler-plugin</artifactId><configuration><release>17</release></configuration></plugin></plugins></build></project>");
    std::fs::write(pom, configured).unwrap();
    let configured = maven_command(
        &mut manager,
        "import",
        json!({"pom":"pom.xml","profiles":[],"offline":true}),
    )
    .unwrap();
    let configured: Value = serde_json::from_str(configured["payload"].as_str().unwrap()).unwrap();
    assert_eq!(
        configured["modules"][0]["jdk"], "17",
        "Effective plugin configuration overrides inherited compiler properties"
    );
    assert!(!workspace.path().join("target/classes/Main.class").exists());
    manager.shutdown_checked().unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// One real reactor uses inherited/profile model facts and Maven's own offline dependency resolver.
#[test]
#[ignore = "Build Maven with --plugin-package; set NANOBUG_MAVEN_PACKAGE; requires javac and jar"]
fn actual_maven_package_imports_reactor_profiles_and_transitive_dependencies() {
    let package = Package::read(std::path::Path::new(
        &std::env::var_os("NANOBUG_MAVEN_PACKAGE").unwrap(),
    ))
    .unwrap();
    let workspace = tempfile::Builder::new()
        .prefix("nanobug-maven-reactor-中文 ")
        .tempdir()
        .unwrap();
    let profile = tempfile::tempdir().unwrap();
    let repo = profile.path().join("m2");
    repository::seed(&repo);
    std::fs::write(workspace.path().join("pom.xml"),r#"<project xmlns="http://maven.apache.org/POM/4.0.0">
      <modelVersion>4.0.0</modelVersion><groupId>sample</groupId><artifactId>reactor</artifactId><version>1</version><packaging>pom</packaging>
      <properties><target.java>8</target.java><maven.compiler.release>${target.java}</maven.compiler.release></properties>
      <dependencyManagement><dependencies><dependency><groupId>fixture</groupId><artifactId>bom</artifactId><version>1</version><type>pom</type><scope>import</scope></dependency></dependencies></dependencyManagement>
      <build><sourceDirectory>${project.basedir}/sources/java</sourceDirectory></build>
      <profiles><profile><id>extra</id><properties><target.java>11</target.java></properties><modules><module>lib</module><module>app</module></modules></profile></profiles>
    </project>"#).unwrap();
    for (module, dependency, source) in [
        (
            "lib",
            "<dependency><groupId>fixture</groupId><artifactId>api</artifactId></dependency>",
            "package lib; public class Library { public static int answer() { return fixture.Api.answer(); } }",
        ),
        (
            "app",
            "<dependency><groupId>sample</groupId><artifactId>lib</artifactId><version>1</version></dependency>",
            "package demo; public class Main { public static void main(String[] args) { System.out.println(lib.Library.answer()); } }",
        ),
    ] {
        let root = workspace.path().join(module);
        std::fs::create_dir_all(root.join("sources/java")).unwrap();
        std::fs::write(root.join("pom.xml"),format!(r#"<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><parent><groupId>sample</groupId><artifactId>reactor</artifactId><version>1</version></parent><artifactId>{module}</artifactId><dependencies>{dependency}</dependencies></project>"#)).unwrap();
        std::fs::write(
            root.join("sources/java").join(if module == "lib" {
                "Library.java"
            } else {
                "Main.java"
            }),
            source,
        )
        .unwrap();
    }
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
    manager
        .update_setting(
            "maven",
            Scope::Project,
            "maven.repository",
            Some(json!(repo.display().to_string())),
        )
        .unwrap();
    demand_support::ready(
        &mut manager,
        "maven",
        plugin_runtime::DemandReason::Command("import".into()),
    );
    let pending = manager
        .invoke_typed_command(
            "maven",
            "import",
            json!({"pom":"pom.xml","profiles":["extra"],"offline":true}),
            180_000,
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(180);
    let result = loop {
        manager.poll();
        match pending.status() {
            RequestUpdate::Completed { result } => break result.unwrap(),
            RequestUpdate::Cancelled { reason, .. } => {
                panic!("Reactor import cancelled: {reason:?}")
            }
            _ => {}
        }
        assert!(Instant::now() < deadline, "Reactor import deadline");
        std::thread::sleep(Duration::from_millis(15));
    };
    let report: Value = serde_json::from_str(result["payload"].as_str().unwrap()).unwrap();
    let modules = report["modules"].as_array().unwrap();
    assert_eq!(
        modules.len(),
        3,
        "The active profile contributes both reactor modules"
    );
    let app = modules
        .iter()
        .find(|entry| entry["id"] == "sample:app:1")
        .unwrap();
    assert_eq!(app["source_roots"], json!(["app/sources/java"]));
    assert_eq!(
        app["jdk"], "11",
        "Effective inherited/profile properties must be evaluated"
    );
    assert_eq!(app["test_source_roots"], json!(["app/src/test/java"]));
    assert_eq!(app["test_output"], "app/target/test-classes");
    let dependencies = app["dependencies"]
        .as_array()
        .expect("The effective module includes resolved dependencies");
    assert!(
        dependencies
            .iter()
            .any(|entry| entry["module"] == "sample:lib:1"),
        "Reactor edges must retain module identity: {app}"
    );
    for (artifact, scope) in [("api", "compile"), ("leaf", "runtime")] {
        let id = format!("fixture:{artifact}:1");
        let dependency = dependencies.iter().find(|entry| entry["id"] == id).unwrap();
        assert_eq!(dependency["scope"], scope);
        let resolved = std::path::Path::new(dependency["file"].as_str().unwrap())
            .canonicalize()
            .unwrap();
        assert_eq!(
            resolved,
            repo.join(format!("fixture/{artifact}/1/{artifact}-1.jar"))
                .canonicalize()
                .unwrap()
        );
    }
    assert!(
        !workspace
            .path()
            .join("app/target/classes/demo/Main.class")
            .exists()
    );
    assert!(
        !workspace
            .path()
            .join("lib/target/classes/lib/Library.class")
            .exists()
    );
    manager.shutdown_checked().unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// A failed refresh keeps the last successful logical model and makes its stale status observable.
#[test]
#[ignore = "Build Maven with --plugin-package and set NANOBUG_MAVEN_PACKAGE"]
fn actual_maven_package_retains_a_stale_model_after_failed_refresh() {
    let package = Package::read(std::path::Path::new(
        &std::env::var_os("NANOBUG_MAVEN_PACKAGE").unwrap(),
    ))
    .unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let pom = workspace.path().join("pom.xml");
    std::fs::write(&pom,r#"<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><groupId>sample</groupId><artifactId>retained</artifactId><version>1</version></project>"#).unwrap();
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
        plugin_runtime::DemandReason::Command("import".into()),
    );
    let arguments = json!({"pom":"pom.xml","profiles":[],"offline":true});
    let imported = maven_command(&mut manager, "import", arguments.clone()).unwrap();
    let original: Value = serde_json::from_str(imported["payload"].as_str().unwrap()).unwrap();
    std::fs::write(&pom, "<project><broken").unwrap();
    let failed = maven_command(&mut manager, "import", arguments);
    assert!(failed.is_err(), "A broken POM must not publish a new model");
    let status = maven_command(&mut manager, "model", Value::Null)
        .expect("The public command must expose the retained model's status");
    let status: Value = serde_json::from_str(status["payload"].as_str().unwrap()).unwrap();
    assert_eq!(status["model"]["generation"], original["generation"]);
    assert_eq!(status["model"]["modules"][0]["id"], "sample:retained:1");
    assert_eq!(status["stale"], true);
    assert_eq!(status["importing"], false);
    manager.shutdown_checked().unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// Drive the same bounded public command receipt as native command consumers, including typed errors.
fn maven_command(
    manager: &mut Manager,
    command: &str,
    arguments: Value,
) -> Result<Value, plugin_runtime::plugin_protocol::api::Failure> {
    let pending = manager.invoke_typed_command("maven", command, arguments, 180_000)?;
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        manager.poll();
        match pending.status() {
            RequestUpdate::Completed { result } => return result,
            RequestUpdate::Cancelled { reason, .. } => {
                return Err(plugin_runtime::plugin_protocol::api::Failure::new(
                    reason,
                    "Maven command cancelled",
                ));
            }
            _ => {}
        }
        assert!(Instant::now() < deadline, "Maven command deadline");
        std::thread::sleep(Duration::from_millis(15));
    }
}
