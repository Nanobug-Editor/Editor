//! Real local JARs verify source attachment and decompilation through the installed language service.
use super::*;
use std::{
    collections::BTreeMap,
    path::Path,
    process::{ChildStdin, Command},
};

/// Local attachments retain original source; a binary-only dependency remains navigable offline.
#[test]
#[ignore = "Build Java with --plugin-package; set NANOBUG_JAVA_PACKAGE; requires the installed javac and jar"]
fn local_jar_definitions_use_attached_sources_or_decompile_binary_content() {
    let package = Package::read(Path::new(
        &std::env::var_os("NANOBUG_JAVA_PACKAGE").unwrap(),
    ))
    .unwrap();
    let workspace = tempfile::Builder::new()
        .prefix("Java JAR 中文 ")
        .tempdir()
        .unwrap();
    let build = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(workspace.path().join("src")).unwrap();
    std::fs::create_dir_all(workspace.path().join("lib")).unwrap();
    create_libraries(build.path(), &workspace.path().join("lib"));
    let source = "import sample.Attached;\nimport sample.BinaryOnly;\npublic class Main { Attached attached; BinaryOnly binary; }\n";
    let path = workspace.path().join("src/Main.java");
    std::fs::write(&path, source).unwrap();
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
            "java.project",
            Some(json!(
                json!({"source_roots":["src"],"jars":["lib/attached.jar","lib/binary.jar"]})
                    .to_string()
            )),
        )
        .unwrap();
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
    let root = url::Url::from_directory_path(&service.root)
        .unwrap()
        .to_string();
    let uri = url::Url::from_file_path(path.canonicalize().unwrap())
        .unwrap()
        .to_string();
    let mut server = Server::new(&service, root.clone());
    let initialized = server.request(1, "initialize", json!({
        "processId":std::process::id(), "rootUri":root,
        "workspaceFolders":[{"uri":root,"name":"plain"}],
        "initializationOptions":service.provider.initialization_options,
        "capabilities":{"workspace":{"workspaceFolders":true},"textDocument":{"definition":{"linkSupport":true}}}
    }));
    assert!(
        initialized["capabilities"]["definitionProvider"].is_object()
            || initialized["capabilities"]["definitionProvider"] == true
    );
    server.send("initialized", json!({}));
    server.send(
        "textDocument/didOpen",
        json!({"textDocument":{"uri":uri,"version":1,"languageId":"java","text":source}}),
    );
    let diagnostic = server.request(
        2,
        "textDocument/diagnostic",
        json!({"textDocument":{"uri":uri}}),
    );
    assert!(
        diagnostic["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["severity"] != 1),
        "{diagnostic}"
    );
    for (index, (symbol, attached)) in [("Attached", true), ("BinaryOnly", false)]
        .into_iter()
        .enumerate()
    {
        let position = source.lines().nth(2).unwrap().find(symbol).unwrap() as u32 + 1;
        let definition = server.request(
            10 + index as u64 * 2,
            "textDocument/definition",
            json!({"textDocument":{"uri":uri},"position":{"line":2,"character":position}}),
        );
        let location = definition
            .as_array()
            .and_then(|items| items.first())
            .expect("Local JAR definition");
        let target = location
            .get("targetUri")
            .or_else(|| location.get("uri"))
            .and_then(Value::as_str)
            .unwrap();
        assert!(target.starts_with("jdt:"), "{target}");
        let content = server.request(
            11 + index as u64 * 2,
            "workspace/textDocumentContent",
            json!({"uri":target}),
        );
        let text = content["text"].as_str().expect("Local JAR content");
        assert!(text.contains(&format!("class {symbol}")), "{text}");
        assert_eq!(text.contains("ORIGINAL_SOURCE"), attached, "{text}");
        assert!(
            text.contains("jar-value"),
            "Decompiler must include the actual method body: {text}"
        );
    }
    drop(server);
    manager.disable("java").unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// Compile bytecode independently of the workspace so navigation cannot accidentally find fixture sources.
pub(super) fn create_libraries(build: &Path, libraries: &Path) {
    std::fs::create_dir_all(build.join("src/sample")).unwrap();
    let mut compiler = Command::new("javac");
    compiler
        .args(["--release", "8", "-encoding", "UTF-8", "-d"])
        .arg(build.join("classes"));
    for symbol in ["Attached", "BinaryOnly"] {
        let path = build.join(format!("src/sample/{symbol}.java"));
        std::fs::write(&path, format!("package sample; /** ORIGINAL_SOURCE {symbol} */ public class {symbol} {{ public String value() {{ return \"jar-value\"; }} }}\n")).unwrap();
        compiler.arg(path);
    }
    checked(&mut compiler);
    for (symbol, name) in [("Attached", "attached"), ("BinaryOnly", "binary")] {
        checked(
            Command::new("jar")
                .args(["--create", "--file"])
                .arg(libraries.join(format!("{name}.jar")))
                .arg("-C")
                .arg(build.join("classes"))
                .arg(format!("sample/{symbol}.class")),
        );
    }
    checked(
        Command::new("jar")
            .args(["--create", "--file"])
            .arg(libraries.join("attached-sources.jar"))
            .arg("-C")
            .arg(build.join("src"))
            .arg("sample/Attached.java"),
    );
}

/// Direct executable/argument arrays preserve Unicode paths and never concatenate shell commands.
fn checked(command: &mut Command) {
    let result = command.output().expect("Installed Java fixture tool");
    assert!(
        result.status.success(),
        "{:?}: {}",
        command,
        String::from_utf8_lossy(&result.stderr)
    );
}

/// The real framed connection owns its process; only immutable configuration and protocol messages are retained.
struct Server {
    _process: plugin_runtime::ServiceProcess,
    input: ChildStdin,
    receive: mpsc::Receiver<Value>,
    configuration: BTreeMap<String, Value>,
    root: String,
}
impl Server {
    /// Start one actual approved service, with an independent EOF reader to bound all waits.
    fn new(service: &plugin_runtime::LanguageService, root: String) -> Self {
        let (process, input, output) = service.spawn().unwrap();
        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || {
            let mut output = BufReader::new(output);
            while let Ok(message) = read_message(&mut output) {
                if send.send(message).is_err() {
                    break;
                }
            }
        });
        Self {
            _process: process,
            input,
            receive,
            configuration: service.provider.configuration.clone(),
            root,
        }
    }
    /// Notifications carry original source snapshots only until they are written to the owned pipe.
    fn send(&mut self, method: &str, params: Value) {
        write_message(
            &mut self.input,
            json!({"jsonrpc":"2.0","method":method,"params":params}),
        );
    }
    /// Standard server configuration calls are answered while awaiting the exact request ID.
    fn request(&mut self, id: u64, method: &str, params: Value) -> Value {
        write_message(
            &mut self.input,
            json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}),
        );
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            assert!(Instant::now() < deadline, "Java {method} timed out");
            let Ok(message) = self.receive.recv_timeout(Duration::from_secs(1)) else {
                continue;
            };
            if let Some(method) = message["method"].as_str() {
                if message.get("id").is_some() {
                    let result = match method {
                        "workspace/configuration" => Value::Array(
                            message["params"]["items"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|item| {
                                    let mut value = Value::Object(
                                        self.configuration.clone().into_iter().collect(),
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
                        "workspace/workspaceFolders" => json!([{"uri":self.root,"name":"plain"}]),
                        _ => Value::Null,
                    };
                    write_message(
                        &mut self.input,
                        json!({"jsonrpc":"2.0","id":message["id"],"result":result}),
                    );
                }
            } else if message["id"] == id {
                assert!(message["error"].is_null(), "{message}");
                return message["result"].clone();
            }
        }
    }
}
