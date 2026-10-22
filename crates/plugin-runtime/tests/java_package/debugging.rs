//! Real Java bytecode debugging enters the same public manager as every independent provider.
use super::*;
use plugin_runtime::{DebugRequest, DebugState, plugin_protocol::api::RequestUpdate};

/// A cold Java project compiles first, then pauses in the actual JVM and reports its local value.
#[test]
#[ignore = "Build Java with --plugin-package and set NANOBUG_JAVA_PACKAGE to the resulting ZIP"]
fn java_package_debugs_a_real_breakpoint_without_an_open_document() {
    let package = Package::read(std::path::Path::new(
        &std::env::var_os("NANOBUG_JAVA_PACKAGE").unwrap(),
    ))
    .unwrap();
    let tree = std::env::var_os("NANOBUG_JAVA_DEBUG_TREE").is_some();
    if tree {
        assert!(
            package.manifest.plugin_services.provides["debug.session"]
                .methods
                .contains_key("threads"),
            "The Java package must offer versioned thread and variable-tree inspection"
        );
    }
    let workspace = tempfile::Builder::new()
        .prefix("Java 调试 ")
        .tempdir()
        .unwrap();
    let profile = tempfile::tempdir().unwrap();
    let profile_path = std::env::var_os("NANOBUG_JAVA_TEST_PROFILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| profile.path().into());
    let root = workspace.path().canonicalize().unwrap();
    let source = root.join("Main.java");
    let text = if tree {
        r#"import java.util.*;
public class Main {
 static volatile boolean ready;
 static class Node { int number = 42; Node next = this; }
 public static void main(String[] args) throws Exception {
  Thread worker = new Thread(() -> hold(), "worker中文"); worker.start(); while (!ready) Thread.yield();
  Node node = new Node(); int[] numbers = new int[250]; for (int i=0;i<numbers.length;i++) numbers[i]=i;
  List<String> collection = Arrays.asList("first", "second", "third");
  int value = 7;
  System.out.println(value);
  worker.join();
 }
 static void hold() { int workerValue = 99; ready = true; while (true) { try { Thread.sleep(1000); } catch (InterruptedException error) { return; } } }
}
"#
    } else {
        "public class Main {\n public static void main(String[] args) {\n  int value = 7;\n  System.out.println(value);\n }\n}\n"
    };
    let custom_cwd = std::env::var_os("NANOBUG_JAVA_DEBUG_CWD").is_some();
    let working_directory = if custom_cwd {
        let path = root.join("独立 run directory");
        std::fs::create_dir(&path).unwrap();
        path
    } else {
        root.clone()
    };
    let text = if custom_cwd {
        text.replace("int value = 7;", "try { new java.io.FileOutputStream(\"cwd-checked.txt\").close(); } catch (Exception error) { throw new RuntimeException(error); } int value = 7;")
    } else {
        text.into()
    };
    std::fs::write(&source, &text).unwrap();
    let breakpoint = text
        .lines()
        .position(|line| line.contains("System.out.println(value)"))
        .unwrap()
        + 1;
    let mut manager = Manager::open(
        profile_path,
        Environment {
            workspace: root.display().to_string(),
            os: "windows".into(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let major = std::env::var("NANOBUG_JAVA_PROJECT_JDK").unwrap_or_else(|_| "25".into());
    manager
        .update_setting(
            "java",
            plugin_runtime::plugin_protocol::settings::Scope::Project,
            "java.jdkVersion",
            Some(json!(major)),
        )
        .unwrap();
    if !manager.installed["java"].enabled {
        manager.enable("java").unwrap();
    }
    assert!(
        manager
            .debug_providers()
            .iter()
            .any(|provider| provider.plugin == "java"),
        "Java must publish its debugger through the public contract"
    );
    let prepare = manager.begin_target_call("java", "prepare", json!({"workspace":root.display().to_string(), "binding":json!({"schema":1,"main_class":"Main"}).to_string(), "env":[]})).unwrap();
    let deadline = Instant::now() + Duration::from_secs(180);
    let prepared = loop {
        manager.poll();
        if let RequestUpdate::Completed { result } = prepare.status() {
            break result.unwrap_or_else(|error| {
                panic!(
                    "Java debug compilation: {error:?}; output={}",
                    prepare.snapshot().output
                )
            });
        }
        assert!(Instant::now() < deadline, "Java compilation timed out");
        std::thread::sleep(Duration::from_millis(10));
    };
    // Retain a real editing backend through cutover as well as the independent paused debugger.
    let analysis = if std::env::var_os("NANOBUG_JAVA_DEBUG_WITH_LSP").is_some() {
        demand_support::ready(
            &mut manager,
            "java",
            DemandReason::Language {
                provider: "java/analysis".into(),
                role: LanguageRole::Lsp,
            },
        );
        let service = manager
            .language_services()
            .remove("java/analysis")
            .unwrap()
            .unwrap();
        let process = service.spawn().unwrap();
        Some((service, process))
    } else {
        None
    };
    let launch = json!({"program":prepared["program"], "args":prepared["prefix_args"], "cwd":working_directory.display().to_string(), "breakpoints":[{"source":source.display().to_string(),"line":breakpoint}]});
    let origin = prepare.origin();
    let start = manager
        .begin_configured_debug_call_with_origins(
            Some("java-matrix"),
            "start",
            launch.clone(),
            &[origin.clone()],
        )
        .unwrap();
    let receipt = answer(&mut manager, start);
    let session = receipt["session"].as_str().unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    let state = loop {
        let state = manager.debug_status(session).unwrap();
        if state.state == DebugState::Paused && state.source.is_some() {
            break state;
        }
        assert!(
            !matches!(state.state, DebugState::Exited | DebugState::Failed),
            "{state:?}"
        );
        assert!(
            Instant::now() < deadline,
            "Java did not hit the breakpoint: {state:?}"
        );
        manager.poll();
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(state.line, Some(breakpoint as u32));
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
            .any(|variable| variable.name == "value" && variable.value == "7"),
        "{variables:?}"
    );
    if tree {
        let pause = state.pause.unwrap();
        let request = manager
            .begin_debug_call("threads", json!({"session":session,"pause":pause}))
            .unwrap();
        let threads = answer(&mut manager, request);
        let main_thread = threads["selected"].as_u64().unwrap();
        let worker = threads["threads"]
            .as_array()
            .unwrap()
            .iter()
            .find(|thread| {
                thread["name"]
                    .as_str()
                    .is_some_and(|name| name.contains("worker中文"))
            })
            .unwrap_or_else(|| panic!("Missing real worker: {threads}"))["id"]
            .as_u64()
            .unwrap();
        let request = manager
            .begin_debug_call(
                "thread_frames",
                json!({"session":session,"pause":pause,"thread":main_thread}),
            )
            .unwrap();
        let main_frames = answer(&mut manager, request);
        let frame = main_frames["frames"]
            .as_array()
            .unwrap()
            .iter()
            .find(|frame| frame["name"].as_str().unwrap().contains("main"))
            .unwrap()["id"]
            .as_u64()
            .unwrap();
        let roots = children(&mut manager, session, pause, main_thread, frame, 0, 0, 100);
        let nodes = roots["variables"].as_array().unwrap();
        let reference = |name: &str| {
            nodes.iter().find(|node| node["name"] == name).unwrap()["reference"]
                .as_u64()
                .unwrap()
        };
        let fields = children(
            &mut manager,
            session,
            pause,
            main_thread,
            frame,
            reference("node"),
            0,
            100,
        );
        assert!(
            fields["variables"]
                .as_array()
                .unwrap()
                .iter()
                .any(|field| field["name"] == "number" && field["value"] == "42"),
            "{fields}"
        );
        let cycle = fields["variables"]
            .as_array()
            .unwrap()
            .iter()
            .find(|field| field["name"] == "next")
            .unwrap()["reference"]
            .as_u64()
            .unwrap();
        assert!(
            children(
                &mut manager,
                session,
                pause,
                main_thread,
                frame,
                cycle,
                0,
                100
            )["variables"]
                .as_array()
                .unwrap()
                .iter()
                .any(|field| field["name"] == "number")
        );
        let page = children(
            &mut manager,
            session,
            pause,
            main_thread,
            frame,
            reference("numbers"),
            100,
            100,
        );
        assert_eq!(page["variables"].as_array().unwrap().len(), 100);
        assert!(
            page["variables"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| node["value"] == "100"),
            "{page}"
        );
        assert_eq!(page["more"], true);
        let collection = children(
            &mut manager,
            session,
            pause,
            main_thread,
            frame,
            reference("collection"),
            0,
            2,
        );
        assert_eq!(collection["variables"].as_array().unwrap().len(), 2);
        let request = manager
            .begin_debug_call(
                "thread_frames",
                json!({"session":session,"pause":pause,"thread":worker}),
            )
            .unwrap();
        let worker_frames = answer(&mut manager, request);
        let worker_frame = worker_frames["frames"]
            .as_array()
            .unwrap()
            .iter()
            .find(|frame| frame["name"].as_str().unwrap().contains("hold"))
            .unwrap_or_else(|| panic!("Worker source frames: {worker_frames}"))["id"]
            .as_u64()
            .unwrap();
        let locals = children(
            &mut manager,
            session,
            pause,
            worker,
            worker_frame,
            0,
            0,
            100,
        );
        assert!(
            locals["variables"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| node["name"] == "workerValue" && node["value"] == "99"),
            "{locals}"
        );
        let wrong = manager.begin_debug_call("variable_children",json!({"session":session,"pause":pause,"thread":worker,"frame":worker_frame,"reference":reference("node"),"start":0,"count":100})).unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            manager.poll();
            if let RequestUpdate::Completed { result } = wrong.status() {
                assert!(
                    result.is_err(),
                    "A variable reference cannot cross thread/frame scope"
                );
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    if let Ok(retirement) = std::env::var("NANOBUG_JAVA_DEBUG_RETIRE") {
        // Exercise actual package management while the target and backend are still paused/live.
        match retirement.as_str() {
            "disable" => manager.disable("java").unwrap(),
            "uninstall" => manager.uninstall("java", false).unwrap(),
            "update" => {
                let mut files = package.files.clone();
                let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
                manifest["version"] = json!("0.1.99");
                files.insert(
                    "manifest.json".into(),
                    serde_json::to_vec(&manifest).unwrap(),
                );
                let declaration = String::from_utf8(files["plugin.toml"].clone())
                    .unwrap()
                    .replace(
                        &format!("version = \"{}\"", package.manifest.version),
                        "version = \"0.1.99\"",
                    );
                files.insert("plugin.toml".into(), declaration.into_bytes());
                let candidate = Package::from_files(files).unwrap();
                manager
                    .install(&candidate, candidate.manifest.permissions.clone())
                    .unwrap();
                assert_eq!(manager.installed["java"].manifest.version, "0.1.99");
            }
            _ => panic!("Unknown retirement verification mode"),
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            manager.poll();
            let status = manager
                .debug_status(session)
                .expect("Retired debug history remains locatable");
            if matches!(status.state, DebugState::Exited | DebugState::Failed) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "Native debug retirement was not observed: {status:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!origin.valid_for(&manager));
        assert!(
            manager
                .begin_configured_debug_call_with_origins(
                    Some("java-matrix"),
                    "start",
                    launch,
                    &[origin]
                )
                .is_err(),
            "Retirement cannot reuse a previous preparation's authority"
        );
        if let Some(instance) = manager.live.get("java") {
            assert_eq!(instance.process_count(), 0);
        }
        if let Some((service, _)) = &analysis {
            assert!(
                service.spawn().is_err(),
                "The previous editing backend cannot restart after native cutover"
            );
        }
        manager.shutdown_checked().unwrap();
        assert_eq!(manager.resource_count(), 0);
        return;
    }
    let stop = manager
        .begin_debug_call("stop", json!({"session":session}))
        .unwrap();
    answer(&mut manager, stop);
    assert_eq!(manager.live["java"].process_count(), 0);
    if custom_cwd {
        assert!(
            working_directory.join("cwd-checked.txt").is_file(),
            "The target must use its selected cwd independently of source indexing"
        );
    }
    assert!(!root.join(".metadata").exists());
    manager.disable("java").unwrap();
    assert!(
        manager
            .begin_configured_debug_call_with_origins(
                Some("java-matrix"),
                "start",
                launch,
                &[origin]
            )
            .is_err(),
        "A retired preparation source cannot create another debug target"
    );
}

/// Read one bounded page using explicit session, pause, thread and frame identities.
fn children(
    manager: &mut Manager,
    session: &str,
    pause: u64,
    thread: u64,
    frame: u64,
    reference: u64,
    start: u32,
    count: u32,
) -> Value {
    let request = manager.begin_debug_call("variable_children",json!({"session":session,"pause":pause,"thread":thread,"frame":frame,"reference":reference,"start":start,"count":count})).unwrap();
    answer(manager, request)
}

/// Wait for a public request's observed completion; failed/cancelled starts remain visible.
fn answer(manager: &mut Manager, request: DebugRequest) -> Value {
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        manager.poll();
        match request.status() {
            RequestUpdate::Completed { result } => return result.unwrap(),
            RequestUpdate::Cancelled { reason, .. } => {
                panic!("Java debug request cancelled: {reason:?}")
            }
            _ => {}
        }
        assert!(
            Instant::now() < deadline,
            "Java debugger response timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
