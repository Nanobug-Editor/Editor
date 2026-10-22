//! Native aliases and reparse boundaries cannot broaden a local-history operation's authority.
#![cfg(windows)]

use editor_history::{LegacyFingerprint, LegacyIo};
use platform_windows::history::{FileStamp, NativeLegacyIo, prepare_recovery_parent};

/// Actual filesystem identity, rather than caller case or DOS/verbatim spelling, guards replacement.
#[test]
fn history_file_stamp_recognizes_windows_case_and_path_aliases() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("Unicode-中文.txt");
    std::fs::write(&path, "kept").unwrap();
    let canonical = path.canonicalize().unwrap();
    assert_eq!(
        FileStamp::read(&path).unwrap(),
        FileStamp::read(&canonical).unwrap()
    );
    let case = canonical.parent().unwrap().join("UNICODE-中文.TXT");
    assert_eq!(
        FileStamp::read(&case).unwrap(),
        FileStamp::read(&canonical).unwrap()
    );
    std::fs::rename(&path, directory.path().join("old.txt")).unwrap();
    std::fs::write(&path, "kept").unwrap();
    assert_ne!(
        FileStamp::read(&path).unwrap(),
        FileStamp::read(&directory.path().join("old.txt")).unwrap()
    );
}

/// A junction fixture needs no symlink privilege; only these test-owned temporary paths are passed as data.
#[test]
fn redirected_legacy_bucket_and_recovery_parent_preserve_outside_files() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("history");
    let outside = directory.path().join("outside");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let root = root.canonicalize().unwrap();
    let bucket = root.join("0123456789abcdef");
    std::fs::write(outside.join("1000-note.txt"), "outside").unwrap();
    let status = std::process::Command::new("powershell.exe").args(["-NoProfile", "-NonInteractive", "-Command",
        "New-Item -ItemType Junction -Path $env:NANOBUG_HISTORY_LINK -Target $env:NANOBUG_HISTORY_TARGET -ErrorAction Stop | Out-Null"])
        .env("NANOBUG_HISTORY_LINK", &bucket).env("NANOBUG_HISTORY_TARGET", &outside).status().unwrap();
    assert!(status.success(), "test-owned junction preparation failed");
    let metadata = std::fs::metadata(outside.join("1000-note.txt")).unwrap();
    let expected = LegacyFingerprint {
        size: metadata.len(),
        modified_ns: metadata
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64,
    };
    let relative = std::path::Path::new("0123456789abcdef/1000-note.txt");
    assert!(NativeLegacyIo.open(&root, relative, expected).is_err());
    assert!(NativeLegacyIo.remove(&root, relative, expected).is_err());
    assert!(prepare_recovery_parent(&root, &bucket.join("missing/note.txt")).is_err());
    assert!(!outside.join("missing").exists());
    assert_eq!(
        std::fs::read_to_string(outside.join("1000-note.txt")).unwrap(),
        "outside"
    );
}
