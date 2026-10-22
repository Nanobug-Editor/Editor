//! The independently built Java package enters through the same public manager as any language.
#[path = "java_package/configurations.rs"]
mod configurations;
#[path = "java_package/debugging.rs"]
mod debugging;
#[path = "support/demand.rs"]
mod demand_support;
#[path = "java_package/dependency_failures.rs"]
mod dependency_failures;
#[path = "java_package/execution.rs"]
mod execution;
#[path = "java_package/files.rs"]
mod files;
#[path = "java_package/libraries.rs"]
mod libraries;
use plugin_runtime::{DemandReason, LanguageRole, Manager, Package, plugin_protocol::Environment};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    sync::mpsc,
    time::{Duration, Instant},
};

/// The public builder validates the independent project and produces the same package as the CLI.
/// The SDK root must be an existing host-exported cache; no SDK checkout is added to the guest.
#[test]
#[ignore = "Set NANOBUG_JAVA_PROJECT and NANOBUG_JAVA_SDK to the source project and host-exported SDK"]
fn java_source_project_packages_through_the_public_builder() {
    let project = plugin_runtime::development::Project::read(std::path::Path::new(
        &std::env::var_os("NANOBUG_JAVA_PROJECT").expect("NANOBUG_JAVA_PROJECT"),
    ))
    .unwrap();
    let sdk =
        std::path::PathBuf::from(std::env::var_os("NANOBUG_JAVA_SDK").expect("NANOBUG_JAVA_SDK"))
            .canonicalize()
            .unwrap();
    let path = sdk
        .display()
        .to_string()
        .trim_start_matches(r"\\?\")
        .replace('\\', "/");
    let options = plugin_runtime::development::BuildOptions {
        release: true,
        cargo_args: vec![
            "--config".into(),
            format!(
                "patch.crates-io.plugin-protocol.path={}",
                serde_json::to_string(&path).unwrap()
            ),
        ],
        ..Default::default()
    };
    project
        .package(&options, &mut |bytes| {
            let _ = std::io::stderr().write_all(bytes);
        })
        .unwrap();
}

/// Installing the actual package yields semantic Java diagnostics and revokes its native process.
#[test]
#[ignore = "Build Java with --plugin-package and set NANOBUG_JAVA_PACKAGE to the resulting ZIP"]
fn actual_java_package_analyzes_plain_sources_and_retires_its_process() {
    let package_path = std::env::var_os("NANOBUG_JAVA_PACKAGE").expect("NANOBUG_JAVA_PACKAGE");
    let mut package = Package::read(std::path::Path::new(&package_path)).unwrap();
    let managed_runtime = std::env::var_os("NANOBUG_JAVA_FORCE_MANAGED_RUNTIME").is_some();
    if managed_runtime {
        // Change only discovery declarations in an otherwise real package. This avoids changing
        // global PATH and exercises the same public absent-tool path on machines with Java installed.
        let mut manifest: Value = serde_json::from_slice(&package.files["manifest.json"]).unwrap();
        for (id, service) in manifest["services"].as_object_mut().unwrap() {
            if id.starts_with("local") {
                service["program"] = json!("nanobug-java-fixture-absent-tool");
                service["search_paths"] = json!([]);
            }
        }
        package.files.insert(
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        );
        package = Package::from_files(package.files).unwrap();
    }
    let workspace = tempfile::tempdir().unwrap();
    let temporary_profile = tempfile::tempdir().unwrap();
    let profile = std::env::var_os("NANOBUG_JAVA_TEST_PROFILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| temporary_profile.path().to_path_buf());
    let source = "public class Main { public void test() { int number = \"wrong\"; } }";
    std::fs::write(workspace.path().join("Main.java"), source).unwrap();
    let mut manager = Manager::open(
        profile.clone(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let project_jdk = std::env::var("NANOBUG_JAVA_PROJECT_JDK").unwrap_or_else(|_| "25".into());
    manager
        .update_setting(
            "java",
            plugin_runtime::plugin_protocol::settings::Scope::Project,
            "java.jdkVersion",
            Some(json!(project_jdk)),
        )
        .unwrap();
    let explicit_home = std::env::var_os("NANOBUG_JAVA_PROJECT_HOME").map(std::path::PathBuf::from);
    if let Some(home) = &explicit_home {
        manager
            .update_setting(
                "java",
                plugin_runtime::plugin_protocol::settings::Scope::Project,
                "java.jdkHome",
                Some(json!(home)),
            )
            .unwrap();
    }
    // Configure while dormant; a reused isolated profile may retain the preceding deliberate disable.
    if !manager.installed["java"].enabled {
        manager.enable("java").unwrap();
    }
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
    let runtime = &service.provider.configuration["java"]["configuration"]["runtimes"][0];
    let expected = if project_jdk == "8" {
        "JavaSE-1.8".into()
    } else {
        format!("JavaSE-{project_jdk}")
    };
    assert_eq!(runtime["name"], expected);
    let project_home =
        std::path::Path::new(runtime["path"].as_str().expect("concrete project SDK path"));
    assert!(
        project_home.join("bin/javac.exe").is_file(),
        "project SDK must contain a real compiler"
    );
    if let Some(home) = explicit_home {
        assert_eq!(
            project_home.canonicalize().unwrap(),
            home.canonicalize().unwrap()
        );
    } else if managed_runtime {
        assert!(
            project_home
                .canonicalize()
                .unwrap()
                .starts_with(profile.canonicalize().unwrap()),
            "missing SDK belongs in isolated private storage: {}",
            project_home.display()
        );
    }
    let (process, mut input, output) = service.spawn().unwrap();
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(output);
        while let Ok(value) = read_message(&mut reader) {
            if send.send(value).is_err() {
                break;
            }
        }
    });
    let root = url::Url::from_directory_path(workspace.path().canonicalize().unwrap()).unwrap();
    let uri = root.join("Main.java").unwrap().to_string();
    let root = root.to_string();
    write_message(
        &mut input,
        json!({"jsonrpc":"2.0", "id":1, "method":"initialize", "params":{
            "processId":null, "rootUri":root, "workspaceFolders":[{"uri":root,"name":"plain"}],
            "capabilities":{"workspace":{"configuration":true},"textDocument":{"publishDiagnostics":{"versionSupport":true}}},
            "initializationOptions":service.provider.initialization_options
        }}),
    );
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        let message = receive.recv_timeout(Duration::from_secs(1));
        assert!(
            Instant::now() < deadline,
            "Java semantic diagnostics did not arrive"
        );
        let Ok(message) = message else {
            continue;
        };
        if message["id"] == 1 && message.get("method").is_none() {
            assert!(message["error"].is_null(), "{message}");
            assert!(message["result"]["capabilities"]["completionProvider"].is_object());
            assert!(
                message["result"]["capabilities"]["diagnosticProvider"].is_object(),
                "Java provides request-bound semantic diagnostics"
            );
            write_message(
                &mut input,
                json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            );
            write_message(
                &mut input,
                json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
                    "textDocument":{"uri":uri,"languageId":"java","version":7,"text":source}
                }}),
            );
            write_message(
                &mut input,
                json!({"jsonrpc":"2.0","id":10,"method":"textDocument/diagnostic",
                "params":{"textDocument":{"uri":uri}}}),
            );
        } else if message.get("id").is_some() && message.get("method").is_some() {
            let result = match message["method"].as_str().unwrap() {
                "workspace/configuration" => Value::Array(
                    message["params"]["items"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|item| {
                            let mut value = Value::Object(
                                service.provider.configuration.clone().into_iter().collect(),
                            );
                            for part in item["section"]
                                .as_str()
                                .unwrap_or("")
                                .split('.')
                                .filter(|part| !part.is_empty())
                            {
                                value = value[part].clone();
                            }
                            value
                        })
                        .collect(),
                ),
                "workspace/workspaceFolders" => json!([{"uri":root,"name":"plain"}]),
                _ => Value::Null,
            };
            write_message(
                &mut input,
                json!({"jsonrpc":"2.0","id":message["id"],"result":result}),
            );
        } else if message["id"] == 10 && message.get("method").is_none() {
            assert!(message["error"].is_null(), "{message}");
            assert_eq!(message["result"]["kind"], "full");
            assert!(
                message["result"]["items"]
                    .as_array()
                    .is_some_and(|items| items.iter().any(|item| item["severity"] == 1
                        && item["message"]
                            .as_str()
                            .is_some_and(|text| text.contains("Type mismatch")))),
                "{message}"
            );
            break;
        }
    }
    // Exercise standard editing requests against the actual prepared server, after proving errors.
    // Their results stay inert here; native revision/transaction application is verified separately.
    let edited = "public class Main {\n public static void main(String[] args) {\n  String value = \"text\";\n  System.out.println(value);\n }\n}\n";
    write_message(
        &mut input,
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":uri,"version":8},"contentChanges":[{"text":edited}]
        }}),
    );
    // didChange is a notification. Await a source-verified round for the new buffer before
    // checking semantic features, rather than racing JDT's document lifecycle jobs under load.
    write_message(
        &mut input,
        json!({"jsonrpc":"2.0","id":11,"method":"textDocument/diagnostic",
        "params":{"textDocument":{"uri":uri}}}),
    );
    let ready_deadline = Instant::now() + Duration::from_secs(30);
    loop {
        assert!(
            Instant::now() < ready_deadline,
            "updated Java source was not analyzed"
        );
        let Ok(message) = receive.recv_timeout(Duration::from_secs(1)) else {
            continue;
        };
        if message["id"] == 11 && message.get("method").is_none() {
            assert!(message["error"].is_null(), "{message}");
            assert_eq!(message["result"]["kind"], "full");
            assert!(
                message["result"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|item| item["severity"] != 1),
                "{message}"
            );
            break;
        }
    }
    let variable = edited.lines().nth(3).unwrap().find("value").unwrap();
    let position = json!({"line":3,"character":variable + 1});
    let requests = [
        (
            2,
            "textDocument/hover",
            json!({"textDocument":{"uri":uri},"position":position}),
        ),
        (
            3,
            "textDocument/definition",
            json!({"textDocument":{"uri":uri},"position":position}),
        ),
        (
            4,
            "textDocument/references",
            json!({"textDocument":{"uri":uri},"position":position,"context":{"includeDeclaration":true}}),
        ),
        (
            5,
            "textDocument/rename",
            json!({"textDocument":{"uri":uri},"position":position,"newName":"message"}),
        ),
        (
            6,
            "textDocument/formatting",
            json!({"textDocument":{"uri":uri},"options":{"tabSize":4,"insertSpaces":true}}),
        ),
        (
            7,
            "textDocument/completion",
            json!({"textDocument":{"uri":uri},"position":{"line":3,"character":9}}),
        ),
    ];
    for (id, method, params) in &requests {
        write_message(
            &mut input,
            json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}),
        );
    }
    let mut answers = std::collections::BTreeMap::new();
    let deadline = Instant::now() + Duration::from_secs(45);
    while answers.len() < requests.len() {
        assert!(
            Instant::now() < deadline,
            "Java editing responses did not arrive: {answers:?}"
        );
        let Ok(message) = receive.recv_timeout(Duration::from_secs(1)) else {
            continue;
        };
        if let Some(id) = message["id"].as_u64().filter(|id| (2..=7).contains(id)) {
            if message.get("method").is_none() {
                assert!(message["error"].is_null(), "{message}");
                answers.insert(id, message["result"].clone());
            }
        }
    }
    assert!(
        answers[&2]["contents"].to_string().contains("String"),
        "hover: {:?}",
        answers[&2]
    );
    assert!(
        answers[&3]
            .as_array()
            .is_some_and(|items| !items.is_empty()),
        "definition: {:?}",
        answers[&3]
    );
    assert!(
        answers[&4].as_array().is_some_and(|items| items.len() >= 2),
        "references: {:?}",
        answers[&4]
    );
    assert!(answers[&5].is_object(), "rename: {:?}", answers[&5]);
    assert!(
        answers[&6]
            .as_array()
            .is_some_and(|items| !items.is_empty()),
        "formatting: {:?}",
        answers[&6]
    );
    assert!(
        answers[&7]["items"]
            .as_array()
            .or_else(|| answers[&7].as_array())
            .is_some_and(|items| !items.is_empty()),
        "completion: {:?}",
        answers[&7]
    );
    // Standard definition must retain a non-file library identity and return real source content.
    write_message(
        &mut input,
        json!({"jsonrpc":"2.0","id":12,"method":"textDocument/definition",
        "params":{"textDocument":{"uri":uri},"position":{"line":2,"character":4}}}),
    );
    let library = response_for(&receive, 12);
    let location = library
        .as_array()
        .and_then(|items| items.first())
        .expect("JDK definition");
    let library_uri = location
        .get("targetUri")
        .or_else(|| location.get("uri"))
        .and_then(Value::as_str)
        .expect("JDK source URI");
    assert!(library_uri.starts_with("jdt:"), "{library_uri}");
    write_message(
        &mut input,
        json!({"jsonrpc":"2.0","id":13,"method":"workspace/textDocumentContent","params":{"uri":library_uri}}),
    );
    let content = response_for(&receive, 13);
    assert!(
        content["text"]
            .as_str()
            .is_some_and(|text| text.contains("class String")),
        "{content}"
    );
    drop(process);
    manager.disable("java").unwrap();
    assert!(
        service.spawn().is_err(),
        "retired language authority must reject another process"
    );
    assert!(
        !workspace.path().join(".project").exists(),
        "analysis metadata belongs in private storage"
    );
}

