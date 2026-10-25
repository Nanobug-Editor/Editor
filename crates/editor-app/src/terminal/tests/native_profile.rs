//! Prepare a parent-owned isolated profile for actual Windows input/IME and scroll inspection.

use super::*;

/// Only explicit temporary roots are written; the product consumes ordinary version-1 terminal data.
#[test]
#[ignore = "set NANOBUG_TERMINAL_QA_ROOT and its ME_EDITOR_PROFILE_HOME to a new temporary profile"]
fn prepare_terminal_native_acceptance_profile() {
    let root =
        PathBuf::from(std::env::var_os("NANOBUG_TERMINAL_QA_ROOT").expect("explicit QA root"));
    assert!(root.is_absolute() && root.starts_with(std::env::temp_dir()));
    let profile = root.join("profile");
    assert_eq!(
        std::env::var_os("ME_EDITOR_PROFILE_HOME").map(PathBuf::from),
        Some(profile.clone())
    );
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&profile).unwrap();
    std::fs::write(
        workspace.join("native-terminal.txt"),
        "native terminal link destination\nsecond line\nthird line\n",
    )
    .unwrap();
    let workspace = workspace.canonicalize().unwrap();
    let mut state = SessionState::for_workspace(&workspace);
    state.workspace_trusted = true;
    state.native_terminal_visible = true;
    state.window_width = 1200.;
    state.window_height = 800.;
    assert!(
        !state.file_path().unwrap().exists(),
        "a QA preparation must never overwrite an existing profile"
    );
    state.try_save().unwrap();
    let settings = Settings::default();
    let mut engine = Engine::new(
        GridSize {
            columns: 100,
            rows: 12,
        },
        settings.history,
    );
    for index in 0..80 {
        engine.process(format!("history {index:02} 中文 😀 \x1b[3mitalic\x1b[0m\r\n").as_bytes());
    }
    engine.process(b"native-terminal.txt:2:1 https://example.com/docs\r\n");
    let saved = persistence::Saved {
        version: 1,
        settings: settings.clone(),
        active: Some(1),
        next_id: 1,
        tab_width: 180.,
        panes: None,
        sessions: vec![persistence::SavedSession {
            id: 1,
            name: "Native QA".into(),
            profile: settings.profiles[0].clone(),
            cwd: workspace.to_string_lossy().into_owned(),
            exited: false,
            grid: engine.snapshot(),
            task: None,
        }],
    };
    let storage = persistence::directory_in_profile(&workspace, &profile);
    std::fs::create_dir_all(&storage).unwrap();
    std::fs::write(
        storage.join("state.json"),
        serde_json::to_vec(&saved).unwrap(),
    )
    .unwrap();
    std::fs::write(root.join("acceptance.json"), serde_json::to_vec_pretty(&serde_json::json!({ "workspace": workspace, "profile": profile, "terminal_storage": storage })).unwrap()).unwrap();
}
