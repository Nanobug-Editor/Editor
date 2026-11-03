//! Profile consumers run in a separate process so environment selection cannot leak across UI tests.

use crate::*;

/// A selected profile owns configuration, session, plugin and SDK data across their real call sites.
#[test]
fn portable_profile_consumers_share_one_root() {
    let temporary = tempfile::tempdir().unwrap();
    let installed = temporary.path().join("installation");
    std::fs::create_dir(&installed).unwrap();
    let executable = installed.join("portable-probe.exe");
    let current = std::env::current_exe().unwrap();
    std::fs::hard_link(&current, &executable)
        .or_else(|_| std::fs::copy(&current, &executable).map(|_| ()))
        .unwrap();
    for explicit in [false, true] {
        let profile = if explicit {
            temporary.path().join("isolated")
        } else {
            installed.clone()
        };
        let mut child = std::process::Command::new(&executable);
        child
            .args([
                "--exact",
                "tests::portable_data::portable_profile_consumers_child",
                "--ignored",
                "--nocapture",
            ])
            .env_remove("ME_EDITOR_PROFILE_HOME")
            .env_remove("ME_EDITOR_PLUGIN_HOME")
            .current_dir(temporary.path());
        if explicit {
            child.env("ME_EDITOR_PROFILE_HOME", &profile);
        }
        let output = child.output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(profile.join("cache/plugin-sdk").is_dir());
        assert!(profile.join("cache/tmp").is_dir());
        assert!(!installed.join("data").exists());
    }
}

/// Only the parent supplies this isolated profile; no environment variables are mutated in-process.
#[test]
#[ignore = "spawned by portable_profile_consumers_share_one_root with a parent-owned profile"]
fn portable_profile_consumers_child() {
    let profile = editor_core::storage::profile_root().unwrap();
    platform_windows::portable_data::prepare(&profile, None).unwrap();
    assert_eq!(
        app::shortcuts::bootstrap::default_path().unwrap(),
        profile.join("shortcuts.json")
    );
    assert_eq!(editor_core::default_root().unwrap(), profile.join("run"));
    assert_eq!(
        extensions::profile_runtime_root(&profile),
        profile.join("runtime-plugins")
    );
    let session = SessionState::load(Path::new("portable-fixture-workspace"));
    assert!(session.file_path().unwrap().starts_with(&profile));
    let history = platform_windows::history::HistoryPaths::for_profile(None).unwrap();
    assert_eq!(history.root, profile.join("history"));
    assert_eq!(history.settings, profile.join("history.json"));
    assert!(
        plugin_development::profile("workspace", "development")
            .unwrap()
            .starts_with(profile.join("plugin-development"))
    );
    assert!(
        sdk_export::cargo_config()
            .unwrap()
            .starts_with(profile.join("cache/plugin-sdk"))
    );
    assert_eq!(
        editor_core::storage::temporary_root().unwrap(),
        profile.join("cache/tmp")
    );
}
