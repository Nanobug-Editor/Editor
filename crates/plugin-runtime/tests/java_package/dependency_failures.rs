//! Failed or cancelled cold SDK acquisition never reaches compilation or bypasses workspace trust.
use super::*;
use plugin_runtime::plugin_protocol::{api::RequestUpdate, settings};
use std::{
    io::Read,
    net::TcpListener,
    sync::atomic::{AtomicBool, Ordering},
};

/// Retain the actual Java guest and immutable artifact identity while controlling only the source.
/// Offline and HTTP failures must stay visible; cancellation must retire the original public wait.
#[test]
#[ignore = "Build Java with --plugin-package and set NANOBUG_JAVA_PACKAGE to the resulting ZIP"]
fn java_package_missing_sdk_reports_offline_download_and_cancel_without_compiling() {
    let original = Package::read(std::path::Path::new(
        &std::env::var_os("NANOBUG_JAVA_PACKAGE").unwrap(),
    ))
    .unwrap();
    for mode in ["offline", "http", "cancel"] {
        let workspace = tempfile::tempdir().unwrap();
        let profile = tempfile::tempdir().unwrap();
        std::fs::write(
            workspace.path().join("Main.java"),
            "public class Main { public static void main(String[] args) {} }\n",
        )
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/jdk.zip", listener.local_addr().unwrap());
        let reached = std::sync::Arc::new(AtomicBool::new(false));
        let finished = std::sync::Arc::new(AtomicBool::new(false));
        let server = if mode == "offline" {
            drop(listener);
            None
        } else {
            listener.set_nonblocking(true).unwrap();
            let reached = reached.clone();
            let finished = finished.clone();
            Some(std::thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(180);
                while !finished.load(Ordering::Acquire) && Instant::now() < deadline {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            stream
                                .set_read_timeout(Some(Duration::from_secs(10)))
                                .unwrap();
                            let mut request = [0; 4096];
                            let _ = stream.read(&mut request);
                            reached.store(true, Ordering::Release);
                            if mode == "cancel" {
                                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\nConnection: close\r\n\r\npartial");
                                while !finished.load(Ordering::Acquire) && Instant::now() < deadline
                                {
                                    std::thread::sleep(Duration::from_millis(5));
                                }
                            } else {
                                let _ = stream.write_all(b"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                            }
                            break;
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(5))
                        }
                        Err(error) => panic!("Controlled SDK source: {error}"),
                    }
                }
            }))
        };
        let mut files = original.files.clone();
        let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
        // Force absence through the declared readonly resolver; no PATH or user installation changes.
        manifest["services"]["local8"]["program"] = json!("deliberately-absent-java-sdk-fixture");
        manifest["services"]["local8"]["search_paths"] = json!([]);
        manifest["services"]["managed8"]["installation"]["artifacts"][0]["source"] =
            json!({"kind":"url","url":url});
        files.insert(
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        );
        let package = Package::from_files(files).unwrap();
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
                settings::Scope::Project,
                "java.jdkVersion",
                Some(json!("8")),
            )
            .unwrap();
        let request = manager.begin_target_call("java", "prepare", json!({"workspace":workspace.path().display().to_string(),"binding":"{\"schema\":1,\"main_class\":\"Main\"}","env":[]})).unwrap();
        let deadline = Instant::now() + Duration::from_secs(180);
        let mut cancelled = false;
        loop {
            manager.poll();
            if mode == "cancel" && reached.load(Ordering::Acquire) && !cancelled {
                request.stop();
                cancelled = true;
                // Close the partial HTTP body so its blocked reader can observe revocation promptly.
                finished.store(true, Ordering::Release);
            }
            match request.status() {
                RequestUpdate::Completed { result } => {
                    let error = result.expect_err("A missing SDK must not yield a launch command");
                    assert_ne!(
                        mode, "cancel",
                        "Cancellation was replaced by a failure: {error:?}"
                    );
                    assert!(!error.message.is_empty());
                    if mode == "http" {
                        assert!(error.message.contains("503"), "{error:?}");
                    }
                    break;
                }
                RequestUpdate::Cancelled { .. } => {
                    assert!(cancelled, "Unexpected cancellation during {mode}");
                    break;
                }
                _ => {}
            }
            assert!(
                Instant::now() < deadline,
                "Missing SDK did not settle: {mode}"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        finished.store(true, Ordering::Release);
        if let Some(server) = server {
            server.join().unwrap();
            assert!(reached.load(Ordering::Acquire));
        }
        assert!(!workspace.path().join("out/Main.class").exists());
        assert_eq!(manager.live["java"].process_count(), 0);
        manager.set_workspace_trust(false).unwrap();
        assert!(manager.begin_target_call("java", "prepare", json!({"workspace":workspace.path().display().to_string(),"binding":"{}","env":[]})).is_err());
        manager.shutdown_checked().unwrap();
        assert_eq!(manager.resource_count(), 0);
    }
}
