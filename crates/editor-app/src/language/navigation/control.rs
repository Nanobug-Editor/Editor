//! Declared native control operations borrow the exact connection used by ordinary language features.
use super::*;
use plugin_runtime::plugin_protocol::{
    api::{ErrorCode, Failure},
    language_control::{MAX_CONTROL_BYTES, SourceMetadata},
};

impl LanguageServer {
    /// Execute one negotiated provider control method on this context's existing stdio connection.
    /// The caller supplies a host-authenticated wrapper; only its original arguments use the plugin
    /// schema. Cancellation affects this wait, and retirement invalidates both queued work and replies.
    pub(crate) fn control_request(
        &self,
        method: &str,
        parameters: Value,
        cancellation: &RequestCancellation,
    ) -> anyhow::Result<Value> {
        self.control_request_with_document(method, parameters, None, cancellation)
    }

    /// Synchronize the captured native source and issue control under the same connection lock.
    /// Source text uses ordinary bounded LSP framing; it is never embedded into the control wrapper.
    pub(crate) fn control_request_for(
        &self,
        method: &str,
        parameters: Value,
        document: DocumentLease,
        source: &str,
        cancellation: &RequestCancellation,
    ) -> anyhow::Result<Value> {
        self.control_request_with_document(
            method,
            parameters,
            Some((document, source, None)),
            cancellation,
        )
    }

    /// Native document revisions are sealed by the dispatcher, then paired with actual wire metadata
    /// under the same connection lock that synchronizes source and sends the controlled operation.
    pub(crate) fn control_request_at_version(
        &self,
        method: &str,
        parameters: Value,
        document: DocumentLease,
        source: &str,
        version: &plugin_runtime::plugin_protocol::api::DocumentVersion,
        cancellation: &RequestCancellation,
    ) -> anyhow::Result<Value> {
        self.control_request_with_document(
            method,
            parameters,
            Some((document, source, Some(version))),
            cancellation,
        )
    }

    /// Both document-free maintenance and source-bound preparation share declared control validation.
    fn control_request_with_document(
        &self,
        method: &str,
        mut parameters: Value,
        source: Option<(
            DocumentLease,
            &str,
            Option<&plugin_runtime::plugin_protocol::api::DocumentVersion>,
        )>,
        cancellation: &RequestCancellation,
    ) -> anyhow::Result<Value> {
        cancellation.check()?;
        ensure!(self.is_active(), "Language control owner was retired");
        if self.service.capability("language.control").is_none()
            || !plugin_runtime::plugin_protocol::language::valid_control_method(method)
        {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "Native language control was not negotiated",
            )
            .into());
        }
        let declaration = self
            .service
            .provider
            .control_methods
            .get(method)
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::UnsupportedOperation,
                    "Native language control method is not declared",
                )
            })?;
        declaration.parameters.accepts(&parameters["arguments"])?;
        self.prepare_until_ready()?;
        let mut connection = loop {
            cancellation.check()?;
            ensure!(self.is_active(), "Language control owner was retired");
            match self.connection.try_lock() {
                Ok(connection) => break connection,
                Err(std::sync::TryLockError::WouldBlock) => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(error) => {
                    return Err(anyhow!("Language control connection lock failed: {error}"));
                }
            }
        };
        let response = (|| {
            let connection = connection
                .as_mut()
                .context("Language context connection is unavailable")?;
            if let Some((document, source, version)) = source {
                connection.bind_document(&document)?;
                let wire = connection.sync_document(document.uri.clone(), source.to_owned())?;
                if let Some(version) = version {
                    seal_sources(
                        connection,
                        &mut parameters,
                        SourceMetadata {
                            document: version.clone(),
                            uri: wire,
                            version: connection
                                .diagnostics
                                .document_version(document.uri.as_str())
                                .context("Language source was not synchronized")?,
                        },
                    )?;
                }
            }
            if serde_json::to_vec(&parameters)?.len() > MAX_CONTROL_BYTES {
                return Err(Failure::new(
                    ErrorCode::LimitExceeded,
                    "Language control metadata exceeds 64 KiB",
                )
                .into());
            }
            connection.request_cancellable(method, parameters, cancellation)
        })();
        self.connection_failed(method, &response, &mut connection);
        cancellation.check()?;
        ensure!(self.is_active(), "Language control owner was retired");
        let response = response?;
        ensure!(
            serde_json::to_vec(&response)?.len() <= MAX_CONTROL_BYTES,
            "Language control response exceeds 64 KiB"
        );
        declaration.result.accepts(&response)?;
        Ok(response)
    }
}

