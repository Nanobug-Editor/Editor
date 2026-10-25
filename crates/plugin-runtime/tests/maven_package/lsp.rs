//! A real framed Java transport verifies immutable unsaved fixtures without introducing host test APIs.
use super::*;
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    process::ChildStdin,
    sync::mpsc,
};

/// The approved service owns its process tree; fixture sources live only in standard LSP messages.
pub(super) struct Server {
    _process: plugin_runtime::ServiceProcess,
    input: ChildStdin,
    receive: mpsc::Receiver<Value>,
    configuration: BTreeMap<String, Value>,
    root: String,
}
impl Server {
    /// Only the Java 8 source module rejects records; the dependent Java 25 source remains valid.
    pub(super) fn analyze_compiler_levels(&mut self, workspace: &std::path::Path) {
        for (id, path, source, errors) in [
            (
                3,
                "lib/src/main/java/Library.java",
                "public class Library { record Value(int n) {} }",
                true,
            ),
            (
                4,
                "app/src/main/java/Main.java",
                "public class Main { record Value(int n) {} Library library; }",
                false,
            ),
        ] {
            let uri = url::Url::from_file_path(workspace.join(path).canonicalize().unwrap())
                .unwrap()
                .to_string();
            self.send(
                "textDocument/didOpen",
                json!({"textDocument":{"uri":uri,"languageId":"java","version":1,"text":source}}),
            );
            let diagnostic = self.request(
                id,
                "textDocument/diagnostic",
                json!({"textDocument":{"uri":uri}}),
            );
            assert_eq!(
                diagnostic["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|item| item["severity"] == 1),
                errors,
                "Compiler level diagnostics for {path}: {diagnostic}"
            );
        }
    }
    /// Both unsaved working copies participate in a rename initiated from the dependency project.
    pub(super) fn rename_library_member(&mut self, workspace: &std::path::Path) {
        let uri = |path: &str| {
            url::Url::from_file_path(workspace.join(path).canonicalize().unwrap())
                .unwrap()
                .to_string()
        };
        let library = uri("lib/src/main/java/Library.java");
        let app = uri("app/src/main/java/Main.java");
        for (uri, text) in [
            (
                &library,
                "public class Library { public String message() { return \"中文\"; } }",
            ),
            (
                &app,
                "public class Main { String value = new Library().message(); }",
            ),
        ] {
            self.send(
                "textDocument/didOpen",
                json!({"textDocument":{"uri":uri,"languageId":"java","version":1,"text":text}}),
            );
        }
        let result = self.request(3,"textDocument/rename",json!({"textDocument":{"uri":library},"position":{"line":0,"character":39},"newName":"greeting"}));
        let encoded = result.to_string();
        assert!(
            encoded.contains("Main.java") && encoded.contains("Library.java"),
            "A library rename must include its dependent module: {result}"
        );
        assert!(
            !encoded.contains("other/"),
            "Unrelated modules cannot receive rename edits: {result}"
        );
    }
    /// The selected module must retain Maven's exclusion even though the reactor sibling resolves it.
    pub(super) fn analyze_excluded_dependency(&mut self, workspace: &std::path::Path) {
        let uri = url::Url::from_file_path(
            workspace
                .join("app/src/main/java/Main.java")
                .canonicalize()
                .unwrap(),
        )
        .unwrap()
        .to_string();
        let source = "import fixture.Api; public class Main { int value = Api.answer(); }";
        self.send(
            "textDocument/didOpen",
            json!({"textDocument":{"uri":uri,"languageId":"java","version":1,"text":source}}),
        );
        let diagnostics = self.request(
            3,
            "textDocument/diagnostic",
            json!({"textDocument":{"uri":uri}}),
        );
        assert!(
            diagnostics["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["severity"] == 1
                    && item["message"]
                        .as_str()
                        .is_some_and(|message| message.contains("cannot be resolved"))),
            "The excluded type must remain unavailable: {diagnostics}"
        );
    }
    /// Spawn only the public Manager's exact prepared native service and bound every output wait.
    pub(super) fn new(service: &plugin_runtime::LanguageService) -> Self {
        let (process, input, output) = service.spawn().unwrap();
        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            while let Ok(message) = read(&mut reader) {
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
            root: url::Url::from_directory_path(&service.root)
                .unwrap()
                .to_string(),
        }
    }
    /// Initialize the actual backend with its approved project options and acknowledge standard readiness.
    pub(super) fn initialize(&mut self, service: &plugin_runtime::LanguageService) -> Value {
        let initialized=self.request(1,"initialize",json!({"processId":std::process::id(),"rootUri":self.root,"workspaceFolders":[{"uri":self.root,"name":"imported"}],"initializationOptions":service.provider.initialization_options,"capabilities":{"workspace":{"configuration":true},"textDocument":{"definition":{"linkSupport":true}}}}));
        self.send("initialized", json!({}));
        initialized
    }
    /// Unsaved cross-module members and a binary-only Maven JAR must both resolve in the same plan.
    pub(super) fn analyze_imported_project(&mut self, workspace: &std::path::Path) {
        let library = "public class Library { public String message() { return \"中文\"; } }";
        let source = "import fixture.Api;\npublic class Main {\n  public String 输出() { return new Library().message() + Api.answer(); }\n}\n";
        let uri = |relative: &str| {
            url::Url::from_file_path(workspace.join(relative).canonicalize().unwrap())
                .unwrap()
                .to_string()
        };
        let main = uri("app/src/main/java/Main.java");
        let library_uri = uri("lib/src/main/java/Library.java");
        for (uri, text) in [(&library_uri, library), (&main, source)] {
            self.send(
                "textDocument/didOpen",
                json!({"textDocument":{"uri":uri,"languageId":"java","version":1,"text":text}}),
            );
        }
        let diagnostic = self.request(
            3,
            "textDocument/diagnostic",
            json!({"textDocument":{"uri":main}}),
        );
        assert!(
            diagnostic["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|item| item["severity"] != 1),
            "Imported project diagnostics: {diagnostic}"
        );
        let line = source.lines().nth(2).unwrap();
        let character = line[..line.find("Library").unwrap() + 1]
            .encode_utf16()
            .count();
        let definition = self.request(
            4,
            "textDocument/definition",
            json!({"textDocument":{"uri":main},"position":{"line":2,"character":character}}),
        );
        let target = target_uri(&definition);
        assert_eq!(
            url::Url::parse(target)
                .unwrap()
                .to_file_path()
                .unwrap()
                .canonicalize()
                .unwrap(),
            workspace
                .join("lib/src/main/java/Library.java")
                .canonicalize()
                .unwrap()
        );
        let binary = self.request(
            5,
            "textDocument/definition",
            json!({"textDocument":{"uri":main},"position":{"line":0,"character":16}}),
        );
        let target = target_uri(&binary);
        assert!(
            target.starts_with("jdt:"),
            "The resolved Maven dependency uses readonly binary content: {binary}"
        );
        let content = self.request(6, "workspace/textDocumentContent", json!({"uri":target}));
        assert!(
            content["text"].as_str().unwrap().contains("class Api"),
            "{content}"
        );
    }
    /// Notifications preserve original source bytes until the single owned pipe consumes them.
    fn send(&mut self, method: &str, params: Value) {
        write(
            &mut self.input,
            json!({"jsonrpc":"2.0","method":method,"params":params}),
        );
    }
    /// Answer bounded standard backend queries while waiting for exactly the requested result ID.
    pub(super) fn request(&mut self, id: u64, method: &str, params: Value) -> Value {
        write(
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
                if message.get("id").is_none() {
                    continue;
                }
                let result = match method {
                    "workspace/configuration" => Value::Array(
                        message["params"]["items"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|item| {
                                let mut value =
                                    Value::Object(self.configuration.clone().into_iter().collect());
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
                    "workspace/workspaceFolders" => json!([{"uri":self.root,"name":"imported"}]),
                    _ => Value::Null,
                };
                write(
                    &mut self.input,
                    json!({"jsonrpc":"2.0","id":message["id"],"result":result}),
                );
            } else if message["id"] == id {
                assert!(message["error"].is_null(), "{message}");
                return message["result"].clone();
            }
        }
    }
}
/// Both standard definition response shapes preserve their original target URI.
fn target_uri(value: &Value) -> &str {
    let first = value
        .as_array()
        .and_then(|items| items.first())
        .expect("A real Java definition");
    first
        .get("targetUri")
        .or_else(|| first.get("uri"))
        .and_then(Value::as_str)
        .unwrap()
}
/// Frame writes use UTF-8 byte length, independent of Unicode character counts.
fn write(output: &mut impl Write, value: Value) {
    let bytes = serde_json::to_vec(&value).unwrap();
    write!(output, "Content-Length: {}\r\n\r\n", bytes.len()).unwrap();
    output.write_all(&bytes).unwrap();
    output.flush().unwrap();
}
/// EOF or malformed framing ends the reader; complete messages remain bounded to the actual LSP limit.
fn read(input: &mut impl BufRead) -> std::io::Result<Value> {
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
            length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(std::io::Error::other)?,
            );
        }
    }
    let length = length
        .filter(|length| *length <= 16 * 1024 * 1024)
        .ok_or_else(|| std::io::Error::other("Invalid LSP length"))?;
    let mut bytes = vec![0; length];
    input.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(std::io::Error::other)
}
