//! A timed-out native import retains its source authority until the possibly loaded worker stops.
//! The real OS scheduler observes the 1000ms public request independently of synchronous pipe IO.
use super::*;
use protocol::{language::DocumentContext, language_control::Import, service, ui::Kind};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// This independent SDK guest is a module publisher, rather than another instance of the recipient.
const SOURCE: &str = "rejected-module-source";

/// Keep the parent's native service/method schemas and declare the two immutable module uses.
/// The fixture acknowledges and withholds requests, but does not execute either module's code.
pub(super) fn prepare(package: Package) -> Vec<Package> {
    let mut files = package.files;
    let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["api"]["required"]["language.modules"] = json!("^1");
    manifest["api"]["required"]["plugin.services"] = json!("^1.1");
    let permissions = manifest["permissions"].as_array_mut().unwrap();
    for permission in ["native.modules", "services.call"] {
        if !permissions.contains(&json!(permission)) {
            permissions.push(json!(permission));
        }
    }
    // Module loading requires the method's explicit participant grants as well as package grants.
    // Keep the parent's native methods and schemas; authorize only those two controlled fixture calls.
    for method in ["fixture/barrier", "fixture/pending"] {
        manifest["language_servers"][0]["control_methods"][method]["permissions"] =
            json!(["editor.read", "workspace.read", "native.modules"]);
    }
    let method = json!({"parameters":{"type":"null"},"result":{"type":"null"}});
    manifest["plugin_services"]["requires"]["novel.module"] =
        json!({"version":"^1","methods":{"identity":method}});
    let healthy = b"module.exports = { kind: 'healthy' };\n";
    manifest["module_exports"]["healthy"] = export("modules/healthy", healthy, "novel.engine/1");
    files.insert("modules/healthy/index.cjs".into(), healthy.to_vec());
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let owner = Package::from_files(files).unwrap();
    let fixture = PathBuf::from(
        std::env::var_os("NANOBUG_NATIVE_CONSUMERS_PACKAGE")
            .expect("current SDK native-consumers package"),
    );
    let guest = Package::read(&fixture).unwrap().files["plugin.wasm"].clone();
    let code = b"module.exports = { kind: 'rejected' };\n";
    let publisher = json!({"id":SOURCE,"name":"Unknown timeout module source","version":"0.1.0","protocol":7,
        "component":"plugin.wasm","storage_limit":16384,"permissions":["native.modules"],
        "api":{"base":"^1","required":{"language.modules":"^1","plugin.services":"^1.1"}},
        "module_exports":{"rejected":export("modules/rejected",code,"novel.engine/2")},
        "plugin_services":{"provides":{"novel.module":{"version":"1.0.0","methods":{"identity":method}}}}});
    vec![
        owner,
        Package::from_files(BTreeMap::from([
            (
                "manifest.json".into(),
                serde_json::to_vec(&publisher).unwrap(),
            ),
            ("plugin.wasm".into(), guest),
            ("modules/rejected/index.cjs".into(), code.to_vec()),
        ]))
        .unwrap(),
    ]
}

/// Observe a real matched cancellation, reuse the original pipe, then revoke its uncertain import.
/// No receipt means the host must not infer that native loading or external effects were undone.
pub(super) async fn exercise(harness: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    eprintln!(
        "native module control phase: activate independent exporter before measured requests"
    );
    extensions::demand_plugin_fixture(
        &mut harness.manager.borrow_mut(),
        SOURCE,
        plugin_runtime::DemandReason::Service("novel.module".into()),
    )?;
    harness.publish(cx)?;
    eprintln!("native module control phase: attach healthy own export");
    let accepted = harness.begin_request(
        request(
            "fixture/barrier",
            vec![Import {
                export: "healthy".into(),
                reference: None,
            }],
        ),
        15_000,
        cx,
    )?;
    harness.finish(&accepted, cx).await?;
    completed(&accepted)?;
    let document = DocumentContext {
        path: "source.txt".into(),
        generation: harness
            .manager
            .borrow_mut()
            .availability("capability-example")?
            .configuration_generation(),
    };
    let original = harness.plan(&document)?;
    let initializations = harness.count("initialize")?;
    let pending_before = harness.count("fixture/pending")?;
    let reference = open_module(&mut harness.manager.borrow_mut())?;
    eprintln!("native module control phase: imported request with unknown native outcome");
    let started = Instant::now();
    let pending = harness.begin_request(
        request(
            "fixture/pending",
            vec![Import {
                export: "rejected".into(),
                reference: Some(reference),
            }],
        ),
        1_000,
        cx,
    )?;
    harness.finish(&pending, cx).await?;
    ensure!(
        matches!(
            pending.status(),
            api::RequestUpdate::Cancelled {
                reason: api::ErrorCode::TimedOut,
                ..
            }
        ),
        "the imported request must retain its original 1000ms budget: {:?}",
        pending.status()
    );
    eprintln!(
        "native module control: 1000ms request observed after {:?}",
        started.elapsed()
    );
    harness.wait_for_cancel(pending_before + 1, cx).await?;
    eprintln!("native module control phase: timeout and matching cancellation verified");
    ensure!(
        harness.count("fixture/pending")? == pending_before + 1,
        "one timed-out import must enter the original pipe exactly once"
    );
    let barrier = harness.begin("fixture/barrier", 15_000, cx)?;
    harness.finish(&barrier, cx).await?;
    completed(&barrier)?;
    ensure!(
        Arc::ptr_eq(&original, &harness.plan(&document)?),
        "a timed-out import replaced the healthy native context"
    );
    ensure!(
        harness.count("initialize")? == initializations,
        "a timed-out import restarted the native process"
    );
    harness.manager.borrow_mut().disable(SOURCE)?;
    harness.publish(cx)?;
    ensure!(
        !original.is_active() && original.process_count() == 0,
        "an uncertain import must retain its original source's revocation authority"
    );
    eprintln!("native module control: uncertain import retained source revocation authority");
    Ok(())
}

/// Input remains a public guest request; the Manager signs imports and current document identity.
fn request(method: &str, imports: Vec<Import>) -> protocol::language_control::Request {
    protocol::language_control::Request {
        provider: "analysis".into(),
        document: DocumentContext {
            path: "source.txt".into(),
            generation: 0,
        },
        method: method.into(),
        arguments: Value::Null,
        context_snapshot: false,
        service_sets: Vec::new(),
        imports,
        publish_binding: None,
    }
}

/// Observe a real SDK-owned reference instead of manufacturing an instance or resource identity.
fn open_module(manager: &mut Manager) -> anyhow::Result<api::ResourceHandle> {
    manager.invoke_command(
        "capability-example",
        "scope-probe",
        serde_json::to_value(api::Operation::Service {
            operation: service::Operation::Open {
                contract: "novel.module".into(),
            },
        })?,
    )?;
    let Kind::Text { text } = &manager.live["capability-example"].views["welcome"]
        .as_ref()
        .root
        .kind
    else {
        anyhow::bail!("the SDK diagnostic view did not publish its public operation result");
    };
    let result: Result<api::Value, api::Failure> = serde_json::from_str(text)?;
    let api::Value::Resource(reference) = result? else {
        anyhow::bail!("the SDK did not return a real service reference");
    };
    Ok(reference)
}

/// Every native entry remains a completely declared, immutable package closure under admission.
fn export(root: &str, code: &[u8], abi: &str) -> Value {
    let hash = format!("{:x}", Sha256::digest(code));
    json!({"root":root,"entry":"index.cjs","sha256":hash,"files":{"index.cjs":hash},"abi":abi,"engine":{"novel":"^1"}})
}
