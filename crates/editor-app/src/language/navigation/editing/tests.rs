//! Standard wire proposals are validated whole before the native revision-gated transaction.
use super::*;

/// A versioned or null-version TextDocumentEdit produces the same paired plan as changes.
#[test]
fn standard_rename_forms_preserve_one_complete_document_plan() {
    let uri: Uri = "file:///sample.novel".parse().unwrap();
    let changes = json!([
        {"range":{"start":{"line":0,"character":1},"end":{"line":0,"character":4}},"newText":"renamed"},
        {"range":{"start":{"line":0,"character":8},"end":{"line":0,"character":11}},"newText":"renamed"}
    ]);
    for response in [
        json!({"changes":{uri.as_str():changes.clone()}}),
        json!({"documentChanges":[{"textDocument":{"uri":uri,"version":7},"edits":changes.clone()}]}),
        json!({"documentChanges":[{"textDocument":{"uri":uri,"version":null},"edits":changes}]}),
    ] {
        let edits = document_edits(response, &uri, 7, Clone::clone).unwrap();
        assert_eq!(edits.len(), 2);
        let merged = merge_edits("<one>x</one>", &edits).unwrap().unwrap();
        assert_eq!(merged.new_text, "renamed>x</renamed");
    }
}

/// Invalid metadata in a later edit cannot authorize a valid prefix or bypass confirmation.
#[test]
fn rename_rejects_stale_foreign_resource_mixed_and_annotated_plans() {
    let uri: Uri = "file:///sample.novel".parse().unwrap();
    let range = json!({"start":{"line":0,"character":1},"end":{"line":0,"character":4}});
    let good = json!({"textDocument":{"uri":uri,"version":7},"edits":[{"range":range,"newText":"renamed"}]});
    let mut stale = good.clone();
    stale["textDocument"]["version"] = json!(6);
    let mut foreign = good.clone();
    foreign["textDocument"]["uri"] = json!("file:///other.novel");
    let mut annotated = good.clone();
    annotated["edits"][0]["annotationId"] = json!("needs-confirmation");
    for response in [
        json!({"documentChanges":[good.clone(),stale]}),
        json!({"documentChanges":[good.clone(),foreign]}),
        json!({"documentChanges":[good.clone(),{"kind":"delete","uri":uri}]}),
        json!({"changes":{},"documentChanges":[good.clone()]}),
        json!({"documentChanges":[good,annotated]}),
        json!({"changes":{uri.as_str():[{"range":range,"newText":"renamed","annotationId":"needs-confirmation"}]}}),
    ] {
        assert!(document_edits(response, &uri, 7, Clone::clone).is_err());
    }
}

/// Safe metadata does not make overlapping text ranges safe; the whole proposal is still refused.
#[test]
fn versioned_rename_overlap_still_fails_before_native_application() {
    let uri: Uri = "file:///sample.novel".parse().unwrap();
    let response = json!({"documentChanges":[{"textDocument":{"uri":uri,"version":7},"edits":[
        {"range":{"start":{"line":0,"character":1},"end":{"line":0,"character":4}},"newText":"renamed"},
        {"range":{"start":{"line":0,"character":3},"end":{"line":0,"character":5}},"newText":"bad suffix"}
    ]}]});
    let edits = document_edits(response, &uri, 7, Clone::clone).unwrap();
    assert!(merge_edits("<one>x</one>", &edits).is_err());
}

