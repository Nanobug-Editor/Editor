//! Current public configuration fixtures retain launch/env/preparation coverage without legacy admission.

/// Install the independently SDK-built test provider through the normal permission-checked Manager.
pub(super) fn install(manager: &mut plugin_runtime::Manager) {
    let package = crate::test_packages::package("sdk-launch-fixture");
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
}

/// Preserve every literal target field in opaque provider values; validation recomputes this projection.
pub(super) fn accept(controls: &mut crate::run::RunControls, config: editor_core::RunConfig) {
    let launch = serde_json::json!({
        "target":config.target,"directory":config.directory,"env":config.env,
        "tool_paths":config.tool_paths,"build":config.build,"prelaunch":config.prelaunch,"provider":config.provider
    });
    let data = editor_core::PluginConfiguration {
        provider: "sdk-launch-fixture".into(),
        template: "literal".into(),
        values: launch.to_string(),
        pending_events: vec![],
        name: config.name.clone(),
        program: launch["target"]["program"]
            .as_str()
            .unwrap_or("literal target")
            .into(),
        revision: 0,
        validation: editor_core::ConfigurationValidation::Valid,
    };
    controls
        .accept_configuration_projection(config, data)
        .unwrap();
}
