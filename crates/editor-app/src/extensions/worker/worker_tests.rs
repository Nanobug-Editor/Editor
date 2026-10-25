//! Exercise the real actor and its published request channel while a local dependency source is paused.
use super::*;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

mod language_failures;

// Cross-project ownership observes the same production actor and public native publications.
#[path = "../../tests/project_windows/runtime.rs"]
mod concurrent;
#[path = "../../tests/project_windows/installation.rs"]
mod concurrent_installation;

/// The native installation panel receives the selected UI language from the actual queue,
/// including before a package worker has performed any preparation or dependency I/O.
#[test]
#[ignore = "process locale is global; run this publication regression with --test-threads=1"]
fn queued_installation_progress_follows_the_selected_ui_locale() {
    struct RestoreLocale(String);
    impl Drop for RestoreLocale {
        fn drop(&mut self) {
            rust_i18n::set_locale(&self.0);
        }
    }
    let _restore = RestoreLocale(rust_i18n::locale().to_string());
    let package = Package::from_files(BTreeMap::from([
        (
            "manifest.json".into(),
            serde_json::to_vec(&json!({
                "id":"unknown-progress", "name":"Unknown progress fixture", "version":"1.0.0",
                "protocol":7, "api":{"base":"^1"}, "contributions":"plugin.toml", "storage_limit":1024
            })).unwrap(),
        ),
        (
            "plugin.toml".into(),
            b"[plugin]\nid='unknown-progress'\nname='Unknown progress fixture'\nversion='1.0.0'\nhost_version='^0.1'\n".to_vec(),
        ),
    ])).unwrap();
    for (locale, preparing) in [("en", "Preparing installation"), ("zh-CN", "准备安装")] {
        rust_i18n::set_locale(locale);
        let directory = tempfile::tempdir().unwrap();
        let worker = Worker::start(directory.path().into(), Environment::default(), true);
        assert!(worker.queue_lifecycle(Work::Install(package.clone(), true)));
        let mut progress = worker
            .state
            .lock()
            .unwrap()
            .installation
            .clone()
            .expect("native publication before dispatch");
        assert!(
            progress.message.contains(preparing),
            "{locale}: {}",
            progress.message
        );
        assert_eq!(progress.id, "unknown-progress");
        assert!(progress.cancellable && !progress.installed);
        use plugin_runtime::InstallStage::*;
        for (stage, english, chinese, cancellable) in [
            (Preparing, "Preparing dependencies", "正在准备依赖", true),
            (
                Downloading("unknown-tool".into()),
                "Downloading / reading dependency: unknown-tool",
                "正在下载 / 读取依赖：unknown-tool",
                true,
            ),
            (
                Verifying("unknown-tool".into()),
                "Verifying SHA-256: unknown-tool",
                "正在验证 SHA-256：unknown-tool",
                true,
            ),
            (
                Extracting("unknown-tool".into()),
                "Extracting dependency: unknown-tool",
                "正在解包依赖：unknown-tool",
                true,
            ),
            (
                AwaitingAuthorization(42),
                "step 42 is waiting",
                "步骤 42 正在等待",
                true,
            ),
            (
                Installing("unknown purpose".into()),
                "Running installation step: unknown purpose",
                "正在执行安装步骤：unknown purpose",
                true,
            ),
            (Prepared, "Dependencies are ready", "依赖已准备", false),
            (Migrating, "Migrating plugin data", "迁移插件数据", false),
            (
                Committing,
                "Committing the plugin version",
                "正在提交插件版本",
                false,
            ),
            (Committed, "have been committed", "已提交", false),
        ] {
            progress.advance(stage);
            let expected = if locale == "en" { english } else { chinese };
            assert!(
                progress.message.contains(expected),
                "{locale}: {}",
                progress.message
            );
            assert_eq!(progress.cancellable, cancellable);
            assert!(
                !progress.installed,
                "a manager stage never fabricates terminal success"
            );
            assert_eq!(progress.id, "unknown-progress");
        }

        // A real actor's failed inspection publishes the translated action and keeps the concrete
        // filesystem cause. No native service, network source or project input is used here.
        let background = Worker::start_background(
            directory.path().join("background"),
            Environment::default(),
            true,
        );
        wait_for(&background, |state| state.ready);
        assert!(background.queue_lifecycle(Work::Inspect(directory.path().join("missing.zip"))));
        wait_for(&background, |state| state.status.is_some());
        let failure = background
            .state
            .lock()
            .unwrap()
            .status
            .clone()
            .unwrap()
            .message;
        let expected = if locale == "en" {
            "Inspect package failed"
        } else {
            "检查插件包失败"
        };
        assert!(failure.contains(expected), "{locale}: {failure}");
        let (tx, rx) = futures::channel::oneshot::channel();
        background.tx.send(Work::Shutdown(Some(tx))).unwrap();
        futures::executor::block_on(rx).unwrap().unwrap();
    }
}

