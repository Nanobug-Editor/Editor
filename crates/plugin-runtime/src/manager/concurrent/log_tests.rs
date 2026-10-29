//! Public workspace routing preserves delayed producer attribution across parked and profile scopes.

use super::*;

/// Live clones and native reporters keep their origin when another actor selects the shared manager.
#[test]
fn unified_messages_manager_routes_new_logs_without_retargeting_parked_producers() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first");
    let second = directory.path().join("second");
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(&second).unwrap();
    let first = first.canonicalize().unwrap().to_string_lossy().into_owned();
    let second = second
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let mut manager = Manager::open_with_trust(
        directory.path().join("runtime"),
        Environment {
            workspace: first.clone(),
            ..Default::default()
        },
        false,
    )
    .unwrap();
    let first_logs = manager.runtime_logs();
    let reporter = manager.native_diagnostics.reporter("plugin", "first");
    manager
        .select_workspace(
            Environment {
                workspace: second.clone(),
                ..Default::default()
            },
            false,
        )
        .unwrap();
    manager
        .runtime_logs()
        .append("plugin", crate::LogLevel::Info, "worker", "second");
    reporter.record("retirement", "first delayed native failure".into());
    first_logs.append(
        "plugin",
        crate::LogLevel::Warning,
        "language",
        "first delayed service",
    );
    manager.select_application(Environment::default());
    manager
        .runtime_logs()
        .append("installer", crate::LogLevel::Info, "package", "application");
    manager
        .select_workspace(
            Environment {
                workspace: first.clone(),
                ..Default::default()
            },
            false,
        )
        .unwrap();
    manager
        .runtime_logs()
        .append("plugin", crate::LogLevel::Info, "worker", "first restored");
    let first_records = manager.runtime_logs().records_since(0, &first).1;
    assert_eq!(first_records.len(), 4);
    assert!(
        first_records
            .iter()
            .all(|record| record.message != "second")
    );
    assert_eq!(first_records[0].workspace.as_deref(), Some(first.as_str()));
    let second_records = manager.runtime_logs().records_since(0, &second).1;
    assert_eq!(
        second_records
            .iter()
            .map(|record| record.message.as_str())
            .collect::<Vec<_>>(),
        vec!["second", "application"]
    );
}
