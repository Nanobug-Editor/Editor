//! Test-only stdio server records the real wire and echoes unsaved text through diagnostics.
use serde_json::{Value, json};
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};

/// Flush each frame so tests observe protocol order without simulated transport.
fn send(value: Value) {
    let bytes = serde_json::to_vec(&value).unwrap();
    let mut output = std::io::stdout().lock();
    write!(output, "Content-Length: {}\r\n\r\n", bytes.len()).unwrap();
    output.write_all(&bytes).unwrap();
    output.flush().unwrap();
}

/// Match only the fixture's explicitly selected physical dependency, independently of URI spelling.
fn is_related_source(document: &Value, expected: &Path) -> bool {
    document["uri"]
        .as_str()
        .and_then(|uri| url::Url::parse(uri).ok())
        .and_then(|uri| uri.to_file_path().ok())
        .and_then(|path| path.canonicalize().ok())
        .is_some_and(|path| path == expected)
}

fn main() {
    let log = std::env::args().nth(1).expect("wire log path");
    // This opt-in backend reads one declared dependency from its overlay or, after didClose, disk.
    // The response demonstrates source lifetime behavior; it does not emulate a compiler's semantics.
    let related_source = std::env::args()
        .collect::<Vec<_>>()
        .windows(2)
        .find(|pair| pair[0] == "--related-source")
        .map(|pair| PathBuf::from(&pair[1]).canonicalize().unwrap());
    let mut related_overlay = None::<String>;
    // Explicit initialization data supplies whole standard replies for editing-contract tests.
    // This test backend performs no rename inference and never writes the requested source files.
    let mut rename_replies = Value::Null;
    // Startup arguments are observable independently of initialization JSON.
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
        .unwrap();
    writeln!(
        file,
        "{}",
        json!({"startupArgs":std::env::args().collect::<Vec<_>>()})
    )
    .unwrap();
    let mut input = std::io::BufReader::new(std::io::stdin().lock());
    // Only the first reference is withheld; the same live server must accept cancellation and reuse.
    let mut reference_ignored = false;
    loop {
        let mut length = 0;
        loop {
            let mut line = String::new();
            if input.read_line(&mut line).unwrap() == 0 {
                return;
            }
            if line == "\r\n" {
                break;
            }
            if let Some(value) = line.strip_prefix("Content-Length: ") {
                length = value.trim().parse::<usize>().unwrap();
            }
        }
        assert!(length <= 32 * 1024 * 1024);
        let mut bytes = vec![0; length];
        input.read_exact(&mut bytes).unwrap();
        let message: Value = serde_json::from_slice(&bytes).unwrap();
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
            .unwrap();
        writeln!(file, "{message}").unwrap();
        let method = message["method"].as_str().unwrap_or("");
        let id = &message["id"];
        let params = &message["params"];
        // Recreating the marker after a healthy handshake also exercises a short-lived reconnect.
        let args = std::env::args().collect::<Vec<_>>();
        if !method.is_empty()
            && args
                .windows(2)
                .any(|pair| pair[0] == "--crash-marker" && std::path::Path::new(&pair[1]).exists())
        {
            // A real startup crash can leave its diagnostic tail without a line terminator.
            if let Some(pair) = args.windows(2).find(|pair| pair[0] == "--crash-stderr") {
                let mut errors = std::io::stderr().lock();
                errors.write_all(pair[1].as_bytes()).unwrap();
                errors.flush().unwrap();
            }
            std::process::exit(19);
        }
        let result = match method {
            "initialize" => {
                rename_replies = params["initializationOptions"]
                    .get("fixtureRenameReplies")
                    .cloned()
                    .unwrap_or(Value::Null);
                let mut result = json!({"capabilities":{"completionProvider":{},"definitionProvider":true,"referencesProvider":true,"signatureHelpProvider":{},"codeActionProvider":true,"textDocumentSync":{"openClose":true,"change":1,"save":true}}});
                if rename_replies.is_object() {
                    result["capabilities"]["renameProvider"] = json!(true);
                }
                if related_source.is_some() {
                    result["capabilities"]["hoverProvider"] = json!(true);
                }
                // Only this test server interprets fixtureControl. Tests exercise real framed
                // initialization replies; the production host never knows these fixture names.
                if params["capabilities"]["experimental"]["nanobugControl"] == 1 {
                    let marker = params["initializationOptions"]
                        .get("fixtureControl")
                        .cloned()
                        .unwrap_or_else(|| {
                            json!({"version":1,"methods":[
                                "fixture/barrier", "fixture/pending", "fixture/refuse",
                                "fixture/context/documents", "fixture/context/all-documents"
                            ]})
                        });
                    if !marker.is_null() {
                        result["capabilities"]["experimental"] = json!({"nanobugControl":marker});
                    }
                }
                result
            }
            "initialized" => {
                send(
                    json!({"jsonrpc":"2.0","id":"server-config","method":"workspace/configuration","params":{"items":[{"section":"fixture.analysis"}]}}),
                );
                send(json!({"jsonrpc":"2.0","method":"fixture/status","params":{"state":"ready"}}));
                // Native UI acceptance uses the same real transport without a host-only injection API.
                if let Some(pair) = args
                    .windows(2)
                    .find(|pair| pair[0] == "--initial-notifications")
                {
                    let notifications: Vec<Value> =
                        serde_json::from_slice(&std::fs::read(&pair[1]).unwrap()).unwrap();
                    for notification in notifications {
                        send(notification);
                    }
                }
                continue;
            }
            "textDocument/didOpen" | "textDocument/didChange" => {
                let document = &params["textDocument"];
                let source = document["text"]
                    .as_str()
                    .or_else(|| params["contentChanges"][0]["text"].as_str())
                    .unwrap();
                if related_source
                    .as_ref()
                    .is_some_and(|expected| is_related_source(document, expected))
                {
                    related_overlay = Some(source.to_owned());
                }
                send(
                    json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{
                    "uri":document["uri"],"version":document["version"],"diagnostics":[{
                        "range":{"start":{"line":0,"character":0},"end":{"line":0,"character":1}},
                        "severity":2,"message":format!("unsaved:{source}")}]}}),
                );
                continue;
            }
            "textDocument/didClose" => {
                if related_source
                    .as_ref()
                    .is_some_and(|expected| is_related_source(&params["textDocument"], expected))
                {
                    related_overlay = None;
                }
                continue;
            }
            "textDocument/hover" if related_source.is_some() => {
                let source = related_overlay.clone().unwrap_or_else(|| {
                    std::fs::read_to_string(related_source.as_ref().unwrap()).unwrap()
                });
                json!({"contents":{"kind":"plaintext","value":source}})
            }
            "textDocument/completion" => json!([{"label":"fixture-completion"}]),
            "textDocument/rename" if rename_replies.is_object() => rename_replies
                .get(params["newName"].as_str().unwrap_or(""))
                .cloned()
                .unwrap_or(Value::Null),
            // This mode has no declaration model; Base must display only its source-state hover.
            "textDocument/definition" if related_source.is_some() => json!([]),
            "textDocument/definition" => {
                json!({"uri":params["textDocument"]["uri"],"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":1}}})
            }
            "textDocument/references" => {
                if !reference_ignored && args.iter().any(|arg| arg == "--ignore-references-once") {
                    reference_ignored = true;
                    continue;
                }
                // The returned strict UTF-16 coordinate is resolved against the supplied dirty source.
                json!([{"uri":params["textDocument"]["uri"],"range":{"start":{"line":0,"character":2},"end":{"line":0,"character":4}}}])
            }
            "textDocument/signatureHelp" => {
                // UTF-16 spans include one surrogate pair. Signature-local parameter wins over help.
                json!({"signatures":[
                    {"label":"f(🙂, 参数)","parameters":[{"label":[2,4]},{"label":[6,8]}]},
                    {"label":"f(🙂, 参数)","parameters":[{"label":[2,4]},{"label":[6,8]}],"activeParameter":0}
                ],"activeSignature":1,"activeParameter":1})
            }
            "textDocument/codeAction" => {
                // The standard edit remains bound to the requested URI; no command is executed.
                let uri = params["textDocument"]["uri"].as_str().unwrap();
                json!([{"title":"Fix Unicode","edit":{"changes":{
                    (uri):[{"range":{"start":{"line":0,"character":2},"end":{"line":0,"character":4}},"newText":"新🙂"}]
                }}}])
            }
            "fixture/notifications" => {
                // Exercise real server notifications without adding any host-side test-only operation.
                if let Some(notifications) = params["notifications"].as_array() {
                    for notification in notifications {
                        send(notification.clone());
                    }
                }
                Value::Null
            }
            "fixture/fail-with-stderr" => {
                // Keep stderr open after a malformed frame, so only the host's fault teardown
                // releases this unterminated diagnostic. A controlled stop must still suppress it.
                let mut errors = std::io::stderr().lock();
                errors
                    .write_all(params["message"].as_str().unwrap().as_bytes())
                    .unwrap();
                errors.flush().unwrap();
                let mut output = std::io::stdout().lock();
                output
                    .write_all(b"Content-Length: 8\r\n\r\nnot-json")
                    .unwrap();
                output.flush().unwrap();
                continue;
            }
            "fixture/stderr" => {
                // The normal reply is a wire barrier confirming that the pending line was written.
                let mut errors = std::io::stderr().lock();
                errors
                    .write_all(params["message"].as_str().unwrap().as_bytes())
                    .unwrap();
                errors.flush().unwrap();
                Value::Null
            }
            "fixture/pending" => continue,
            "fixture/context/documents" | "fixture/context/all-documents" => {
                // This test backend selects an explicit project member; the host has no language
                // or filename rule and must keep unrelated candidate text private.
                let query = &params["arguments"];
                let target = query["target"]["id"].as_str().unwrap();
                let documents = query["candidates"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|candidate| candidate["id"].as_str())
                    .filter(|id| {
                        method == "fixture/context/all-documents"
                            || *id == target
                            || *id == "related"
                    })
                    .collect::<Vec<_>>();
                json!({"generation":"fixture-graph-1","documents":documents})
            }
            "fixture/refuse" => {
                // A valid per-request refusal must leave the shared framed connection usable.
                let mut error = json!({"code":-32042,"message":"fixture request refused"});
                if args.iter().any(|arg| arg == "--large-control-error") {
                    // Seventy KiB of UTF-8 fits a normal LSP frame but exceeds control's byte budget.
                    error["message"] = json!("🙂".repeat(70 * 1024 / 4));
                }
                if args.iter().any(|arg| arg == "--released-on-refuse") {
                    // This backend retains no adapters. The explicit receipt releases only this
                    // request's imports; plain refusal still communicates no lifetime disposition.
                    error["data"] = json!({"version":1,"imports":"released"});
                }
                send(json!({"jsonrpc":"2.0","id":id,"error":error}));
                continue;
            }
            "fixture/block-with-ready" => {
                // A reply much larger than the native pipe buffer tests the caller's existing deadline.
                // Stop reading stdin only after publishing the request, readiness, and a receipt barrier.
                send(
                    json!({"jsonrpc":"2.0", "id":"blocked-config", "method":"workspace/configuration",
                    "params":{"items":vec![json!({"section":"missing"}); 65_536]}}),
                );
                send(json!({"jsonrpc":"2.0","method":"fixture/status","params":{"state":"ready"}}));
                send(json!({"jsonrpc":"2.0","method":"window/logMessage",
                    "params":{"type":3,"message":"fixture stdin blocked"}}));
                // The host must terminate the process when the bounded readiness operation fails.
                std::thread::sleep(std::time::Duration::from_secs(60));
                continue;
            }
            _ => Value::Null,
        };
        if !id.is_null() && !method.is_empty() {
            send(json!({"jsonrpc":"2.0","id":id,"result":result}));
        }
    }
}
