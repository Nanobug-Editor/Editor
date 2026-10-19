//! Independent original guest consumers share code while retaining separately revocable authority.
//! Real stdio barriers prove worker continuity; the fixture deliberately does not execute adapters.
use plugin_runtime::{
    DemandReason, LanguageRole, LanguageService, Manager, NativeModuleLease, Package,
    ServiceProcess,
    plugin_protocol::{Environment, api, language::DocumentContext, service},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{ChildStdin, ChildStdout},
    sync::Arc,
    time::{Duration, Instant},
};
#[path = "../support/demand.rs"]
mod demand;
#[path = "consumers/release.rs"]
mod release;

const SOURCE: &str = "unfamiliar-code-owner";
const ENGINE: &str = "unfamiliar-native-engine";
const CLIENT_A: &str = "independent-client-a";
const CLIENT_B: &str = "independent-client-b";
const DENIED: &str = "independent-denied-client";

/// Neither a source/engine identity nor an owning host is used in place of two original guests.
#[test]
#[ignore = "build native-consumers through host --plugin-package, set NANOBUG_NATIVE_CONSUMERS_PACKAGE, and build lsp_fixture"]
fn original_consumers_revoke_independently_and_both_module_ends_reap_actual_workers() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    for directory in ["first", "second"] {
        std::fs::create_dir(workspace.path().join(directory)).unwrap();
        std::fs::write(
            workspace.path().join(directory).join("source.novel"),
            "saved source",
        )
        .unwrap();
    }
    let mut manager = Manager::open(
        profile.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    let component = Package::read(&input("NANOBUG_NATIVE_CONSUMERS_PACKAGE"))
        .unwrap()
        .files["plugin.wasm"]
        .clone();
    // Wire instrumentation stays in the isolated host profile rather than the source workspace.
    for package in packages(&component, &profile.path().join("wire.jsonl")) {
        if package.manifest.id == DENIED {
            // Method authority is always declared. Declining the original consumer's native
            // consent is rejected before a component can acquire a reference or loaded lease.
            let mut grants = package.manifest.permissions.clone();
            grants.remove("native.modules");
            let error = manager.install(&package, grants).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("Permission confirmation required")
            );
            assert!(manager.instance_id(DENIED).is_none());
            continue;
        }
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
    }
    demand::ready(
        &mut manager,
        SOURCE,
        DemandReason::Service("novel.module".into()),
    );
    let source_owner = manager.instance_id(SOURCE).unwrap().to_owned();
    let original = prepare(&mut manager, "first/source.novel");
    let sibling = prepare(&mut manager, "second/source.novel");
    assert!(!Arc::ptr_eq(&original, &sibling));
    let mut workers = vec![Worker::spawn(sibling.clone())];
    workers[0].barrier();
    for client in [CLIENT_A, CLIENT_B] {
        demand::ready(&mut manager, client, DemandReason::Command("run".into()));
    }
    let first_owner = manager.instance_id(CLIENT_A).unwrap().to_owned();
    let second_owner = manager.instance_id(CLIENT_B).unwrap().to_owned();
    assert_ne!(first_owner, second_owner);
    let mut leases = BTreeMap::<String, Arc<NativeModuleLease>>::new();
    for client in [CLIENT_A, CLIENT_B] {
        assert_eq!(
            query(&mut manager, client, &original, &mut workers, &mut leases)["state"],
            "complete"
        );
    }
    assert_eq!(leases.len(), 2);
    assert!(leases[&first_owner].is_live() && leases[&second_owner].is_live());
    // A repeated rejected request owns only its new reservation. Its release cannot erase the
    // same caller's already committed code use, which must still protect this worker below.
    let mut repeated = leases[&second_owner].reserve(&original).unwrap();
    repeated.dispatch().unwrap();
    repeated.release();
    let mut cancelled_before_dispatch = leases[&first_owner].reserve(&original).unwrap();
    let mut revoked_after_dispatch = leases[&first_owner].reserve(&original).unwrap();
    revoked_after_dispatch.dispatch().unwrap();
    assert_eq!(
        original.process_count(),
        1,
        "a second consumer reuses the existing worker"
    );
    assert!(manager.instance_id(DENIED).is_none());

    manager.disable(CLIENT_A).unwrap();
    assert!(
        cancelled_before_dispatch.dispatch().is_err(),
        "an inert reservation cannot revive its original consumer after retirement"
    );
    assert!(
        revoked_after_dispatch.dispatch().is_err(),
        "reusing a dispatched reservation must recheck the retired original authority"
    );
    revoked_after_dispatch.commit();
    assert!(!leases[&first_owner].is_live());
    assert!(leases[&second_owner].is_live());
    assert!(original.is_active());
    assert_eq!(
        original.retired_control_consumers(),
        vec![first_owner.clone()]
    );
    // Only this host dispatcher sends lifecycle metadata on its already-owned pipe. The guest
    // never declares or invokes the reserved notification; acknowledge after the real write.
    worker(&mut workers, &original).notify_retired(std::slice::from_ref(&first_owner));
    original.acknowledge_control_consumers(&[first_owner]);
    assert!(original.retired_control_consumers().is_empty());
    worker(&mut workers, &original).barrier();
    assert_eq!(
        query(&mut manager, CLIENT_B, &original, &mut workers, &mut leases)["state"],
        "complete"
    );
    assert_eq!(original.process_count(), 1);

    manager.disable(SOURCE).unwrap();
    assert!(!original.is_active());
    assert_eq!(original.process_count(), 0);
    worker(&mut workers, &original).assert_closed();
    assert!(
        sibling.is_active(),
        "a context without this export keeps its native worker"
    );
    worker(&mut workers, &sibling).barrier();
    manager.enable(SOURCE).unwrap();
    demand::ready(
        &mut manager,
        SOURCE,
        DemandReason::Service("novel.module".into()),
    );
    assert_ne!(manager.instance_id(SOURCE), Some(source_owner.as_str()));
    let replacement = prepare(&mut manager, "first/source.novel");
    assert!(!Arc::ptr_eq(&original, &replacement));
    assert_eq!(
        query(
            &mut manager,
            CLIENT_B,
            &replacement,
            &mut workers,
            &mut leases
        )["state"],
        "complete"
    );
    worker(&mut workers, &replacement).barrier();

    let publisher = manager.instance_id(SOURCE).unwrap().to_owned();
    let mut retired_recipient = leases[&second_owner].reserve(&replacement).unwrap();
    manager.disable(ENGINE).unwrap();
    assert!(
        retired_recipient.dispatch().is_err(),
        "a reserved import cannot retarget a retired recipient"
    );
    for plan in [&replacement, &sibling] {
        assert!(!plan.is_active());
        assert_eq!(plan.process_count(), 0);
        worker(&mut workers, plan).assert_closed();
    }
    assert_eq!(manager.instance_id(SOURCE), Some(publisher.as_str()));
    assert_eq!(manager.instance_id(CLIENT_B), Some(second_owner.as_str()));
    assert!(
        !leases[&second_owner].is_live(),
        "recipient retirement also seals its loaded code"
    );
    for directory in ["first", "second"] {
        assert_eq!(
            std::fs::read_to_string(workspace.path().join(directory).join("source.novel")).unwrap(),
            "saved source"
        );
    }
    manager.shutdown();
}

