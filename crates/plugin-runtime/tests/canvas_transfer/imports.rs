//! User-dropped files expose bounded import handles without borrowing filesystem authority.
use super::*;

/// Native path producers never serialize paths; selected import reads stay scoped to the drop lease.
struct FileDrop(std::path::PathBuf);
impl plugin_runtime::TransferReader for FileDrop {
    fn read(&self, _mime: &str) -> Result<plugin_runtime::HostTransferData, api::Failure> {
        Ok(plugin_runtime::HostTransferData::Files(vec![
            self.0.clone(),
        ]))
    }
}

/// A user drop grants only bounded reads of the admitted object, including foreign/closed rejection.
#[test]
#[ignore = "opt-in actual independent Canvas Package/WASM; see fixtures/packages/README.md"]
fn native_file_drop_issues_readonly_imports_without_exposing_paths() {
    use plugin_runtime::plugin_protocol::transfer::{self, Origin, Phase, Position};
    let mut first = Harness::open();
    let mut second = Harness::open();
    let external = tempfile::tempdir().unwrap();
    let path = external.path().join("example.tsv");
    std::fs::write(&path, b"name\tvalue\ncell\t42").unwrap();
    let revision = first.manager.live["canvas-flow"].views["diagram"].revision;
    let offer = first
        .manager
        .begin_transfer(
            "canvas-flow",
            "diagram",
            transfer::Address::node("drop-zone", revision),
            plugin_runtime::HostTransferOffer::new(
                Origin::OperatingSystem,
                vec![Format {
                    mime: transfer::FILES_MIME.into(),
                    byte_len: None,
                }],
                std::sync::Arc::new(FileDrop(path)),
            )
            .unwrap(),
            Position { x: 3., y: 5. },
        )
        .unwrap();
    first
        .manager
        .update_transfer(
            "canvas-flow",
            "diagram",
            &offer.handle,
            Phase::Drop,
            Position { x: 3., y: 5. },
        )
        .unwrap();
    let read = |operation| api::Operation::Editor {
        operation: api::EditorOperation::Transfer { operation },
        timeout_ms: 30_000,
    };
    first
        .call(read(transfer::Operation::Read {
            offer: offer.handle.clone(),
            preferred: vec![transfer::FILES_MIME.into()],
        }))
        .unwrap();
    let request = first.manager.take_editor_requests().pop().unwrap().1;
    assert!(request.begin());
    let resource = request.transfer_offer().unwrap().clone();
    let transfer::Value::Files(files) = resource
        .read_selected(&[transfer::FILES_MIME.into()])
        .unwrap()
    else {
        panic!("controlled file imports expected")
    };
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "example.tsv");
    assert_eq!(files[0].byte_len, 18);
    assert!(
        !serde_json::to_string(&files)
            .unwrap()
            .contains(&external.path().to_string_lossy().to_string())
    );
    let file = files[0].handle.clone();
    assert!(
        first.call(api::Operation::OpenWorkspace).is_err(),
        "drop consent grants no workspace authority"
    );
    assert_eq!(
        first
            .call(api::Operation::ReadFile {
                handle: offer.handle.clone(),
                path: String::new()
            })
            .unwrap_err()
            .code,
        api::ErrorCode::InvalidHandle,
        "offer handles are never ordinary file roots"
    );
    request.finish(Ok(api::EditorValue::Transfer(transfer::Value::Files(
        files,
    ))));
    first.manager.poll();
    let chunk = || {
        read(transfer::Operation::ReadFile {
            file: file.clone(),
            offset: 11,
            max_bytes: 16,
        })
    };
    assert_eq!(
        second.call(chunk()).unwrap_err().code,
        api::ErrorCode::InvalidHandle
    );
    assert!(matches!(
        first.call(chunk()).unwrap(),
        api::Value::Accepted(_)
    ));
    let request = first.manager.take_editor_requests().pop().unwrap().1;
    assert!(request.begin());
    let transfer::Value::FileBytes { bytes, eof } = request
        .transfer_offer()
        .unwrap()
        .read_file(&file, 11, 16)
        .unwrap()
    else {
        panic!("file bytes expected")
    };
    assert_eq!(bytes, b"cell\t42");
    assert!(eof);
    request.finish(Ok(api::EditorValue::Transfer(transfer::Value::FileBytes {
        bytes,
        eof,
    })));
    first.manager.poll();
    first
        .call(api::Operation::CloseResource {
            handle: offer.handle,
        })
        .unwrap();
    assert_eq!(
        first.call(chunk()).unwrap_err().code,
        api::ErrorCode::InvalidHandle
    );
    assert!(resource.read_file(&file, 0, 16).is_err());
}