/// A program the caller stopped is not the same fact as one nobody can describe.
///
/// The vocabulary here is the execution contract's, and the terminal provider answers a stop with
/// `stopped`. Without an arm for that word the editor read it as `Unknown` — the same thing it says
/// when a provider cannot answer at all — and the sequence's `Terminated` outcome was unreachable from
/// any real answer, so a preparation stopped by the user reached it as an unexplained failure.
#[test]
fn a_stopped_program_is_terminated_rather_than_unknown() {
    // The four states the terminal provider's status call can report, and what each means here.
    assert_eq!(
        RunStatus::from_value(&json!({"state": "running"})),
        RunStatus::Running
    );
    assert_eq!(
        RunStatus::from_value(&json!({"state": "exited", "code": 3})),
        RunStatus::Ended { code: Some(3) }
    );
    assert_eq!(
        RunStatus::from_value(&json!({"state": "ended"})),
        RunStatus::Ended { code: None },
        "an end with no status of its own is an end, never a success"
    );
    assert_eq!(
        RunStatus::from_value(&json!({"state": "stopped"})),
        RunStatus::Terminated,
        "a stopped program is terminated, not unknown"
    );
    // Anything the contract does not define stays unknown rather than being read as an ending.
    assert_eq!(
        RunStatus::from_value(&json!({"state": "something-new"})),
        RunStatus::Unknown
    );
    assert_eq!(
        RunStatus::from_value(&json!({})),
        RunStatus::Unknown,
        "an answer with no state describes nothing"
    );
}

/// Native batches cannot accumulate while the worker is busy, and dropping rejected work restores capacity.
#[test]
fn image_offer_queue_releases_capacity_on_rejection_and_shutdown() {
    let directory = tempfile::tempdir().unwrap();
    let worker = Worker::start(directory.path().into(), Environment::default(), true);
    let first = worker.reserve_image_offer().unwrap();
    let second = worker.reserve_image_offer().unwrap();
    assert!(
        worker.reserve_image_offer().is_none(),
        "the pending channel cannot exceed two 32 MiB batches"
    );
    drop(first);
    let replacement = worker.reserve_image_offer().unwrap();
    assert!(worker.reserve_image_offer().is_none());
    drop(worker);
    drop(second);
    drop(replacement);
}