/// One closed method shape is shared by the actual public consumer and receiving engine.
fn attach_method() -> service::Method {
    let mut method = plugin_runtime::plugin_protocol::language_control::binding_prepare_method();
    method.result = service::Schema::Null;
    method
}

/// All participants consume one independently built SDK component, without host path dependencies.
fn packages(component: &[u8], wire: &Path) -> Vec<Package> {
    let method = attach_method();
    let module_method = json!({"parameters":{"type":"null"},"result":{"type":"null"}});
    let base = json!({"version":"0.1.0","protocol":7,"component":"plugin.wasm","storage_limit":16384,
        "api":{"base":"^1","required":{"plugin.services":"^1.1","language.control":"^1","language.modules":"^1","language.lsp":"^1.5","process":"^1"}}});
    let code = b"module.exports = { unknownFixture: true };\n";
    let digest = format!("{:x}", Sha256::digest(code));
    let mut source = base.clone();
    source["id"] = json!(SOURCE);
    source["name"] = json!("Unknown code publisher");
    source["permissions"] = json!(["native.modules"]);
    source["module_exports"] = json!({"adapter":{"root":"modules/adapter","entry":"index.cjs","sha256":digest,
        "files":{"index.cjs":digest},"abi":"unfamiliar.engine/1","engine":{"unfamiliar":"^1"}}});
    source["plugin_services"] = json!({"provides":{"novel.module":{"version":"1.0.0","methods":{"identity":module_method}}}});
    let publisher = package(
        source,
        component,
        BTreeMap::from([("modules/adapter/index.cjs".into(), code.to_vec())]),
    );
    let mut engine = base.clone();
    engine["id"] = json!(ENGINE);
    engine["name"] = json!("Unknown receiving native engine");
    engine["permissions"] = json!([
        "native.modules",
        "editor.read",
        "workspace.read",
        "services.call",
        "process.service.analysis"
    ]);
    let native = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/examples/lsp_fixture.exe")
        .canonicalize()
        .unwrap();
    engine["services"] = json!({"analysis":{"program":native,"args":[wire]}});
    engine["plugin_services"] = json!({"provides":{"novel.engine":{"version":"1.0.0","methods":{"attach":method}}},
        "requires":{"novel.module":{"version":"^1","methods":{"identity":module_method}}}});
    engine["language_servers"] = json!([{"id":"analysis","language":"unfamiliar-native-engine","service":"analysis","hook":true,
        "control_methods":{"novel/attach":{"parameters":{"type":"null"},"result":{"type":"null"},"permissions":["native.modules","editor.read","workspace.read"]}}}]);
    let mut packages = vec![publisher, package(engine, component, BTreeMap::new())];
    for id in [CLIENT_A, CLIENT_B, DENIED] {
        let mut consumer = base.clone();
        consumer["id"] = json!(id);
        consumer["name"] = json!(id);
        consumer["permissions"] = json!([
            "services.call",
            "editor.read",
            "workspace.read",
            "native.modules"
        ]);
        consumer["commands"] = json!([{"id":"run","title":"Borrow the unknown native module"}]);
        consumer["plugin_services"] =
            json!({"requires":{"novel.engine":{"version":"^1","methods":{"attach":method}}}});
        packages.push(package(consumer, component, BTreeMap::new()));
    }
    packages
}

