//! Users import an actual project from a native Maven panel, without invoking a hidden typed command.
use super::*;
use plugin_runtime::{
    DemandReason,
    plugin_protocol::{api, ui},
};

/// The published native button updates the same imported model later consumed by language providers.
#[test]
#[ignore = "Build Maven with --plugin-package and set NANOBUG_MAVEN_PACKAGE"]
fn maven_native_project_panel_imports_into_the_public_model() {
    let package = Package::read(std::path::Path::new(
        &std::env::var_os("NANOBUG_MAVEN_PACKAGE").unwrap(),
    ))
    .unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("pom.xml"),r#"<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><groupId>sample</groupId><artifactId>panel</artifactId><version>1</version></project>"#).unwrap();
    std::fs::create_dir_all(workspace.path().join("独立 工程")).unwrap();
    std::fs::create_dir_all(workspace.path().join("独立 工程/.mvn")).unwrap();
    std::fs::write(
        workspace.path().join("独立 工程/.mvn/maven.config"),
        "-Dfixture.release=17\n",
    )
    .unwrap();
    std::fs::write(workspace.path().join("独立 工程/pom.xml"),r#"<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><groupId>sample</groupId><artifactId>selected</artifactId><version>1</version><profiles><profile><id>dev</id><properties><maven.compiler.release>${fixture.release}</maven.compiler.release></properties></profile></profiles></project>"#).unwrap();
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
    demand_support::try_ready(&mut manager, "maven", DemandReason::View("project".into()))
        .expect("The Maven project view must be available from native window controls");
    let document = manager.live["maven"].views["project"].clone();
    manager
        .validate_native_document("maven", &document)
        .unwrap();
    manager
        .event(
            "maven",
            Some("project".into()),
            api::Notification::Ui(ui::UiEvent {
                revision: document.revision,
                node: "maven-find".into(),
                action: ui::Action::Click,
            }),
        )
        .unwrap();
    let document = manager.live["maven"].views["project"].clone();
    let ui::Kind::Column { children } = &document.root.kind else {
        panic!("The native project controls must be published");
    };
    let choice = children
        .iter()
        .find(|node| matches!(&node.kind,ui::Kind::Button {label} if label=="独立 工程/pom.xml"))
        .expect("The real workspace discovery must expose the independent Maven root");
    manager
        .event(
            "maven",
            Some("project".into()),
            api::Notification::Ui(ui::UiEvent {
                revision: document.revision,
                node: choice.id.clone(),
                action: ui::Action::Click,
            }),
        )
        .unwrap();
    for (node, action) in [
        ("maven-profiles", ui::Action::Change("dev".into())),
        ("maven-offline", ui::Action::Toggle(true)),
    ] {
        let revision = manager.live["maven"].views["project"].revision;
        manager
            .event(
                "maven",
                Some("project".into()),
                api::Notification::Ui(ui::UiEvent {
                    revision,
                    node: node.into(),
                    action,
                }),
            )
            .unwrap();
    }
    let document = manager.live["maven"].views["project"].clone();
    manager
        .event(
            "maven",
            Some("project".into()),
            api::Notification::Ui(ui::UiEvent {
                revision: document.revision,
                node: "maven-import".into(),
                action: ui::Action::Click,
            }),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    let status = loop {
        manager.poll();
        let status = maven_command(&mut manager, "model", Value::Null).unwrap();
        let status: Value = serde_json::from_str(status["payload"].as_str().unwrap()).unwrap();
        if !status["model"].is_null() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "The native import button did not publish a model"
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(status["model"]["modules"][0]["id"], "sample:selected:1");
    assert_eq!(status["model"]["modules"][0]["jdk"], "17");
    assert_eq!(status["stale"], false);
    assert_eq!(status["importing"], false);
    // Native filesystem observations withdraw the model before another language demand can borrow it.
    manager.workspace_file_event(plugin_runtime::plugin_protocol::workspace::FileEvent {
        sequence: 1,
        kind: plugin_runtime::plugin_protocol::workspace::FileEventKind::Modified {
            path: "独立 工程/pom.xml".into(),
        },
    });
    manager.poll();
    let changed = maven_command(&mut manager, "model", Value::Null).unwrap();
    let changed: Value = serde_json::from_str(changed["payload"].as_str().unwrap()).unwrap();
    assert_eq!(
        changed["stale"], true,
        "POM observations must retire imported inputs"
    );
    assert_eq!(
        changed["model"]["generation"],
        status["model"]["generation"]
    );
    // Cancel before a process completion tick: a late successful report cannot replace the old cache.
    for node in ["maven-import", "maven-cancel"] {
        let revision = manager.live["maven"].views["project"].revision;
        manager
            .event(
                "maven",
                Some("project".into()),
                api::Notification::Ui(ui::UiEvent {
                    revision,
                    node: node.into(),
                    action: ui::Action::Click,
                }),
            )
            .unwrap();
    }
    let cancelled = maven_command(&mut manager, "model", Value::Null).unwrap();
    let cancelled: Value = serde_json::from_str(cancelled["payload"].as_str().unwrap()).unwrap();
    assert_eq!(cancelled["stale"], true);
    assert_eq!(cancelled["importing"], false);
    assert_eq!(
        cancelled["model"]["generation"],
        status["model"]["generation"]
    );
    manager.shutdown_checked().unwrap();
    assert_eq!(manager.resource_count(), 0);
}
