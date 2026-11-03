//! An admitted burst must survive a slow native reader without dropping or reordering its bytes.

use super::*;

/// Saturate real stdin before the child reads, then verify every accepted chunk through its output.
#[test]
fn admitted_input_survives_writer_backpressure_in_order() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("input_probe.rs");
    let binary = directory
        .path()
        .join(format!("input-probe{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(
        &source,
        r#"
//! Delay stdin so the parent's bounded writer saturates before verifying its exact byte stream.
use std::io::{Read, Write};
fn main() {
    if std::env::args().nth(1).as_deref() == Some("block") {
        std::thread::sleep(std::time::Duration::from_secs(60));
        return;
    }
    std::thread::sleep(std::time::Duration::from_millis(500));
    let mut bytes = vec![0; 64 * 4096];
    std::io::stdin().read_exact(&mut bytes).unwrap();
    for (index, chunk) in bytes.chunks(4096).enumerate() {
        assert!(chunk.iter().all(|byte| *byte == index as u8));
    }
    print!("INPUT_ORDER_OK");
    std::io::stdout().flush().unwrap();
}
"#,
    )
    .unwrap();
    let build = std::process::Command::new(resolve_program("rustc").unwrap())
        .arg(&source)
        .args(["--crate-name", "input_probe", "--edition=2024", "-o"])
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let group = NativeProcessGroup::new(true);
    group
        .launch(
            1,
            NativeLaunch {
                program: binary.display().to_string(),
                args: vec![],
                cwd: directory.path().display().to_string(),
                env: BTreeMap::new(),
                transport: Transport::Stdio,
            },
        )
        .unwrap();
    for byte in 0..64 {
        group.write(1, vec![byte; 4096]).unwrap();
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut output = Vec::new();
    loop {
        for event in group.poll() {
            match event {
                NativeProcessEvent::Update {
                    update: Update::Output { bytes, .. },
                    ..
                } => output.extend(bytes),
                NativeProcessEvent::Failed { message, .. } => {
                    panic!("accepted input failed: {message}")
                }
                _ => {}
            }
        }
        if String::from_utf8_lossy(&output).contains("INPUT_ORDER_OK") {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "admitted input never reached the child in order"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    group
        .shutdown()
        .unwrap()
        .recv_timeout(Duration::from_secs(5))
        .unwrap();

    // A reader that never resumes must still honor the out-of-band shutdown gate.
    let blocked = NativeProcessGroup::new(true);
    blocked
        .launch(
            2,
            NativeLaunch {
                program: binary.display().to_string(),
                args: vec!["block".into()],
                cwd: directory.path().display().to_string(),
                env: BTreeMap::new(),
                transport: Transport::Stdio,
            },
        )
        .unwrap();
    for byte in 0..64 {
        blocked.write(2, vec![byte; 4096]).unwrap();
    }
    std::thread::sleep(Duration::from_millis(500));
    blocked
        .shutdown()
        .unwrap()
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
}