/// Package admission checks each role's real manifest and complete bytes before any activation.
fn package(manifest: Value, component: &[u8], mut files: BTreeMap<String, Vec<u8>>) -> Package {
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    files.insert("plugin.wasm".into(), component.to_vec());
    Package::from_files(files).unwrap()
}

/// The engine's public language hook gives distinct native project contexts and exact document seals.
fn prepare(manager: &mut Manager, path: &str) -> Arc<LanguageService> {
    let availability = manager.availability(ENGINE).unwrap();
    let document = DocumentContext {
        path: path.into(),
        generation: availability.configuration_generation(),
    };
    demand::ready(
        manager,
        ENGINE,
        DemandReason::LanguageDocument {
            provider: format!("{ENGINE}/analysis"),
            role: LanguageRole::Lsp,
            document: document.clone(),
        },
    );
    manager
        .language_document_service(&format!("{ENGINE}/analysis"), &document)
        .unwrap()
        .unwrap()
}

/// Drive the ordinary broker and native EditorRequest seam, retaining only trusted host leases.
fn query(
    manager: &mut Manager,
    client: &str,
    plan: &Arc<LanguageService>,
    workers: &mut Vec<Worker>,
    leases: &mut BTreeMap<String, Arc<NativeModuleLease>>,
) -> Value {
    manager
        .invoke_command(
            client,
            "run",
            json!({"document":{"path":"first/source.novel","generation":0}}),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        manager.poll();
        for (_, request) in manager.take_editor_requests() {
            assert!(request.begin());
            let metadata = request.language_control_metadata().unwrap();
            assert_eq!(metadata.consumer, manager.instance_id(client).unwrap());
            assert_eq!(metadata.owner, manager.instance_id(ENGINE).unwrap());
            assert_eq!(metadata.imports.len(), 1);
            assert_eq!(metadata.imports[0].descriptor().abi, "unfamiliar.engine/1");
            request.register_language_consumer(plan).unwrap();
            // Two independent potential loads of the same code are not one transferable grant.
            // Releasing one before its sibling commits must retain the latter's source lifetime.
            let mut first = metadata.imports[0].reserve(plan).unwrap();
            let mut second = metadata.imports[0].reserve(plan).unwrap();
            first.dispatch().unwrap();
            second.dispatch().unwrap();
            first.release();
            second.commit();
            leases.insert(metadata.consumer.clone(), metadata.imports[0].clone());
            if !workers.iter().any(|worker| Arc::ptr_eq(&worker.plan, plan)) {
                workers.push(Worker::spawn(plan.clone()));
            }
            worker(workers, plan).barrier();
            request.finish(Ok(api::EditorValue::Language(Value::Null)));
        }
        let snapshot = manager.live.get_mut(client).unwrap().snapshot().unwrap();
        let receipt = serde_json::from_str::<Value>(&snapshot.data).unwrap()["receipt"].clone();
        if matches!(receipt["state"].as_str(), Some("complete" | "failed")) {
            return receipt;
        }
        assert!(
            Instant::now() < deadline,
            "original consumer did not finish: {receipt}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A stable stdio handle proves that success after a peer retires comes from the original child.
struct Worker {
    plan: Arc<LanguageService>,
    process: ServiceProcess,
    input: ChildStdin,
    output: Option<BufReader<ChildStdout>>,
    sequence: u64,
}
impl Worker {
    fn spawn(plan: Arc<LanguageService>) -> Self {
        let (process, input, output) = plan.spawn().unwrap();
        Self {
            plan,
            process,
            input,
            output: Some(BufReader::new(output)),
            sequence: 0,
        }
    }

    /// A bounded real JSON-RPC round trip is stronger than a retained process-resource count.
    fn barrier(&mut self) {
        self.sequence += 1;
        let body = serde_json::to_vec(&json!({"jsonrpc":"2.0","id":self.sequence,"method":"fixture/barrier","params":{"arguments":null}})).unwrap();
        write!(self.input, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        self.input.write_all(&body).unwrap();
        self.input.flush().unwrap();
        let reply = self.read(false);
        assert_eq!(reply["id"], self.sequence);
        assert_eq!(reply["result"], Value::Null);
    }

    /// Host-only lifecycle notification precedes any later request on this exact ordered pipe.
    fn notify_retired(&mut self, consumers: &[String]) {
        let body = serde_json::to_vec(&json!({"jsonrpc":"2.0",
            "method":plugin_runtime::plugin_protocol::language_control::CONSUMERS_RETIRED_METHOD,
            "params":{"consumers":consumers}}))
        .unwrap();
        write!(self.input, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        self.input.write_all(&body).unwrap();
        self.input.flush().unwrap();
    }

    /// The known fixture closes stdout only when its process ends; retirement must produce EOF.
    fn assert_closed(&mut self) {
        self.read(true);
    }

    /// Pipe reads run behind a bounded wait. On failure stop only this already-owned child.
    fn read(&mut self, closed: bool) -> Value {
        let mut output = self.output.take().unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<Value> {
                if closed {
                    let mut byte = [0];
                    anyhow::ensure!(
                        output.read(&mut byte)? == 0,
                        "retired worker still has stdout data"
                    );
                    return Ok(Value::Null);
                }
                let mut length = None;
                loop {
                    let mut line = String::new();
                    anyhow::ensure!(
                        output.read_line(&mut line)? > 0,
                        "worker closed before replying"
                    );
                    anyhow::ensure!(line.len() <= 1024, "fixture response header overflow");
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(value) = line.strip_prefix("Content-Length: ") {
                        length = Some(value.trim().parse::<usize>()?);
                    }
                }
                let length =
                    length.ok_or_else(|| anyhow::anyhow!("missing fixture frame length"))?;
                anyhow::ensure!(length <= 64 * 1024, "fixture response overflow");
                let mut body = vec![0; length];
                output.read_exact(&mut body)?;
                Ok(serde_json::from_slice(&body)?)
            })();
            let _ = send.send((output, result));
        });
        match receive.recv_timeout(Duration::from_secs(5)) {
            Ok((output, result)) => {
                self.output = Some(output);
                result.unwrap()
            }
            Err(error) => {
                self.process.stop();
                panic!("owned fixture response timed out: {error}")
            }
        }
    }
}

/// Find the exact existing native worker, never a service with only a matching display identity.
fn worker<'a>(workers: &'a mut [Worker], plan: &Arc<LanguageService>) -> &'a mut Worker {
    workers
        .iter_mut()
        .find(|worker| Arc::ptr_eq(&worker.plan, plan))
        .unwrap()
}
fn input(name: &str) -> PathBuf {
    std::env::var_os(name).map(PathBuf::from).expect(name)
}