/// A stalled external renderer must not keep its destination's instance worker from revoking consent.
struct DelayedFileDrop {
    path: std::path::PathBuf,
    started: std::sync::mpsc::SyncSender<()>,
    release: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
}
impl plugin_runtime::TransferReader for DelayedFileDrop {
    fn read(&self, _: &str) -> Result<plugin_runtime::HostTransferData, api::Failure> {
        self.started.send(()).unwrap();
        self.release.lock().unwrap().recv().unwrap();
        Ok(plugin_runtime::HostTransferData::Files(vec![
            self.path.clone(),
        ]))
    }
}

/// Closing a live offer stays responsive while an external provider blocks; late data cannot revive it.
#[test]
#[ignore = "opt-in actual independent Canvas Package/WASM; see fixtures/packages/README.md"]
fn cancelling_a_drop_does_not_wait_for_a_stalled_native_file_producer() {
    use plugin_runtime::plugin_protocol::transfer::{self, Origin, Phase, Position};
    let mut harness = Harness::open();
    let external = tempfile::tempdir().unwrap();
    let path = external.path().join("pending.txt");
    std::fs::write(&path, b"late data").unwrap();
    let (started_tx, started_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let revision = harness.manager.live["canvas-flow"].views["diagram"].revision;
    let offer = harness
        .manager
        .begin_transfer(
            "canvas-flow",
            "diagram",
            transfer::Address::node("drop-zone", revision),
            plugin_runtime::HostTransferOffer::new(
                Origin::OperatingSystem,
                vec![Format {
                    mime: transfer::FILES_MIME.into(),
                    byte_len: None,
                }],
                std::sync::Arc::new(DelayedFileDrop {
                    path,
                    started: started_tx,
                    release: std::sync::Mutex::new(release_rx),
                }),
            )
            .unwrap(),
            Position { x: 1., y: 1. },
        )
        .unwrap();
    harness
        .manager
        .update_transfer(
            "canvas-flow",
            "diagram",
            &offer.handle,
            Phase::Drop,
            Position { x: 1., y: 1. },
        )
        .unwrap();
    harness
        .call(api::Operation::Editor {
            operation: api::EditorOperation::Transfer {
                operation: transfer::Operation::Read {
                    offer: offer.handle.clone(),
                    preferred: vec![transfer::FILES_MIME.into()],
                },
            },
            timeout_ms: 30_000,
        })
        .unwrap();
    let request = harness.manager.take_editor_requests().pop().unwrap().1;
    assert!(request.begin());
    let resource = request.transfer_offer().unwrap().clone();
    let reader = std::thread::spawn(move || resource.read_selected(&[transfer::FILES_MIME.into()]));
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let (closed_tx, closed_rx) = std::sync::mpsc::sync_channel(1);
    let closer = std::thread::spawn(move || {
        let result = harness.call(api::Operation::CloseResource {
            handle: offer.handle,
        });
        closed_tx.send(result).unwrap();
    });
    let responsive = closed_rx.recv_timeout(Duration::from_secs(1));
    release_tx.send(()).unwrap();
    closer.join().unwrap();
    assert!(
        reader.join().unwrap().is_err(),
        "late files cannot reissue revoked import authority"
    );
    responsive
        .expect("CloseResource blocked on an external file producer")
        .unwrap();
}