/// Native publications are the same seam the editor polls; no second manager services these commands.
/// Preserve the caller and a bounded publication snapshot on timeout so distinct lifecycle waits
/// can be diagnosed without changing their predicate, deadline or frozen package input.
#[track_caller]
fn wait_for(worker: &Worker, ready: impl Fn(&Published) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if ready(&worker.state.lock().unwrap()) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "worker publication timed out: {}",
            {
                let state = worker.state.lock().unwrap();
                format!(
                    "ready={}, views={:?}, status={:?}, startup={:?}, installation={:?}, editor_requests={}",
                    state.ready,
                    state.views.keys().collect::<Vec<_>>(),
                    state.status.as_ref().map(|status| &status.message),
                    state.startup,
                    state.installation,
                    state.editor_requests.len(),
                )
            }
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A fixture enters through the same published static identity the native command consumer uses.
fn command_work(worker: &Worker, command: &str, arguments: serde_json::Value) -> Work {
    let state = worker.state.lock().unwrap();
    Work::Invoke {
        plugin: "capability-example".into(),
        command: command.into(),
        arguments,
        context: None,
        expected_epoch: state
            .command_epoch("capability-example", command)
            .unwrap_or_default(),
        origin: state
            .availabilities
            .get("capability-example")
            .cloned()
            .map(|availability| super::demands::CommandOrigin {
                availability,
                guard: None,
            }),
    }
}

/// Callback tests explicitly show the declared view; installation alone no longer runs Main.
fn show_fixture_view(worker: &Worker) {
    wait_for(worker, |state| state.ready);
    let availability = worker.state.lock().unwrap().availabilities["capability-example"].clone();
    let mut wait = worker
        .demand(
            availability,
            plugin_runtime::DemandReason::View("welcome".into()),
            None,
        )
        .unwrap();
    futures::executor::block_on(wait.wait()).unwrap();
}

/// Installed command metadata is discoverable without creating an instance or publishing a view.
/// The independently built public package follows the same cold startup path as a user profile.
#[test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn cold_commands_are_discoverable_without_starting_the_guest() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("plugins");
    // Static discovery reads the current component's declaration but must not execute its business.
    let package = crate::test_packages::package("capability-example-0.19.1");
    let command = package.manifest.commands.first().unwrap().id.clone();
    let mut manager = Manager::open(root.clone(), Environment::default()).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    drop(manager);
    let worker = Worker::start_background(root, Environment::default(), true);
    wait_for(&worker, |state| state.ready);
    let (discoverable, has_views, has_instances) = {
        let state = worker.state.lock().unwrap();
        (
            state
                .command_epoch("capability-example", &command)
                .is_some(),
            !state.views.is_empty(),
            !state.instance_ids.is_empty(),
        )
    };
    // Release the publication mutex and shut down before assertions so a genuine regression
    // cannot poison the actor's cleanup path and turn the expected test failure into an abort.
    let (tx, rx) = futures::channel::oneshot::channel();
    worker.tx.send(Work::Shutdown(Some(tx))).unwrap();
    // Checked retirement reports persistence/native cleanup failure separately from delivery.
    futures::executor::block_on(rx).unwrap().unwrap();
    assert!(
        discoverable,
        "opening a command menu must not depend on a live guest"
    );
    assert!(!has_views, "metadata discovery cannot run Main");
    assert!(!has_instances, "the installed guest stays cold");
}

/// Actual command business code must execute after first birth under the original cold epoch.
/// Discoverability alone or an empty Main publication cannot satisfy this positive control.
#[test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn cold_command_first_birth_preserves_its_original_command_epoch() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("plugins");
    // Both fixture installation and the production actor must select the same open project;
    // an empty workspace only manages application metadata and cannot consume this command.
    let environment = Environment {
        workspace: directory.path().display().to_string(),
        ..Default::default()
    };
    // Preserve the current consumer's actual command callback and original cold epoch.
    let package = crate::test_packages::package("capability-example-0.19.1");
    let mut manager = Manager::open(root.clone(), environment.clone()).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    drop(manager);
    let worker = Worker::start_background(root, environment, true);
    wait_for(&worker, |state| state.ready);
    let original = worker
        .state
        .lock()
        .unwrap()
        .command_epoch("capability-example", "check-errors");
    worker
        .tx
        .send(command_work(
            &worker,
            "check-errors",
            serde_json::Value::Null,
        ))
        .unwrap();
    wait_for(&worker, |state| {
        state.views.values().any(|view| {
            serde_json::to_string(view)
                .unwrap()
                .contains("Typed errors and request IDs verified.")
        })
    });
    let (epoch, born) = {
        let state = worker.state.lock().unwrap();
        (
            state.command_epoch("capability-example", "check-errors"),
            state.instance_ids.contains_key("capability-example"),
        )
    };
    let (tx, rx) = futures::channel::oneshot::channel();
    worker.tx.send(Work::Shutdown(Some(tx))).unwrap();
    futures::executor::block_on(rx).unwrap().unwrap();
    assert!(born, "first demand must produce its real owner");
    assert!(
        original.is_some(),
        "the original command was statically visible"
    );
    assert_eq!(
        epoch, original,
        "first birth cannot revoke its own original command"
    );
}