/// Pair the native source vector with this exact connection while its synchronization lock is held.
/// A competing consumer may have changed an overlay since graph capture; reject before analysis
/// instead of delivering a vector whose public revision and transport version describe different text.
fn seal_sources(
    connection: &LanguageServerConnection,
    parameters: &mut Value,
    target: SourceMetadata,
) -> anyhow::Result<()> {
    if parameters["_nanobug"]["sources"].is_null() {
        parameters["_nanobug"]["sources"] = serde_json::to_value([target])?;
        return Ok(());
    }
    let sources: Vec<SourceMetadata> =
        serde_json::from_value(parameters["_nanobug"]["sources"].clone())?;
    let mut ids = std::collections::BTreeSet::new();
    if sources.is_empty()
        || sources.len() > plugin_runtime::plugin_protocol::language_context::MAX_CANDIDATES
        || !sources.contains(&target)
        || sources.iter().any(|source| {
            let Ok(uri) = source.uri.parse::<Uri>() else {
                return true;
            };
            let logical = connection.logical_uri(&uri);
            !ids.insert(&source.document.id)
                || source.version < 1
                || connection.wire_uri(&logical) != uri
                || connection.diagnostics.document_version(logical.as_str()) != Some(source.version)
        })
    {
        return Err(Failure::new(
            ErrorCode::StaleRevision,
            "Control source vector changed before dispatch",
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use plugin_runtime::{
        DemandReason, LanguageRole, Manager, Package,
        plugin_protocol::{Environment, language::DocumentContext},
    };
    use std::collections::BTreeMap;

    /// Real framed requests verify shared startup, per-source language IDs and standard cancellation.
    #[test]
    #[ignore = "build cargo build -p plugin-runtime --example lsp_fixture first"]
    fn native_control_and_document_aliases_share_one_connection() {
        let workspace = tempfile::tempdir().unwrap();
        let profile = tempfile::tempdir().unwrap();
        let executable = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("examples/lsp_fixture.exe")
            .canonicalize()
            .expect("build the real native fixture first");
        let log = workspace.path().join("wire.jsonl");
        let manifest = json!({
            "id":"native-context-control","name":"Native context control","version":"0.1.0","protocol":7,
            "api":{"base":"^1","required":{"language.lsp":"^1.5","language.control":"^1","process":"^1"}},
            "contributions":"plugin.toml","permissions":["process.service.analysis"],"storage_limit":1024,
            "services":{"analysis":{"program":executable,"args":[log]}},
            "language_servers":[{"id":"analysis","language":"unfamiliar","languages":["different"],"service":"analysis",
                "client_experimental":{"nanobugControl":999},
                "control_methods":{
                    "fixture/barrier":{"parameters":{"type":"null"},"result":{"type":"null"}},
                    "fixture/pending":{"parameters":{"type":"null"},"result":{"type":"null"}}
                    ,"fixture/refuse":{"parameters":{"type":"null"},"result":{"type":"null"}}
                }}]
        });
        let package = Package::from_files(BTreeMap::from([
            ("manifest.json".into(),serde_json::to_vec(&manifest).unwrap()),
            ("plugin.toml".into(),b"[plugin]\nid='native-context-control'\nname='Native context control'\nversion='0.1.0'\nhost_version='^0.1'\n".to_vec())
        ])).unwrap();
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
        let token = manager.availability("native-context-control").unwrap();
        let document = DocumentContext {
            path: "one.novel".into(),
            generation: token.configuration_generation(),
        };
        let wait = manager
            .begin_demand(
                &token,
                DemandReason::LanguageDocument {
                    provider: "native-context-control/analysis".into(),
                    role: LanguageRole::Lsp,
                    document: document.clone(),
                },
                10_000,
            )
            .unwrap();
        let prepared = manager.take_demand_preparation().unwrap().run();
        manager.commit_demand(prepared).unwrap();
        assert!(matches!(
            wait.status(),
            plugin_runtime::RequestUpdate::Completed { result: Ok(_) }
        ));
        let service = manager
            .language_document_service("native-context-control/analysis", &document)
            .unwrap()
            .unwrap();
        let server = Arc::new(LanguageServer::from_service(service).unwrap());
        assert!(
            server
                .control_request(
                    "workspace/applyEdit",
                    json!({"arguments":null}),
                    &RequestCancellation::default()
                )
                .is_err()
        );
        assert!(
            !log.exists(),
            "reserved requests must fail before starting the native server"
        );
        let source = "x".repeat(70_000);
        for (path, language) in [("one.novel", "unfamiliar"), ("two.novel", "different")] {
            let uri = file_uri(&workspace.path().join(path)).unwrap();
            let document = server.open_document_for_language(uri, language);
            server
                .control_request_for(
                    "fixture/barrier",
                    json!({"arguments":null}),
                    document,
                    &source,
                    &RequestCancellation::default(),
                )
                .unwrap();
        }
        let native_version = plugin_runtime::plugin_protocol::api::DocumentVersion {
            id: "native-source-incarnation".into(),
            path: "one.novel".into(),
            revision: 37,
        };
        let uri = file_uri(&workspace.path().join("one.novel")).unwrap();
        server
            .control_request_at_version(
                "fixture/barrier",
                json!({"arguments":null,"_nanobug":{"consumer":"native-fixture"}}),
                server.open_document(uri),
                &source,
                &native_version,
                &RequestCancellation::default(),
            )
            .unwrap();
        let uri = file_uri(&workspace.path().join("one.novel")).unwrap();
        let stale = server.control_request_at_version(
            "fixture/barrier",
            json!({"arguments":null,"_nanobug":{"sources":[{
                "document":native_version,"uri":uri.as_str(),"version":37
            }]}}),
            server.open_document(uri),
            &source,
            &native_version,
            &RequestCancellation::default(),
        );
        assert_eq!(
            stale.unwrap_err().downcast_ref::<Failure>().unwrap().code,
            ErrorCode::StaleRevision,
            "a native document revision cannot be supplied as its connection wire version"
        );
        assert_eq!(
            server
                .control_request(
                    "fixture/barrier",
                    json!({"arguments":null}),
                    &RequestCancellation::default()
                )
                .unwrap(),
            Value::Null
        );
        let cancellation = RequestCancellation::default();
        let worker_cancel = cancellation.clone();
        let worker = server.clone();
        let pending = std::thread::spawn(move || {
            worker.control_request("fixture/pending", json!({"arguments":null}), &worker_cancel)
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while !std::fs::read_to_string(&log)
            .unwrap_or_default()
            .contains("fixture/pending")
        {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        cancellation.cancel();
        assert!(pending.join().unwrap().is_err());
        assert!(
            server
                .control_request(
                    "fixture/refuse",
                    json!({"arguments":null}),
                    &RequestCancellation::default()
                )
                .is_err()
        );
        server
            .control_request(
                "fixture/barrier",
                json!({"arguments":null}),
                &RequestCancellation::default(),
            )
            .unwrap();
        let frames = std::fs::read_to_string(&log)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            frames
                .iter()
                .filter(|frame| frame["method"] == "initialize")
                .count(),
            1
        );
        assert_eq!(
            frames
                .iter()
                .find(|frame| frame["method"] == "initialize")
                .unwrap()["params"]["capabilities"]["experimental"]["nanobugControl"],
            1,
            "the reserved control marker must come from the negotiated host capability"
        );
        let languages = frames
            .iter()
            .filter(|frame| frame["method"] == "textDocument/didOpen")
            .map(|frame| {
                frame["params"]["textDocument"]["languageId"]
                    .as_str()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(languages, vec!["unfamiliar", "different"]);
        let metadata = frames
            .iter()
            .find(|frame| {
                frame["method"] == "fixture/barrier"
                    && frame["params"]["_nanobug"]["consumer"] == "native-fixture"
            })
            .expect("a native source-bound control must retain sealed metadata");
        assert_eq!(
            metadata["params"]["_nanobug"]["sources"][0]["document"],
            serde_json::to_value(&native_version).unwrap()
        );
        assert_eq!(
            metadata["params"]["_nanobug"]["sources"][0]["version"], 1,
            "native revision 37 must not be mistaken for internal LSP version 1"
        );
        assert!(
            metadata["params"]["_nanobug"]["sources"][0]["uri"]
                .as_str()
                .unwrap()
                .ends_with("/one.novel")
        );
        let opened = frames
            .iter()
            .position(|frame| frame["method"] == "textDocument/didOpen")
            .unwrap();
        let control = frames
            .iter()
            .position(|frame| frame["method"] == "fixture/barrier")
            .unwrap();
        assert!(
            opened < control,
            "first control must synchronize its original source before the native request"
        );
        assert_eq!(
            frames[opened]["params"]["textDocument"]["text"]
                .as_str()
                .unwrap(),
            source,
            "source larger than the 64 KiB control limit must use the separate LSP document channel"
        );
        assert!(
            frames
                .iter()
                .any(|frame| frame["method"] == "$/cancelRequest")
        );
        manager.disable("native-context-control").unwrap();
        assert!(!server.is_active());
    }
}
