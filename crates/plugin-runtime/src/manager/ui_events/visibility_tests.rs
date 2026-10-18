//! Visibility admission is opt-in and cannot borrow another panel, consent or workspace authority.
use super::*;

/// A second identity can consume the same general contract without feature-specific host hooks.
fn installed() -> Installed {
    let manifest: Manifest = serde_json::from_value(serde_json::json!({
        "id":"independent.visibility","name":"Visibility","version":"1.0.0","protocol":7,
        "permissions":["ui.panels"],"storage_limit":1024,
        "api":{"base":"^1","required":{"ui.visibility":"^1"}},
        "panels":[{"id":"first","title":"First","position":"bottom"}]
    }))
    .unwrap();
    Installed {
        grants: manifest.permissions.clone(),
        manifest,
        digest: String::new(),
        enabled: true,
        project_enabled: Default::default(),
        replaced_by: None,
        global_enabled: None,
        retired_ui_contract: false,
        error: None,
    }
}

/// Native observations cannot route a peer panel or inherit missing grants from host code.
#[test]
fn panel_visibility_requires_ownership_consent_trust_and_capability() {
    let mut entry = installed();
    assert!(validate_panel_visibility(&entry, Some("first"), "first", true, true).is_ok());
    assert_eq!(
        validate_panel_visibility(&entry, None, "first", true, true)
            .unwrap_err()
            .code,
        api::ErrorCode::InvalidRequest
    );
    assert_eq!(
        validate_panel_visibility(&entry, Some("second"), "second", true, true)
            .unwrap_err()
            .code,
        api::ErrorCode::PermissionDenied
    );
    for (trusted, open) in [(false, true), (true, false)] {
        assert_eq!(
            validate_panel_visibility(&entry, Some("first"), "first", trusted, open)
                .unwrap_err()
                .code,
            api::ErrorCode::PermissionDenied
        );
    }
    entry.grants.clear();
    assert_eq!(
        validate_panel_visibility(&entry, Some("first"), "first", true, true)
            .unwrap_err()
            .code,
        api::ErrorCode::PermissionDenied
    );
    entry.grants = entry.manifest.permissions.clone();
    entry.manifest.api.as_mut().unwrap().required.clear();
    assert_eq!(
        validate_panel_visibility(&entry, Some("first"), "first", true, true)
            .unwrap_err()
            .code,
        api::ErrorCode::CapabilityUnavailable
    );
}

/// Metadata plus a true observation must not cross the normal cold-demand activation boundary.
#[test]
fn panel_visibility_never_activates_an_installed_cold_owner() {
    let directory = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(
        directory.path().into(),
        Environment {
            workspace: directory.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    let entry = installed();
    let id = entry.manifest.id.clone();
    manager.installed.insert(id.clone(), entry);
    let error = manager
        .event(
            &id,
            Some("first".into()),
            api::Notification::PanelVisibility {
                panel: "first".into(),
                visible: true,
            },
        )
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<api::Failure>().unwrap().code,
        api::ErrorCode::InvalidState
    );
    assert!(manager.live.is_empty());
}
