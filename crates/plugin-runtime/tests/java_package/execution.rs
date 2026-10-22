//! Java build targets use actual package policy and the public native execution session boundary.
use super::*;
use plugin_runtime::{
    ExecutionState, RunRequest,
    plugin_protocol::{api::RequestUpdate, process::Update, settings},
};

/// A cold project selects its JDK and compiles a source without starting a language server or target.
#[test]
#[ignore = "Build Java with --plugin-package and set NANOBUG_JAVA_PACKAGE to the resulting ZIP"]
fn java_package_cold_build_uses_the_selected_project_compiler() {
    let archive = std::path::PathBuf::from(std::env::var_os("NANOBUG_JAVA_PACKAGE").unwrap());
    let mut package = Package::read(&archive).unwrap();
    if std::env::var_os("NANOBUG_JAVA_OFFLINE_CACHE").is_some() {
        // Keep the publisher's version/hash and real guest. With local discovery absent and the
        // source unavailable, only an already verified immutable private SDK can complete this run.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let unavailable = format!(
            "http://{}/unavailable-jdk.zip",
            listener.local_addr().unwrap()
        );
        drop(listener);
        let major = std::env::var("NANOBUG_JAVA_PROJECT_JDK").unwrap();
        let mut manifest: Value = serde_json::from_slice(&package.files["manifest.json"]).unwrap();
        manifest["services"][format!("local{major}")]["program"] =
            json!("deliberately-absent-java-sdk-fixture");
        manifest["services"][format!("local{major}")]["search_paths"] = json!([]);
        manifest["services"][format!("managed{major}")]["installation"]["artifacts"][0]["source"] =
            json!({"kind":"url","url":unavailable});
        package.files.insert(
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        );
        package = Package::from_files(package.files).unwrap();
    }
    let workspace = tempfile::Builder::new()
        .prefix("Java 构建 空格 ")
        .tempdir()
        .unwrap();
    let profile = tempfile::tempdir().unwrap();
    let profile_path = std::env::var_os("NANOBUG_JAVA_TEST_PROFILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| profile.path().into());
    let root = workspace.path().canonicalize().unwrap();
    let complex = std::env::var_os("NANOBUG_JAVA_PLAIN_PROJECT").is_some();
    let library_build = tempfile::tempdir().unwrap();
    let (main, class_file) = if complex {
        for directory in ["源码/app/demo", "源码/common/helper"] {
            std::fs::create_dir_all(root.join(directory)).unwrap();
        }
        let libraries = library_build.path().join("依赖 jar");
        std::fs::create_dir_all(&libraries).unwrap();
        super::libraries::create_libraries(library_build.path(), &libraries);
        std::fs::write(root.join("源码/common/helper/Message.java"), "package helper; public class Message { public String value() { return \"中文 build \"; } }\n").unwrap();
        std::fs::write(root.join("源码/app/demo/Main.java"), "package demo; public class Main { public static void main(String[] args) { System.out.println(new helper.Message().value() + new sample.Attached().value()); } }\n").unwrap();
        std::fs::write(root.join("源码/app/demo/Other.java"), "package demo; public class Other { public static void main(String[] args) { System.out.println(\"secondary-choice\"); } }\n").unwrap();
        ("demo.Main", "输出 classes/demo/Main.class")
    } else {
        std::fs::write(root.join("Main.java"), "public class Main { public static void main(String[] args) { System.out.println(\"中文 build\"); } }\n").unwrap();
        ("Main", "out/Main.class")
    };
    let mut manager = Manager::open(
        profile_path,
        Environment {
            workspace: root.display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager.runtime_observations().start_capture();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    if complex {
        manager
            .update_setting(
                "java",
                settings::Scope::Project,
                "java.project",
                Some(json!(json!({
            "source_roots":["源码/app","源码/common"],
            "jars":[library_build.path().join("依赖 jar/attached.jar").display().to_string()],
            "output":"输出 classes", "main_class":main
        }).to_string())),
            )
            .unwrap();
    }
    let major = std::env::var("NANOBUG_JAVA_PROJECT_JDK").unwrap_or_else(|_| "25".into());
    manager
        .update_setting(
            "java",
            settings::Scope::Project,
            "java.jdkVersion",
            Some(json!(major)),
        )
        .unwrap();
    if !manager.installed["java"].enabled {
        manager.enable("java").unwrap();
    }
    assert!(
        manager
            .target_providers()
            .iter()
            .any(|provider| provider == "java"),
        "Java must supply its cold build target through the public run.targets contract"
    );
    let pending = manager
        .begin_target_call(
            "java",
            "prepare",
            json!({
                "workspace":root.display().to_string(),
                "binding":json!({"schema":1,"main_class":main}).to_string(), "env":[]
            }),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(180);
    let prepared = loop {
        manager.poll();
        match pending.status() {
            RequestUpdate::Completed { result } => {
                break result.unwrap_or_else(|error| {
                    let observations = manager.runtime_observations().snapshot(Some("java"));
                    panic!(
                        "Java preparation: {error:?}; output={}; timings={:?}",
                        pending.snapshot().output,
                        observations.records
                    );
                });
            }
            RequestUpdate::Cancelled { reason, .. } => {
                panic!("Java build preparation ended: {reason:?}")
            }
            _ => {}
        }
        assert!(
            Instant::now() < deadline,
            "Java build preparation did not finish"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(pending.origin().valid_for(&manager));
    assert!(
        root.join(class_file).is_file(),
        "Target preparation must finish compilation before returning the executable"
    );
    assert!(
        !pending.snapshot().output.contains("中文 build"),
        "Preparation cannot execute main"
    );
    for service in manager
        .language_services()
        .values()
        .filter_map(|service| service.as_ref().ok())
    {
        assert_eq!(
            service.process_count(),
            0,
            "Compiler preparation must not start JDT LS"
        );
    }
    let request = RunRequest {
        program: prepared["program"].as_str().unwrap().into(),
        args: serde_json::from_value(prepared["prefix_args"].clone()).unwrap(),
        cwd: Some(root.display().to_string()),
        name: Some("Java Build".into()),
        env: Vec::new(),
    };
    let session = manager
        .start_terminal_configuration_execution("java-build", request)
        .unwrap();
    let mut exit = None;
    let mut updates = Vec::new();
    let mut compiler_output = Vec::new();
    let mut cursor_query_answered = false;
    loop {
        manager.poll();
        for message in manager.take_native_execution_updates() {
            for update in message.updates {
                if let Update::Output { bytes, .. } = &update {
                    compiler_output.extend_from_slice(bytes);
                    // This headless consumer owns VT input just as the native terminal does.
                    if !cursor_query_answered
                        && compiler_output.windows(4).any(|part| part == b"\x1b[6n")
                    {
                        manager
                            .input_execution(session.id(), b"\x1b[1;1R".to_vec())
                            .unwrap();
                        cursor_query_answered = true;
                    }
                }
                if let Update::Exited { code } = update {
                    exit = Some(code);
                }
                updates.push(format!("{update:?}"));
            }
        }
        let snapshot = session.snapshot();
        assert_ne!(
            snapshot.state,
            ExecutionState::Failed,
            "{:?}",
            snapshot.failure
        );
        if snapshot.state == ExecutionState::Exited && exit.is_some() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Java compiler did not finish: {updates:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(exit, Some(0), "Compiler observations: {updates:?}");
    assert!(
        String::from_utf8_lossy(&compiler_output).contains("中文 build"),
        "The prepared application runs only after explicit native execution: {updates:?}"
    );
    if complex {
        assert!(
            String::from_utf8_lossy(&compiler_output).contains("jar-value"),
            "The explicit local JAR must be on the runtime classpath"
        );
        assert!(root.join("输出 classes/helper/Message.class").is_file());
        // A second application uses its explicit portable binding, independently of project defaults
        // and the already completed first configuration. Preparing it must not run either main.
        let second = manager.begin_target_call("java","prepare",json!({"workspace":root.display().to_string(),"binding":"{\"schema\":1,\"main_class\":\"demo.Other\"}","env":[]})).unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        let other = loop {
            manager.poll();
            if let RequestUpdate::Completed { result } = second.status() {
                break result.unwrap();
            }
            assert!(
                Instant::now() < deadline,
                "The second main was not prepared"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(
            other["prefix_args"].as_array().unwrap().last().unwrap(),
            "demo.Other"
        );
        assert!(root.join("输出 classes/demo/Other.class").is_file());
        assert!(!second.snapshot().output.contains("secondary-choice"));
        // A failed compile must not return the previous executable, even with class output present.
        std::fs::write(
            root.join("源码/app/demo/Main.java"),
            "package demo; public class Main { int value = \"broken\"; }\n",
        )
        .unwrap();
        let failure = manager
            .begin_target_call(
                "java",
                "prepare",
                json!({"workspace":root.display().to_string(),
            "binding":json!({"schema":1,"main_class":main}).to_string(),"env":[]}),
            )
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            manager.poll();
            match failure.status() {
                RequestUpdate::Completed { result } => {
                    assert!(
                        result.is_err(),
                        "Compilation failure cannot supply a stale launch artifact"
                    );
                    let output = failure.snapshot().output;
                    assert!(
                        output.contains("源码") && output.contains("Main.java"),
                        "Readable Unicode compiler diagnostics: {output}"
                    );
                    break;
                }
                RequestUpdate::Cancelled { reason, .. } => {
                    panic!("Compile error did not complete: {reason:?}")
                }
                _ => {}
            }
            assert!(Instant::now() < deadline, "Compiler error was not observed");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    let class = std::fs::read(root.join(class_file)).unwrap();
    assert_eq!(&class[..4], &[0xca, 0xfe, 0xba, 0xbe]);
    let expected = match major.as_str() {
        "8" => 52,
        "11" => 55,
        "17" => 61,
        "21" => 65,
        "25" => 69,
        _ => panic!("Unsupported JDK fixture"),
    };
    assert_eq!(u16::from_be_bytes([class[6], class[7]]), expected);
    manager.shutdown_checked().unwrap();
}