/// Production dispatch retains a command target while inspection failures remain manager-wide.
#[test]
fn operation_errors_retain_plugin_ownership_through_worker_publication() {
    let directory = tempfile::tempdir().unwrap();
    let worker = Worker::start_background(
        directory.path().join("plugins"),
        Environment::default(),
        true,
    );
    worker
        .tx
        .send(Work::Invoke {
            plugin: "missing-plugin".into(),
            origin: None,
            context: None,
            command: "unknown".into(),
            arguments: json!(null),
            // The actor must reject an absent owner using the observed publication, not invent one.
            expected_epoch: worker
                .state
                .lock()
                .unwrap()
                .instance_epochs
                .get("missing-plugin")
                .copied()
                .unwrap_or_default(),
        })
        .unwrap();
    wait_for(&worker, |state| state.status.is_some());
    let status = worker.state.lock().unwrap().status.take().unwrap();
    assert_eq!(status.plugin.as_deref(), Some("missing-plugin"));
    let expected = api::Failure::new(
        api::ErrorCode::StaleRevision,
        rust_i18n::t!("plugins.command_owner_changed").to_string(),
    )
    .to_string();
    assert_eq!(status.message, expected);
    let logs = worker.state.lock().unwrap().logs.clone();
    assert!(logs.records("missing-plugin").iter().any(|record| {
        record.level == plugin_runtime::logs::LogLevel::Error
            && record.source == "host.operation"
            && record.message == expected
    }));
    worker.queue_lifecycle(Work::Inspect(directory.path().join("missing.zip")));
    wait_for(&worker, |state| state.status.is_some());
    assert!(
        worker
            .state
            .lock()
            .unwrap()
            .status
            .as_ref()
            .unwrap()
            .plugin
            .is_none()
    );
    let (tx, rx) = futures::channel::oneshot::channel();
    worker.tx.send(Work::Shutdown(Some(tx))).unwrap();
    futures::executor::block_on(rx).unwrap().unwrap();
}

/// Obsolete native revisions and incarnations remain rejected without faulting a healthy real guest.
#[test]
#[ignore = "opt-in real WASM/native integration; uses fixed reviewed test packages and required pinned native tools"]
fn obsolete_ui_callbacks_are_information_but_real_rejections_are_errors() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("plugins");
    let environment = Environment {
        workspace: directory.path().display().to_string(),
        ..Default::default()
    };
    let package = Package::read(&crate::test_packages::archive("capability-example")).unwrap();
    let mut manager = Manager::open(root.clone(), environment.clone()).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    drop(manager);
    let worker = Worker::start_background(root, environment, true);
    show_fixture_view(&worker);
    wait_for(&worker, |state| {
        state.views.contains_key("capability-example/welcome")
    });
    let (logs, epoch, revision) = {
        let state = worker.state.lock().unwrap();
        (
            state.logs.clone(),
            state.instance_epochs["capability-example"],
            state.views["capability-example/welcome"].revision,
        )
    };
    worker
        .tx
        .send(Work::Event(
            "capability-example".into(),
            epoch,
            Some("welcome".into()),
            api::Notification::Ui(ui::UiEvent {
                revision: revision.wrapping_add(1),
                node: "obsolete".into(),
                action: ui::Action::Click,
            }),
        ))
        .unwrap();
    wait_for(&worker, |_| {
        logs.records("capability-example")
            .iter()
            .any(|entry| entry.source == "host.ui.stale")
    });
    assert_eq!(logs.unread_severity("capability-example"), None);
    assert!(worker.state.lock().unwrap().status.is_none());
    worker
        .tx
        .send(Work::Event(
            "capability-example".into(),
            epoch.wrapping_add(1),
            Some("welcome".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: "obsolete".into(),
                action: ui::Action::Click,
            }),
        ))
        .unwrap();
    wait_for(&worker, |_| {
        logs.records("capability-example")
            .iter()
            .any(|entry| entry.source == "host.ui.retired")
    });
    assert_eq!(logs.unread_severity("capability-example"), None);
    // A current callback to an invalid node is a real error; typed stale handling cannot swallow it.
    worker
        .tx
        .send(Work::Event(
            "capability-example".into(),
            epoch,
            Some("welcome".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: "missing-node".into(),
                action: ui::Action::Click,
            }),
        ))
        .unwrap();
    wait_for(&worker, |state| state.status.is_some());
    assert_eq!(
        logs.unread_severity("capability-example"),
        Some(plugin_runtime::logs::LogLevel::Error)
    );
    assert!(
        worker
            .state
            .lock()
            .unwrap()
            .views
            .contains_key("capability-example/welcome")
    );
    let (tx, rx) = futures::channel::oneshot::channel();
    worker.tx.send(Work::Shutdown(Some(tx))).unwrap();
    futures::executor::block_on(rx).unwrap().unwrap();
}