/// Await one ordinary response while allowing unrelated diagnostics/progress notifications.
fn response_for(receive: &mpsc::Receiver<Value>, id: u64) -> Value {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        assert!(Instant::now() < deadline, "Java response {id} timed out");
        if let Ok(message) = receive.recv_timeout(Duration::from_secs(1)) {
            if message["id"] == id && message.get("method").is_none() {
                assert!(message["error"].is_null(), "{message}");
                return message["result"].clone();
            }
        }
    }
}

/// An explicitly selected local project JDK must fail visibly instead of downloading a substitute.
#[test]
#[ignore = "Build Java with --plugin-package and set NANOBUG_JAVA_PACKAGE to the resulting ZIP"]
fn actual_java_package_reports_an_invalid_explicit_project_jdk() {
    let package = Package::read(std::path::Path::new(
        &std::env::var_os("NANOBUG_JAVA_PACKAGE").unwrap(),
    ))
    .unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let missing = workspace.path().join("not-an-installed-jdk");
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
            plugin_runtime::plugin_protocol::settings::Scope::Project,
            "java.jdkHome",
            Some(json!(missing)),
        )
        .unwrap();
    let error = demand_support::try_ready(
        &mut manager,
        "java",
        DemandReason::Language {
            provider: "java/analysis".into(),
            role: LanguageRole::Lsp,
        },
    )
    .unwrap_err();
    assert!(error.message.contains("NotFound"), "{error:?}");
    assert_eq!(manager.resource_count(), 0);
    assert!(!missing.exists());
}

