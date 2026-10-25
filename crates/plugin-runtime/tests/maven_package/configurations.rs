//! Native configuration consumers execute a real Maven goal with literal arguments and console output.
use super::*;
use plugin_runtime::{
    ExecutionState, RunRequest,
    plugin_protocol::{configurations as contract, process::Update},
};

/// A saved Maven goal uses the public native form, target preparation and actual execution session.
#[test]
#[ignore = "Build Maven with --plugin-package and set NANOBUG_MAVEN_PACKAGE"]
fn maven_native_configuration_executes_a_literal_goal_in_its_own_session() {
    let package = Package::read(std::path::Path::new(
        &std::env::var_os("NANOBUG_MAVEN_PACKAGE").unwrap(),
    ))
    .unwrap();
    let workspace = tempfile::Builder::new()
        .prefix("Maven 目标 中文 ")
        .tempdir()
        .unwrap();
    let profile = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("pom.xml"),r#"<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><groupId>sample</groupId><artifactId>goals</artifactId><version>1</version><name>${fixture.label}</name><properties><fixture.label>Default label</fixture.label></properties><build><plugins><plugin><artifactId>maven-antrun-plugin</artifactId><version>3.1.0</version><configuration><target><touch file="${user.dir}/maven-native.marker"/><echo message="Maven long running goal"/><sleep seconds="120"/></target></configuration></plugin></plugins></build></project>"#).unwrap();
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
    assert!(
        manager
            .configuration_providers()
            .iter()
            .any(|provider| provider == "maven"),
        "Maven must provide the native configuration catalog"
    );
    let root = manager.workspace().to_owned();
    let catalog: contract::Catalog = contract::decode(configuration(
        &mut manager,
        "catalog",
        json!({"workspace":root,"locale":"zh-CN","os":"windows"}),
    ))
    .unwrap();
    let goal = catalog
        .templates
        .iter()
        .find(|template| template.id == "goal")
        .unwrap();
    let mut values: Value = serde_json::from_str(&goal.defaults).unwrap();
    let label = "literal two words & 中文 ; quotes";
    values["command"]["args"] = json!(["validate", format!("-Dfixture.label={label}"), "-o"]);
    let form:contract::Form=contract::decode(configuration(&mut manager,"form",json!({"workspace":root,"locale":"zh-CN","os":"windows","template":"goal","values":values.to_string(),"event":""}))).unwrap();
    manager
        .validate_native_document("maven", &form.document)
        .unwrap();
    let validation:contract::Validation=contract::decode(configuration(&mut manager,"validate",json!({"workspace":root,"locale":"zh-CN","os":"windows","template":"goal","values":form.values,"intent":"run"}))).unwrap();
    assert!(validation.valid, "{}", validation.message);
    let launch = validation.launch.unwrap();
    let pending = manager
        .begin_target_call(
            "maven",
            "prepare",
            json!({"workspace":root,"binding":launch.target["binding"],"env":[]}),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    let prepared = loop {
        manager.poll();
        if let RequestUpdate::Completed { result } = pending.status() {
            break result.unwrap();
        }
        assert!(Instant::now() < deadline, "Maven target deadline");
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut args: Vec<String> = serde_json::from_value(prepared["prefix_args"].clone()).unwrap();
    args.extend(serde_json::from_value::<Vec<String>>(launch.target["args"].clone()).unwrap());
    let execution = manager
        .start_terminal_configuration_execution_with_origins(
            "maven-goal",
            RunRequest {
                program: prepared["program"].as_str().unwrap().into(),
                args,
                cwd: Some(root),
                env: vec![],
                name: Some("Maven validate".into()),
            },
            &[pending.origin()],
        )
        .unwrap();
    let mut output = Vec::new();
    let mut exit = None;
    while exit.is_none() {
        manager.poll();
        for update in manager
            .take_native_execution_updates()
            .into_iter()
            .flat_map(|event| event.updates)
        {
            match update {
                Update::Output { bytes, .. } => {
                    output.extend(bytes);
                    if output.windows(4).any(|part| part == b"\x1b[6n") {
                        let _ = manager.input_execution(execution.id(), b"\x1b[1;1R".to_vec());
                    }
                }
                Update::Exited { code } => exit = Some(code),
                _ => {}
            }
        }
        let snapshot = execution.snapshot();
        assert_ne!(
            snapshot.state,
            ExecutionState::Failed,
            "{:?}",
            snapshot.failure
        );
        assert!(
            Instant::now() < deadline,
            "Maven execution deadline: {}",
            String::from_utf8_lossy(&output)
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = String::from_utf8_lossy(&output);
    assert_eq!(exit, Some(0), "{output}");
    assert!(
        output.contains(label),
        "Literal Maven property was lost: {output}"
    );
    assert!(output.contains("BUILD SUCCESS"), "{output}");
    // A real standard Maven goal stays alive long enough to observe Stop and a fresh second Run.
    let cwd = workspace.path().join("独立 cwd");
    std::fs::create_dir(&cwd).unwrap();
    let marker = cwd.join("maven-native.marker");
    let mut previous = None;
    for _ in 0..2 {
        let mut args: Vec<String> =
            serde_json::from_value(prepared["prefix_args"].clone()).unwrap();
        args.extend([
            "-N".into(),
            "org.apache.maven.plugins:maven-antrun-plugin:3.1.0:run".into(),
        ]);
        let session = manager
            .start_terminal_configuration_execution_with_origins(
                "maven-long-goal",
                RunRequest {
                    program: prepared["program"].as_str().unwrap().into(),
                    args,
                    cwd: Some(cwd.display().to_string()),
                    env: vec![],
                    name: Some("Maven long running goal".into()),
                },
                &[pending.origin()],
            )
            .unwrap();
        if let Some(previous) = previous {
            assert_ne!(session.id(), previous, "Rerun owns a fresh native session");
        }
        previous = Some(session.id());
        let deadline = Instant::now() + Duration::from_secs(180);
        let mut bytes = Vec::new();
        // ConPTY may insert cursor controls inside an echoed word at its wrap column. The actual
        // plugin-created marker proves execution reached the sleep; output bytes prove a live console.
        while !marker.exists() || bytes.is_empty() {
            manager.poll();
            for update in manager
                .take_native_execution_updates()
                .into_iter()
                .flat_map(|event| event.updates)
            {
                if let Update::Output { bytes: chunk, .. } = update {
                    bytes.extend(chunk);
                    if bytes.windows(4).any(|part| part == b"\x1b[6n") {
                        let _ = manager.input_execution(session.id(), b"\x1b[1;1R".to_vec());
                    }
                }
            }
            assert!(
                !matches!(
                    session.snapshot().state,
                    ExecutionState::Failed | ExecutionState::Exited
                ),
                "Long Maven goal ended early: {:?}; {}",
                session.snapshot(),
                String::from_utf8_lossy(&bytes)
            );
            assert!(
                Instant::now() < deadline,
                "Maven goal did not start in the configured directory: {}",
                String::from_utf8_lossy(&bytes)
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        manager.stop_execution(session.id()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while session.snapshot().state.is_active() {
            manager.poll();
            manager.take_native_execution_updates();
            assert!(
                Instant::now() < deadline,
                "Maven Stop did not finish actual cleanup"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        std::fs::remove_file(&marker).unwrap();
    }
    manager.shutdown_checked().unwrap();
}

/// Await only the authenticated contributor's original result; cold activation is part of this boundary.
fn configuration(manager: &mut Manager, method: &str, arguments: Value) -> Value {
    let pending = manager
        .begin_configuration_call("maven", method, arguments)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        manager.poll();
        if let RequestUpdate::Completed { result } = pending.status() {
            return result.unwrap();
        }
        assert!(Instant::now() < deadline, "Maven configuration deadline");
        std::thread::sleep(Duration::from_millis(10));
    }
}
