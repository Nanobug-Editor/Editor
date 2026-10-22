//! Java configurations enter the ordinary provider catalog and native form admission while cold.
use super::*;
use plugin_runtime::plugin_protocol::{api::RequestUpdate, configurations as contract, settings};

/// A plain Java project supplies a native application form without requiring an opened source.
#[test]
#[ignore = "Build Java with --plugin-package and set NANOBUG_JAVA_PACKAGE to the resulting ZIP"]
fn java_package_offers_an_application_configuration_without_an_open_document() {
    let archive = std::path::PathBuf::from(std::env::var_os("NANOBUG_JAVA_PACKAGE").unwrap());
    let package = Package::read(&archive).unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    std::fs::write(
        workspace.path().join("Main.java"),
        "public class Main { public static void main(String[] args) {} }\n",
    )
    .unwrap();
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
            "java",
            settings::Scope::Project,
            "java.project",
            Some(json!(
                "{\"source_roots\":[\".\"],\"jars\":[],\"output\":\"out\",\"main_class\":\"Main\"}"
            )),
        )
        .unwrap();
    assert!(
        manager
            .configuration_providers()
            .iter()
            .any(|provider| provider == "java"),
        "The installed Java package must contribute to the public native configuration catalog"
    );
    let root = manager.workspace().to_owned();
    let catalog: contract::Catalog = contract::decode(reply(
        &mut manager,
        "catalog",
        json!({
            "workspace":root,"locale":"zh-CN","os":"windows"
        }),
    ))
    .unwrap();
    let template = catalog
        .templates
        .iter()
        .find(|item| item.id == "application")
        .unwrap();
    assert_eq!(template.group, "Java");
    assert!(template.unavailable.is_none());
    let mut form: contract::Form = contract::decode(reply(
        &mut manager,
        "form",
        json!({
            "workspace":root,"locale":"zh-CN","os":"windows", "template":template.id,
            "values":template.defaults,"event":""
        }),
    ))
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&form.values).unwrap()["main_class"],
        "Main"
    );
    manager
        .validate_native_document("java", &form.document)
        .unwrap();
    assert!(!form.name.trim().is_empty());
    for (node, action) in [
        (
            "java-main-class",
            plugin_runtime::plugin_protocol::ui::Action::Change("demo.入口".into()),
        ),
        (
            "arg-add",
            plugin_runtime::plugin_protocol::ui::Action::Click,
        ),
        (
            "arg-0",
            plugin_runtime::plugin_protocol::ui::Action::Change(
                "two words & 中文 ; \"quotes\"".into(),
            ),
        ),
    ] {
        let event = plugin_runtime::plugin_protocol::ui::UiEvent {
            revision: form.document.revision,
            node: node.into(),
            action,
        };
        form = contract::decode(reply(
            &mut manager,
            "form",
            json!({
                "workspace":root,"locale":"zh-CN","os":"windows", "template":template.id,
                "values":form.values,"event":serde_json::to_string(&event).unwrap()
            }),
        ))
        .unwrap();
        manager
            .validate_native_document("java", &form.document)
            .unwrap();
    }
    let validated: contract::Validation = contract::decode(reply(
        &mut manager,
        "validate",
        json!({
            "workspace":root,"locale":"zh-CN","os":"windows", "template":template.id,
            "values":form.values,"intent":"run"
        }),
    ))
    .unwrap();
    assert!(validated.valid, "{}", validated.message);
    let launch = validated.launch.unwrap();
    assert_eq!(launch.target["mode"], "provided");
    assert_eq!(launch.target["provider"], "java");
    assert_eq!(
        launch.target["args"],
        json!(["two words & 中文 ; \"quotes\""])
    );
    let binding: Value = serde_json::from_str(launch.target["binding"].as_str().unwrap()).unwrap();
    assert_eq!(binding["schema"], 1);
    assert_eq!(binding["main_class"], "demo.入口");
    assert_eq!(
        launch.build.len(),
        1,
        "An application compiles exactly once before running"
    );
    assert_eq!(launch.build[0]["target"]["kind"], "action");
    let compile = &launch.build[0]["target"]["target"];
    assert_eq!(compile["mode"], "provided");
    assert_eq!(compile["provider"], "java");
    assert_eq!(compile["args"], json!([]));
    let compile_binding: Value =
        serde_json::from_str(compile["binding"].as_str().unwrap()).unwrap();
    assert_eq!(
        compile_binding, binding,
        "The completed build resolves this exact main target"
    );
    manager.shutdown_checked().unwrap();
}

/// Poll the public request's real activation, original provider and result; no synthetic guest is used.
fn reply(manager: &mut Manager, method: &str, arguments: Value) -> Value {
    let pending = manager
        .begin_configuration_call("java", method, arguments)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        manager.poll();
        match pending.status() {
            RequestUpdate::Completed { result } => return result.unwrap(),
            RequestUpdate::Cancelled { reason, .. } => {
                panic!(
                    "Java configuration {method} ended: {reason:?}; {:?}",
                    pending.snapshot()
                )
            }
            _ => {}
        }
        assert!(
            Instant::now() < deadline,
            "Java native configuration did not complete"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
