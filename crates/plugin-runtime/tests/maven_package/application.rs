//! The Maven configuration keeps build-tool policy separate from Java's ordinary native launcher.
use super::*;
use plugin_runtime::{
    ExecutionState, RunRequest,
    plugin_protocol::{configurations as contract, process::Update},
};

/// Every launch revalidates its selected model and includes reactor outputs and runtime-only JARs.
#[test]
#[ignore = "Build Maven and Java; set NANOBUG_MAVEN_PACKAGE and NANOBUG_JAVA_PACKAGE"]
fn maven_application_uses_java_target_and_real_reactor_runtime_classpath() {
    let workspace = tempfile::Builder::new()
        .prefix("Maven 应用 ")
        .tempdir()
        .unwrap();
    let profile = tempfile::tempdir().unwrap();
    for path in ["lib/src/main/java", "app/src/main/java"] {
        std::fs::create_dir_all(workspace.path().join(path)).unwrap();
    }
    std::fs::write(workspace.path().join("pom.xml"),r#"<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><groupId>sample</groupId><artifactId>parent</artifactId><version>1</version><packaging>pom</packaging><properties><maven.compiler.release>25</maven.compiler.release></properties><modules><module>lib</module><module>app</module></modules></project>"#).unwrap();
    for module in ["lib", "app"] {
        let mut dependencies = if module == "app" {
            r#"<dependencies><dependency><groupId>sample</groupId><artifactId>lib</artifactId><version>1</version></dependency><dependency><groupId>fixture</groupId><artifactId>leaf</artifactId><version>1</version><scope>runtime</scope></dependency></dependencies>"#
        } else {
            ""
        }.to_owned();
        if module == "app" {
            for index in 0..64 {
                dependencies=dependencies.replace("</dependencies>",&format!("<dependency><groupId>fixture</groupId><artifactId>padding-{index}</artifactId><version>1</version></dependency></dependencies>"));
            }
        }
        std::fs::write(workspace.path().join(format!("{module}/pom.xml")),format!(r#"<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><parent><groupId>sample</groupId><artifactId>parent</artifactId><version>1</version></parent><artifactId>{module}</artifactId>{dependencies}</project>"#)).unwrap();
    }
    std::fs::write(
        workspace.path().join("lib/src/main/java/Library.java"),
        "public class Library { public static String text() { return \"中文 reactor\"; } }",
    )
    .unwrap();
    std::fs::write(workspace.path().join("app/src/main/java/Main.java"),"public class Main {\n public static void main(String[] args) throws Exception {\n  int result = ((Integer)Class.forName(\"fixture.Leaf\").getMethod(\"value\").invoke(null)).intValue();\n  String text = Library.text();\n  System.out.println(text + \":\" + result + \":\" + args[0]);\n }\n}\n").unwrap();
    repository::seed(&profile.path().join("m2"));
    for index in 0..64 {
        let directory = profile.path().join(format!("m2/fixture/padding-{index}/1"));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::copy(
            profile.path().join("m2/fixture/api/1/api-1.jar"),
            directory.join(format!("padding-{index}-1.jar")),
        )
        .unwrap();
        std::fs::write(directory.join(format!("padding-{index}-1.pom")),format!("<project><modelVersion>4.0.0</modelVersion><groupId>fixture</groupId><artifactId>padding-{index}</artifactId><version>1</version></project>")).unwrap();
    }
    let mut manager = Manager::open(
        profile.path().join("plugins"),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    for variable in ["NANOBUG_MAVEN_PACKAGE", "NANOBUG_JAVA_PACKAGE"] {
        eprintln!("Installing {variable}");
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
            Some(json!(profile.path().join("m2"))),
        )
        .unwrap();
    let root = manager.workspace().to_owned();
    eprintln!("Reading Maven application catalog");
    let catalog: contract::Catalog = contract::decode(configuration(
        &mut manager,
        "catalog",
        json!({"workspace":root,"locale":"zh-CN","os":"windows"}),
    ))
    .unwrap();
    let template = catalog
        .templates
        .iter()
        .find(|entry| entry.id == "application")
        .expect("Maven must provide a Java application template");
    let mut values: Value = serde_json::from_str(&template.defaults).unwrap();
    values["module"] = json!("sample:app:1");
    values["main_class"] = json!("Main");
    values["command"]["args"] = json!(["literal & 中文"]);
    maven_command(
        &mut manager,
        "import",
        json!({"pom":"pom.xml","profiles":[],"offline":false}),
    )
    .unwrap();
    eprintln!("Imported reactor; validating Java application projection");
    let validation:contract::Validation=contract::decode(configuration(&mut manager,"validate",json!({"workspace":root,"locale":"zh-CN","os":"windows","template":"application","values":values.to_string(),"intent":"run"}))).unwrap();
    assert!(validation.valid, "{}", validation.message);
    let launch = validation.launch.unwrap();
    assert!(
        launch.env.values().any(|value| value.len() > 4096),
        "A real server classpath must exceed the individual argument budget"
    );
    assert_eq!(launch.target["provider"], "java");
    assert_eq!(launch.build.len(), 1);
    assert!(
        !workspace
            .path()
            .join("app/target/classes/Main.class")
            .exists(),
        "Validation cannot build the project"
    );
    let build = &launch.build[0]["target"];
    let build_preparation = prepare(&mut manager, "maven", &root, build, &launch.env);
    let build_output = execute(
        &mut manager,
        &build_preparation,
        build,
        &root,
        "maven-app-build",
        &launch.env,
    );
    assert!(build_output.contains("BUILD SUCCESS"), "{build_output}");
    let target_preparation = prepare(&mut manager, "java", &root, &launch.target, &launch.env);
    let output = execute(
        &mut manager,
        &target_preparation,
        &launch.target,
        &root,
        "maven-app-run",
        &launch.env,
    );
    assert!(output.contains("中文 reactor:7:literal & 中文"), "{output}");
    assert!(
        !workspace.path().join("out").exists(),
        "Java cannot replace Maven's build output"
    );
    let RequestUpdate::Completed { result } = target_preparation.status() else {
        panic!("Java target was not prepared");
    };
    let prepared = result.unwrap();
    let mut args = prepared["prefix_args"].as_array().unwrap().clone();
    args.extend(launch.target["args"].as_array().unwrap().iter().cloned());
    eprintln!("Debugging the actual Maven-built application");
    let source = workspace
        .path()
        .join("app/src/main/java/Main.java")
        .canonicalize()
        .unwrap();
    let env = launch
        .env
        .iter()
        .map(|(name, value)| json!({"name":name,"value":value}))
        .collect::<Vec<_>>();
    let debug_launch = json!({"program":prepared["program"],"args":args,"cwd":root,"env":env,"breakpoints":[{"source":source,"line":5}]});
    let start = manager
        .begin_configured_debug_call_with_origins(
            Some("maven-app-debug"),
            "start",
            debug_launch.clone(),
            &[target_preparation.origin()],
        )
        .unwrap();
    let receipt = debug_answer(&mut manager, start);
    let session = receipt["session"].as_str().unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    let state = loop {
        manager.poll();
        let state = manager.debug_status(session).unwrap();
        if state.state == plugin_runtime::DebugState::Paused && state.source.is_some() {
            break state;
        }
        assert!(
            !matches!(
                state.state,
                plugin_runtime::DebugState::Exited | plugin_runtime::DebugState::Failed
            ),
            "{state:?}"
        );
        assert!(
            Instant::now() < deadline,
            "Maven application breakpoint: {state:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(state.line, Some(5));
    let frames = manager.debug_frames(session, state.pause.unwrap()).unwrap();
    let frame = frames
        .iter()
        .find(|frame| frame.name.contains("main"))
        .unwrap();
    let variables = manager
        .debug_variables(session, state.pause.unwrap(), frame.id)
        .unwrap();
    assert!(
        variables
            .iter()
            .any(|variable| variable.name == "result" && variable.value == "7"),
        "{variables:?}"
    );
    let stop = manager
        .begin_debug_call("stop", json!({"session":session}))
        .unwrap();
    debug_answer(&mut manager, stop);
    assert_eq!(manager.live["java"].process_count(), 0);
    // A process-only caller can check metadata lifetime without borrowing a file read permission.
    manager.disable("maven").unwrap();
    let stale = manager
        .begin_configured_debug_call_with_origins(
            Some("maven-stale-debug"),
            "start",
            debug_launch,
            &[target_preparation.origin()],
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        manager.poll();
        match stale.status() {
            RequestUpdate::Completed { result } => {
                assert_eq!(
                    result.unwrap_err().code,
                    plugin_runtime::plugin_protocol::api::ErrorCode::InvalidHandle
                );
                break;
            }
            RequestUpdate::Cancelled { reason, .. } => {
                assert_eq!(
                    reason,
                    plugin_runtime::plugin_protocol::api::ErrorCode::InvalidHandle
                );
                break;
            }
            _ => {}
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(manager.live["java"].process_count(), 0);
    manager.shutdown_checked().unwrap();
}

/// Each real debug request retains its original completion and bounded native preparation output.
fn debug_answer(manager: &mut Manager, pending: plugin_runtime::DebugRequest) -> Value {
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        manager.poll();
        match pending.status() {
            RequestUpdate::Completed { result } => return result.unwrap(),
            RequestUpdate::Cancelled { reason, .. } => {
                panic!("Debug request cancelled: {reason:?}")
            }
            _ => {}
        }
        assert!(Instant::now() < deadline, "Debug request timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Keep the real provider origin attached to each independently prepared step.
fn prepare(
    manager: &mut Manager,
    provider: &str,
    root: &str,
    target: &Value,
    env: &std::collections::BTreeMap<String, String>,
) -> plugin_runtime::TargetRequest {
    eprintln!("Preparing real {provider} target");
    let pending = manager
        .begin_target_call(
            provider,
            "prepare",
            json!({"workspace":root,"binding":target["binding"],"env":env.iter().map(|(name,value)|json!({"name":name,"value":value})).collect::<Vec<_>>()}),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        manager.poll();
        match pending.status() {
            RequestUpdate::Completed { result } => {
                result.unwrap_or_else(|error| {
                    panic!(
                        "{provider} preparation: {error:?}; {:?}",
                        pending.snapshot()
                    )
                });
                return pending;
            }
            _ => {}
        }
        assert!(
            Instant::now() < deadline,
            "{provider} preparation timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Execute actual native steps through ConPTY, with the original target prefix and literal argv.
fn execute(
    manager: &mut Manager,
    pending: &plugin_runtime::TargetRequest,
    target: &Value,
    root: &str,
    id: &str,
    env: &std::collections::BTreeMap<String, String>,
) -> String {
    let RequestUpdate::Completed { result } = pending.status() else {
        panic!("Target was not prepared");
    };
    let prepared = result.unwrap();
    let mut args: Vec<String> = serde_json::from_value(prepared["prefix_args"].clone()).unwrap();
    args.extend(serde_json::from_value::<Vec<String>>(target["args"].clone()).unwrap());
    let session = manager
        .start_terminal_configuration_execution_with_origins(
            id,
            RunRequest {
                program: prepared["program"].as_str().unwrap().into(),
                args,
                cwd: Some(root.into()),
                env: env
                    .iter()
                    .map(|(name, value)| plugin_runtime::RunEnvEntry {
                        name: name.clone(),
                        value: value.clone(),
                    })
                    .collect(),
                name: Some(id.into()),
            },
            &[pending.origin()],
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut output = Vec::new();
    let mut code = None;
    while code.is_none() {
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
                        let _ = manager.input_execution(session.id(), b"\x1b[1;1R".to_vec());
                    }
                }
                Update::Exited { code: exit } => code = Some(exit),
                _ => {}
            }
        }
        assert_ne!(
            session.snapshot().state,
            ExecutionState::Failed,
            "{:?}",
            session.snapshot()
        );
        assert!(
            Instant::now() < deadline,
            "Execution timed out: {}",
            String::from_utf8_lossy(&output)
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = String::from_utf8_lossy(&output).into_owned();
    assert_eq!(code, Some(0), "{output}");
    output
}

/// Read a current native configuration result, including the real plugin's asynchronous preparation.
fn configuration(manager: &mut Manager, method: &str, args: Value) -> Value {
    let pending = manager
        .begin_configuration_call("maven", method, args)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        manager.poll();
        match pending.status() {
            RequestUpdate::Completed { result } => return result.unwrap(),
            RequestUpdate::Cancelled { reason, .. } => panic!(
                "Configuration {method} cancelled: {reason:?}; {:?}; Maven logs {:?}; Java logs {:?}",
                pending.snapshot(),
                manager.runtime_logs().records("maven"),
                manager.runtime_logs().records("java")
            ),
            _ => {}
        }
        assert!(
            Instant::now() < deadline,
            "Configuration {method} deadline: {:?}",
            pending.snapshot()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
