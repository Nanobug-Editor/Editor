//! A temporary real Maven repository keeps dependency resolution offline and separate from user data.
use std::{path::Path, process::Command};

/// Seed actual Java JARs plus a BOM and a runtime transitive edge; none is a reactor build output.
pub(super) fn seed(repository: &Path) {
    artifact(
        repository,
        "leaf",
        "",
        "public class Leaf { public static int value() { return 7; } }",
    );
    artifact(
        repository,
        "api",
        r#"<dependencies><dependency><groupId>fixture</groupId><artifactId>leaf</artifactId><version>1</version><scope>runtime</scope></dependency></dependencies>"#,
        "public class Api { public static int answer() { return 42; } }",
    );
    let directory = repository.join("fixture/bom/1");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("bom-1.pom"),r#"<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><groupId>fixture</groupId><artifactId>bom</artifactId><version>1</version><packaging>pom</packaging><dependencyManagement><dependencies><dependency><groupId>fixture</groupId><artifactId>api</artifactId><version>1</version></dependency></dependencies></dependencyManagement></project>"#).unwrap();
}
/// Compiler and archive commands use literal arguments; the class payload is independent fixture evidence.
fn artifact(repository: &Path, name: &str, dependencies: &str, source: &str) {
    let directory = repository.join(format!("fixture/{name}/1"));
    let build = repository.join(format!("fixture-build/{name}"));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::create_dir_all(&build).unwrap();
    let class = if name == "api" { "Api" } else { "Leaf" };
    let file = build.join(format!("{class}.java"));
    std::fs::write(&file, format!("package fixture; {source}")).unwrap();
    let compile = Command::new("javac")
        .args(["--release", "8", "-Xlint:-options", "-d"])
        .arg(build.join("classes"))
        .arg(&file)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let archive = Command::new("jar")
        .args(["--create", "--file"])
        .arg(directory.join(format!("{name}-1.jar")))
        .arg("-C")
        .arg(build.join("classes"))
        .arg(".")
        .output()
        .unwrap();
    assert!(
        archive.status.success(),
        "{}",
        String::from_utf8_lossy(&archive.stderr)
    );
    std::fs::write(directory.join(format!("{name}-1.pom")),format!(r#"<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><groupId>fixture</groupId><artifactId>{name}</artifactId><version>1</version>{dependencies}</project>"#)).unwrap();
}