/// The real package shares one analysis owner across distinct source discovery intents.
#[test]
#[ignore = "Build Java with --plugin-package and set NANOBUG_JAVA_PACKAGE to the resulting ZIP"]
fn actual_java_package_shares_its_context_between_source_documents() {
    use plugin_runtime::plugin_protocol::language::DocumentContext;
    let package = Package::read(std::path::Path::new(
        &std::env::var_os("NANOBUG_JAVA_PACKAGE").unwrap(),
    ))
    .unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let documents = [
        DocumentContext {
            path: "Main.java".into(),
            generation: 4,
        },
        DocumentContext {
            path: "Second.java".into(),
            generation: 9,
        },
    ];
    for document in &documents {
        std::fs::write(workspace.path().join(&document.path), "class Source {}\n").unwrap();
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
    let mut services = Vec::new();
    for document in &documents {
        demand_support::ready(
            &mut manager,
            "java",
            DemandReason::LanguageDocument {
                provider: "java/analysis".into(),
                role: LanguageRole::Lsp,
                document: document.clone(),
            },
        );
        services.push(
            manager
                .language_document_service("java/analysis", document)
                .unwrap()
                .unwrap(),
        );
    }
    assert!(
        std::sync::Arc::ptr_eq(&services[0], &services[1]),
        "ordinary project sources must share the original transport owner"
    );
    manager.release_language_document("java/analysis", &documents[0]);
    assert!(
        services[1].is_active(),
        "closing one source must retain its peer's analysis owner"
    );
    manager.disable("java").unwrap();
    assert!(!services[1].is_active());
    assert_eq!(manager.resource_count(), 0);
}

/// Send one standard LSP frame; `value` is structured protocol data, never shell command text.
fn write_message(output: &mut impl Write, value: Value) {
    let body = serde_json::to_vec(&value).unwrap();
    write!(output, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
    output.write_all(&body).unwrap();
    output.flush().unwrap();
}

/// Decode bounded standard LSP framing from the real service stdout; EOF terminates the reader.
fn read_message(input: &mut impl BufRead) -> std::io::Result<Value> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        if line == "\r\n" {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        }
        if line.len() > 4096 {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
    }
    let length = length
        .filter(|value| *value <= 16 * 1024 * 1024)
        .ok_or(std::io::ErrorKind::InvalidData)?;
    let mut body = vec![0; length];
    input.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}
