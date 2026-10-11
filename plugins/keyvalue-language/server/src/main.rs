//! A bounded stdio key-value language service owns domain policy and speaks standard LSP JSON-RPC.
//! The service does not read project files, spawn tools, or borrow an entire compiler/SDK runtime.
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{BufRead, Read, Write},
};

/// Replies use exact request IDs and a flushed standard Content-Length frame.
fn send(value: Value) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(&value)?;
    let mut output = std::io::stdout().lock();
    write!(output, "Content-Length: {}\r\n\r\n", bytes.len())?;
    output.write_all(&bytes)?;
    output.flush()
}

/// Reject malformed or oversized native input rather than allocating an unbounded frame.
fn read(input: &mut impl BufRead) -> std::io::Result<Option<Value>> {
    let mut length = None;
    let mut headers = 0;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        headers += line.len();
        if headers > 4096 {
            return Err(std::io::Error::other("Oversized LSP headers"));
        }
        if line == "\r\n" {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let length = length
        .filter(|length| *length <= 2 * 1024 * 1024)
        .ok_or_else(|| std::io::Error::other("Invalid LSP frame length"))?;
    let mut bytes = vec![0; length];
    input.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(std::io::Error::other)
}

/// Native positions are UTF-16; the business parser still works with immutable UTF-8 slices.
fn offset(text: &str, position: &Value) -> Option<usize> {
    let line = position["line"].as_u64()? as usize;
    let character = position["character"].as_u64()? as usize;
    let mut start = 0;
    for _ in 0..line {
        start += text[start..].find('\n')? + 1;
    }
    let mut units = 0;
    for (byte, ch) in text[start..].char_indices() {
        if units == character {
            return Some(start + byte);
        }
        if ch == '\r' || ch == '\n' {
            return None;
        }
        units += ch.len_utf16();
        if units > character {
            return None;
        }
    }
    (units == character).then_some(text.len())
}

/// Produce standard UTF-16 ranges from validated token byte offsets, preserving Unicode columns.
fn position(text: &str, offset: usize) -> Value {
    let prefix = &text[..offset];
    json!({"line":prefix.bytes().filter(|byte| *byte == b'\n').count(),
        "character":prefix.rsplit('\n').next().unwrap().encode_utf16().count()})
}

/// Identifier policy is independent from the editor; symbols may contain Unicode letters.
fn token(text: &str, cursor: usize) -> Option<&str> {
    let start = text[..cursor]
        .char_indices()
        .rev()
        .take_while(|(_, ch)| ch.is_alphanumeric() || *ch == '_')
        .last()
        .map_or(cursor, |(byte, _)| byte);
    let end = text[cursor..]
        .char_indices()
        .take_while(|(_, ch)| ch.is_alphanumeric() || *ch == '_')
        .last()
        .map_or(cursor, |(byte, ch)| cursor + byte + ch.len_utf8());
    (start < end).then_some(&text[start..end])
}

/// Standard requests return plugin-owned metadata and edits; no server command bypasses native preview.
fn feature(method: &str, params: &Value, documents: &BTreeMap<String, String>) -> Value {
    let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
    let Some(text) = documents.get(uri) else {
        return Value::Null;
    };
    match method {
        "textDocument/references" => {
            let Some(cursor) = offset(text, &params["position"]) else {
                return Value::Null;
            };
            let locations = token(text, cursor)
                .map(|name| {
                    text.match_indices(name)
                        .filter(|(start, _)| {
                            text[..*start]
                                .chars()
                                .next_back()
                                .is_none_or(|ch| !ch.is_alphanumeric() && ch != '_')
                                && text[*start + name.len()..]
                                    .chars()
                                    .next()
                                    .is_none_or(|ch| !ch.is_alphanumeric() && ch != '_')
                        })
                        .take(256)
                        .map(|(start, _)| {
                            json!({"uri":uri,"range":{
                "start":position(text,start),"end":position(text,start + name.len())}})
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            json!(locations)
        }
        "textDocument/codeAction" => {
            let actions = text.match_indices("port=oops").next().map(|(start, _)| {
                let start = start + 5;
                json!({"title":"Use default port / 使用默认端口","kind":"quickfix","edit":{"changes":{
                    (uri):[{"range":{"start":position(text,start),"end":position(text,start+4)},"newText":"8080"}]
                }}})
            }).into_iter().collect::<Vec<_>>();
            json!(actions)
        }
        "textDocument/signatureHelp" => {
            let Some(cursor) = offset(text, &params["position"]) else {
                return Value::Null;
            };
            let prefix = &text[..cursor];
            let Some(open) = prefix
                .rfind("setting(")
                .filter(|start| !prefix[*start..].contains(')'))
            else {
                return Value::Null;
            };
            let parameter = prefix[open + 8..]
                .bytes()
                .filter(|byte| *byte == b',')
                .count()
                .min(1);
            json!({"signatures":[{"label":"setting(key, value)","documentation":"Set a configuration value / 设置配置值",
                "parameters":[{"label":"key"},{"label":"value"}],"activeParameter":parameter}],"activeSignature":0,"activeParameter":parameter})
        }
        "textDocument/completion" => {
            json!([{"label":"port","insertText":"port"},{"label":"setting","insertText":"setting"}])
        }
        _ => Value::Null,
    }
}

/// One process keeps at most 128 synchronized snapshots and handles cancellation as a normal notification.
fn main() -> std::io::Result<()> {
    let mut input = std::io::BufReader::new(std::io::stdin().lock());
    let mut documents = BTreeMap::<String, String>::new();
    while let Some(message) = read(&mut input)? {
        let method = message["method"].as_str().unwrap_or("");
        let params = &message["params"];
        let result = match method {
            "initialize" => {
                json!({"capabilities":{"positionEncoding":"utf-16","textDocumentSync":{"openClose":true,"change":1},
                "referencesProvider":true,"codeActionProvider":true,"signatureHelpProvider":{"triggerCharacters":["(",","]},"completionProvider":{}}})
            }
            "textDocument/didOpen" | "textDocument/didChange" => {
                let uri = params["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or("")
                    .to_owned();
                let source = params["textDocument"]["text"]
                    .as_str()
                    .or_else(|| params["contentChanges"][0]["text"].as_str())
                    .unwrap_or("");
                if source.len() <= 1024 * 1024
                    && (documents.contains_key(&uri) || documents.len() < 128)
                {
                    documents.insert(uri, source.into());
                }
                continue;
            }
            "textDocument/didClose" => {
                if let Some(uri) = params["textDocument"]["uri"].as_str() {
                    documents.remove(uri);
                }
                continue;
            }
            "exit" => return Ok(()),
            "initialized" | "$/cancelRequest" => continue,
            "shutdown" => Value::Null,
            _ => feature(method, params, &documents),
        };
        if !message["id"].is_null() {
            send(json!({"jsonrpc":"2.0","id":message["id"],"result":result}))?;
        }
    }
    Ok(())
}