/// A real Package owns the declared service, initialization data and process permission. The fixture
/// only returns the supplied standard replies; no test-only host operation or language rule is used.
fn packaged_rename_server(
    root: &Path,
    replies: Value,
) -> (plugin_runtime::Manager, LanguageServer) {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/lsp_fixture.exe")
        .canonicalize()
        .expect("build plugin-runtime --example lsp_fixture first");
    let manifest = json!({
        "id":"complete-rename", "name":"Complete rename", "version":"0.1.0", "protocol":7,
        "api":{"base":"^1","required":{"language.lsp":"^1.5","process":"^1",
            "language.editing":"^1","editor.edit":"^1.1","workspace.files":"^1.4"}},
        "contributions":"plugin.toml", "storage_limit":1024,
        "permissions":["process.service.analysis","editor.read","editor.write","workspace.read"],
        "services":{"analysis":{"program":executable,"args":[root.join("wire.jsonl")]}},
        "language_servers":[{"id":"analysis","language":"unfamiliar","service":"analysis",
            "editing":true,"initialization_options":{"fixtureRenameReplies":replies}}]
    });
    let package = plugin_runtime::Package::from_files(std::collections::BTreeMap::from([
        ("manifest.json".into(), serde_json::to_vec(&manifest).unwrap()),
        (
            "plugin.toml".into(),
            b"[plugin]\nid='complete-rename'\nname='Complete rename'\nversion='0.1.0'\nhost_version='^0.1'\n".to_vec(),
        ),
    ]))
    .unwrap();
    let mut manager = plugin_runtime::Manager::open(
        root.join(".profile"),
        plugin_runtime::plugin_protocol::Environment {
            workspace: root.display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    crate::extensions::demand_plugin_fixture(
        &mut manager,
        "complete-rename",
        plugin_runtime::DemandReason::Language {
            provider: "complete-rename/analysis".into(),
            role: plugin_runtime::LanguageRole::Lsp,
        },
    )
    .unwrap();
    let service = manager.language_services()["complete-rename/analysis"]
        .as_ref()
        .unwrap()
        .clone();
    (manager, LanguageServer::from_service(service).unwrap())
}

/// A valid leading document never escapes when the complete standard reply has an illegal tail.
/// Positive two-target responses prove the test reached the workspace decoder, rather than failing
/// from missing capabilities, a foreign second target or an unprepared fixture process.
#[test]
#[ignore = "build cargo build -p plugin-runtime --example lsp_fixture first; uses real Package/stdio"]
fn packaged_rename_rejects_illegal_tail_without_exposing_a_valid_prefix() {
    let source = "one\r\n";
    for case in [
        "resource",
        "annotation",
        "stale",
        "command",
        "mixed",
        "duplicate",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let first_path = root.join("first.unfamiliar");
        let second_path = root.join("second.unfamiliar");
        std::fs::write(&first_path, source).unwrap();
        std::fs::write(&second_path, source).unwrap();
        let first = file_uri(&first_path).unwrap();
        let second = file_uri(&second_path).unwrap();
        let change = |uri: &Uri| {
            json!({
                "textDocument":{"uri":uri,"version":null},
                "edits":[{"range":{"start":{"line":0,"character":0},
                    "end":{"line":0,"character":3}},"newText":"renamed"}]
            })
        };
        let leading = change(&first);
        let trailing = change(&second);
        let valid = json!({"documentChanges":[leading.clone(), trailing.clone()]});
        let mut invalid = valid.clone();
        match case {
            "resource" => invalid["documentChanges"][1] = json!({"kind":"delete","uri":second}),
            "annotation" => {
                invalid["documentChanges"][1]["edits"][0]["annotationId"] = json!("consent")
            }
            "stale" => invalid["documentChanges"][1]["textDocument"]["version"] = json!(0),
            "command" => invalid["command"] = json!({"command":"fixture/side-effect"}),
            "mixed" => invalid["changes"] = json!({}),
            "duplicate" => invalid["documentChanges"]
                .as_array_mut()
                .unwrap()
                .push(trailing),
            _ => unreachable!(),
        }
        let (mut manager, server) =
            packaged_rename_server(&root, json!({"valid":valid,"invalid":invalid}));
        let first_lease = server.open_document(first.clone());
        let second_lease = server.open_document(second.clone());
        let related = || vec![(second_lease.clone(), source.to_owned())];
        let complete = server
            .rename_for(
                first_lease.clone(),
                source.into(),
                Position::new(0, 1),
                "valid".into(),
                related(),
            )
            .unwrap();
        assert_eq!(
            complete.len(),
            2,
            "{case}: the complete valid proposal must survive"
        );
        assert_eq!(complete[0].uri, first);
        assert_eq!(complete[1].uri, second);
        for target in complete {
            assert_eq!(
                merge_edits(source, &target.edits)
                    .unwrap()
                    .unwrap()
                    .new_text,
                "renamed"
            );
        }
        let rejected = server.rename_for(
            first_lease,
            source.into(),
            Position::new(0, 1),
            "invalid".into(),
            related(),
        );
        assert!(
            rejected.is_err(),
            "{case}: an illegal tail must reject the whole response"
        );
        assert!(
            manager.take_editor_requests().is_empty(),
            "{case}: no accepted prefix may become a plan"
        );
        assert_eq!(std::fs::read_to_string(&first_path).unwrap(), source);
        assert_eq!(std::fs::read_to_string(&second_path).unwrap(), source);
        let wire = std::fs::read_to_string(root.join("wire.jsonl")).unwrap();
        let renames = wire
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|frame| frame["method"] == "textDocument/rename")
            .collect::<Vec<_>>();
        assert_eq!(
            renames.len(),
            2,
            "{case}: both requests must cross the actual framed pipe"
        );
        assert_eq!(renames[1]["params"]["newName"], "invalid");
        manager.disable("complete-rename").unwrap();
    }
}
