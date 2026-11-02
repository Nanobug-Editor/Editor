//! Opt-in checks use a real packaged Windows executable, independently of the test harness subsystem.
#![cfg(target_os = "windows")]

use std::{path::PathBuf, process::Command};

/// Return the explicitly selected distribution artifact so a debug/test executable cannot satisfy release checks.
fn executable() -> PathBuf {
    std::env::var_os("NANOBUG_WINDOWS_DISTRIBUTION_EXE")
        .map(PathBuf::from)
        .expect("set NANOBUG_WINDOWS_DISTRIBUTION_EXE to the actual release executable")
}

/// The PE subsystem controls whether Explorer creates an unwanted console when opening the editor.
#[test]
#[ignore = "requires an actual Windows release executable in NANOBUG_WINDOWS_DISTRIBUTION_EXE"]
fn release_executable_uses_gui_subsystem() {
    let bytes = std::fs::read(executable()).unwrap();
    assert_eq!(&bytes[..2], b"MZ");
    let pe = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap()) as usize;
    assert_eq!(&bytes[pe..pe + 4], b"PE\0\0");
    // Subsystem has the same offset in PE32 and PE32+ optional headers, after the COFF header.
    let offset = pe + 24 + 68;
    let subsystem = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
    assert_eq!(
        subsystem, 2,
        "the shipped editor must be a Windows GUI application"
    );
}

/// GUI linking must preserve machine-readable output when a caller supplies redirected standard handles.
#[test]
#[ignore = "requires the release executable and NANOBUG_WINDOWS_SMOKE_PACKAGE"]
fn release_cli_preserves_redirected_json_output() {
    let package = std::env::var_os("NANOBUG_WINDOWS_SMOKE_PACKAGE")
        .expect("set NANOBUG_WINDOWS_SMOKE_PACKAGE to a valid independently built plugin ZIP");
    let output = Command::new(executable())
        .arg("--plugin-check")
        .arg(package)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = String::from_utf8(output.stdout).unwrap();
    assert!(report.trim_start().starts_with('{'));
    assert!(report.contains("\"passed\":true"), "{report}");
    assert!(report.trim_end().ends_with('}'));
}

/// Invalid CLI usage must still return a failure code and a diagnostic through redirected stderr.
#[test]
#[ignore = "requires an actual Windows release executable in NANOBUG_WINDOWS_DISTRIBUTION_EXE"]
fn release_cli_preserves_failure_exit_and_stderr() {
    let output = Command::new(executable())
        .arg("--export-plugin-sdk")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--export-plugin-sdk"));
}