/// Both old-version writes and typed editor completions must progress before the download is released.
#[test]
#[ignore = "opt-in real WASM/native integration; uses fixed reviewed test packages and required pinned native tools"]
fn background_preparation_keeps_old_plugin_commands_and_requests_running() {
    run_preparation(false);
}

/// A grant queued before a later revocation must not revive the old plugin after cancellation finishes.
#[test]
#[ignore = "opt-in real WASM/native integration; uses fixed reviewed test packages and required pinned native tools"]
fn cancelled_preparation_does_not_replay_an_older_trust_grant() {
    run_preparation(true);
}

/// Both paths use a paused HTTP source and the production actor, including real cancellation cleanup.
fn run_preparation(restrict: bool) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("plugins");
    std::fs::write(directory.path().join("source.txt"), "workspace").unwrap();
    let environment = Environment {
        workspace: directory.path().display().to_string(),
        ..Default::default()
    };
    let source = crate::test_packages::archive("capability-example");
    let old = Package::read(&source).unwrap();
    let mut manager = Manager::open(root.clone(), environment.clone()).unwrap();
    manager
        .install(&old, old.manifest.permissions.clone())
        .unwrap();
    drop(manager);
    let worker = Worker::start_background(root, environment, true);
    show_fixture_view(&worker);
    wait_for(&worker, |state| {
        state.views.contains_key("capability-example/welcome")
    });

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/artifact", listener.local_addr().unwrap());
    let (entered, waiting) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let payload = b"unused preparation fixture";
    let download = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0; 4096];
        stream.read(&mut request).unwrap();
        entered.send(()).unwrap();
        // A dropped sender also unblocks this thread when an earlier assertion fails.
        let _ = gate.recv_timeout(Duration::from_secs(30));
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            payload.len()
        );
        let _ = stream.write_all(payload);
    });
    let mut files = old.files.clone();
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["version"] = json!("99.0.0");
    manifest["api"]["required"]["dependencies"] = json!("^1");
    manifest["permissions"]
        .as_array_mut()
        .unwrap()
        .push(json!("dependencies.prepare"));
    manifest["services"] = json!({"prepared-only":{
        "program":"unused-prepared-service", "args":[], "installation":{
            "executable":"artifact/tool.exe", "artifacts":[{
                "id":"artifact", "version":"1.0.0", "platform":format!("{}-{}",std::env::consts::OS,std::env::consts::ARCH),
                "sha256":format!("{:x}",Sha256::digest(payload)),
                "source":{"kind":"url","url":url}, "format":{"kind":"file","path":"tool.exe"}
            }]
        }
    }});
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let update = crate::extensions::language_tests::packages::repack(files).unwrap();
    assert!(worker.queue_lifecycle(Work::Install(update, true)));
    waiting.recv_timeout(Duration::from_secs(60)).unwrap();
    worker
        .tx
        .send(command_work(
            &worker,
            "scope-write",
            json!({"text":"during preparation"}),
        ))
        .unwrap();
    wait_for(&worker, |state| {
        state.views.get("capability-example/welcome").map(|document| document.as_ref())
            .is_some_and(|ui| matches!(&ui.root.kind, ui::Kind::Text{text} if text == "workspace|during preparation"))
    });
    assert!(worker.state.lock().unwrap().progress.is_some());
    worker
        .tx
        .send(command_work(&worker, "active-directory", json!(null)))
        .unwrap();
    wait_for(&worker, |state| !state.editor_requests.is_empty());
    let request = worker
        .state
        .lock()
        .unwrap()
        .editor_requests
        .pop()
        .unwrap()
        .1;
    assert!(request.begin());
    request.finish(Ok(api::EditorValue::Directory {
        path: "still-responsive".into(),
    }));
    wait_for(&worker, |state| {
        state.views.get("capability-example/welcome").map(|document| document.as_ref())
            .is_some_and(|ui| matches!(&ui.root.kind, ui::Kind::Text{text} if text.contains("still-responsive")))
    });
    let old_epoch = worker.state.lock().unwrap().instance_epochs["capability-example"];
    if restrict {
        // Match Extensions::set_workspace_trusted: native authority changes before its mailbox
        // notification. Leaving the startup grant here would tell each routed tick to trust again.
        worker
            .trusted
            .store(true, std::sync::atomic::Ordering::Release);
        worker.tx.send(Work::SetTrust(true)).unwrap();
        worker
            .trusted
            .store(false, std::sync::atomic::Ordering::Release);
        worker.tx.send(Work::SetTrust(false)).unwrap();
        // Revocation takes effect while the network is still paused, not after background cleanup.
        wait_for(&worker, |state| state.views.is_empty());
    }
    release.send(()).unwrap();
    download.join().unwrap();
    wait_for(&worker, |state| state.progress.is_none());
    if restrict {
        assert!(
            !worker
                .state
                .lock()
                .unwrap()
                .installation
                .as_ref()
                .unwrap()
                .installed
        );
        worker
            .tx
            .send(command_work(&worker, "scope-read", json!(null)))
            .unwrap();
        // This later command is an ordering barrier after any queued grant; it must remain rejected.
        let explanation = rust_i18n::t!("plugins.command_owner_changed").to_string();
        wait_for(&worker, |state| {
            state.status.as_ref().is_some_and(|status| {
                status.message.starts_with("StaleRevision: ")
                    && status.message.contains(explanation.as_str())
            })
        });
        let state = worker.state.lock().unwrap();
        assert!(state.views.is_empty());
        assert_eq!(state.entries[0].manifest.version, old.manifest.version);
    } else {
        assert!(
            worker
                .state
                .lock()
                .unwrap()
                .installation
                .as_ref()
                .unwrap()
                .installed
        );
        worker
            .tx
            .send(Work::Event(
                "capability-example".into(),
                old_epoch,
                None,
                api::Notification::Command {
                    id: "scope-write".into(),
                    context: None,
                    arguments: Some(json!({"text":"stale callback"})),
                },
            ))
            .unwrap();
        worker
            .tx
            .send(command_work(&worker, "scope-read", json!(null)))
            .unwrap();
        wait_for(&worker, |state| {
            state.views.get("capability-example/welcome").map(|document| document.as_ref())
            .is_some_and(|ui| matches!(&ui.root.kind, ui::Kind::Text{text} if text == "workspace|during preparation"))
        });
    }
    // Shutdown acknowledgement waits for owner cleanup, so this test never races temp directory deletion.
    let (ack, finished) = futures::channel::oneshot::channel();
    worker.tx.send(Work::Shutdown(Some(ack))).unwrap();
    futures::executor::block_on(finished).unwrap().unwrap();
}
