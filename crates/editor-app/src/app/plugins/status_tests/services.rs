//! Public multi-service declarations drive distinct footer counts and independent viewing acknowledgements.
use super::*;
use crate::app::language_servers::ServiceLoadState;
use gpui_base::test_support::snapshots;
use gpui_kit::Role;

/// Two declared native services have one plugin owner; neither fixture executable is launched here.
fn package_with_services() -> Package {
    let manifest = serde_json::json!({
        "id":"parallel-services", "name":"Parallel services", "version":"1.0.0", "protocol":7,
        "api":{"base":"^1","required":{"language.lsp":"^1","process":"^1"}},
        "contributions":"plugin.toml", "storage_limit":1024,
        "services":{"alpha":{"program":std::env::current_exe().unwrap()},"beta":{"program":std::env::current_exe().unwrap()}},
        "permissions":["process.service.alpha","process.service.beta"],
        "language_servers":[{"id":"alpha","language":"alpha-fixture","service":"alpha"},{"id":"beta","language":"beta-fixture","service":"beta"}]
    });
    let declaration = b"[plugin]\nid = \"parallel-services\"\nname = \"Parallel services\"\nversion = \"1.0.0\"\nhost_version = \"^0.1\"\n[[language_definitions]]\nid = \"alpha-fixture\"\nname = \"Alpha fixture\"\nextensions = [\"alpha-fixture\"]\n[[language_definitions]]\nid = \"beta-fixture\"\nname = \"Beta fixture\"\nextensions = [\"beta-fixture\"]\n";
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in [
        ("manifest.json", serde_json::to_vec(&manifest).unwrap()),
        ("plugin.toml", declaration.to_vec()),
    ] {
        archive
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(&bytes).unwrap();
    }
    Package::from_bytes(&archive.finish().unwrap().into_inner()).unwrap()
}

/// Read the labels actually painted by native controls, including status rows and footer buttons.
fn labels(form: &mut VisualTestContext, role: Role) -> Vec<String> {
    form.update(|window, _| {
        snapshots(window)
            .into_iter()
            .filter(|node| node.visible() && node.role() == Some(role))
            .filter_map(|node| node.label().map(str::to_owned))
            .collect()
    })
}

/// Independent service identities remain visible, update live and never acknowledge unrelated plugin logs.
#[gpui::test]
fn service_status_lists_each_engine_without_confirming_plugin_alerts(cx: &mut TestAppContext) {
    with_status(cx, 1, "en", false, |form, app, manager| {
        let package = package_with_services();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        crate::extensions::lsp_tests::publish(app, manager, form);
        let root = form.update(|_, cx| app.read(cx).workspace.root().to_owned());
        for extension in ["alpha-fixture", "beta-fixture"] {
            let path = root.join(format!("loading.{extension}"));
            std::fs::write(&path, "fixture").unwrap();
            form.update(|window, cx| app.update(cx, |app, cx| app.open_file(path, window, cx)));
        }
        draw(form);
        let servers = crate::extensions::lsp_tests::publish_with(
            app,
            manager,
            form,
            |app, manager, _, cx| {
                let plans = manager.language_services();
                let servers = ["alpha", "beta"].map(|name| {
                    let plan = plans[&format!("parallel-services/{name}")]
                        .as_ref()
                        .unwrap()
                        .clone();
                    let server =
                        Arc::new(language_navigation::LanguageServer::from_service(plan).unwrap());
                    app.language_servers
                        .insert(format!("{name}-fixture"), server.clone());
                    app.language_service_states
                        .insert(format!("{name}-fixture"), ServiceLoadState::Loading);
                    server
                });
                app.extensions.update(cx, |panel, _| {
                    panel
                        .startup
                        .insert("status-00".into(), "Resource plugin".into());
                });
                cx.notify();
                servers
            },
        );
        draw(form);
        assert!(labels(form, Role::Button).contains(&"Loading plugins 1".to_owned()));
        assert!(labels(form, Role::Button).contains(&"Starting services 2".to_owned()));
        let services = form.debug_bounds("service-starting-indicator").unwrap();
        let plugins = form.debug_bounds("plugin-loading-indicator").unwrap();
        assert!(services.right() <= plugins.left());

        let logs = manager.runtime_logs();
        logs.append(
            "status-00",
            LogLevel::Warning,
            "plugin.stderr",
            "Unconfirmed plugin warning",
        );
        publish(form, app);
        click(form, "service-starting-indicator");
        assert!(form.debug_bounds("service-status-popup").is_some());
        assert!(form.debug_bounds("plugin-status-popup").is_none());
        let names = labels(form, Role::Button);
        assert!(names.contains(&"View runtime log · parallel-services/alpha".to_owned()));
        assert!(names.contains(&"View runtime log · parallel-services/beta".to_owned()));
        assert_eq!(
            logs.pending_reminders(),
            vec![("status-00".into(), LogLevel::Warning)]
        );
        assert_eq!(logs.unread_severity("status-00"), Some(LogLevel::Warning));
        assert!(form.debug_bounds("plugin-warning-indicator").is_some());

        form.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.finish_server_loading("alpha-fixture", &servers[0], Ok(()), cx)
            })
        });
        draw(form);
        assert!(labels(form, Role::Button).contains(&"Starting services 1".to_owned()));
        assert!(
            !labels(form, Role::Button)
                .contains(&"View runtime log · parallel-services/alpha".to_owned())
        );
        assert!(
            labels(form, Role::Button)
                .contains(&"View runtime log · parallel-services/beta".to_owned())
        );
        form.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.finish_server_loading(
                    "beta-fixture",
                    &servers[1],
                    Err(anyhow::anyhow!("fixture startup failed")),
                    cx,
                )
            })
        });
        draw(form);
        assert!(form.debug_bounds("service-starting-indicator").is_none());
        assert!(form.debug_bounds("service-status-popup").is_none());
        assert!(form.update(|_, cx| app.read(cx).plugin_popup_snapshot.is_none()));
        assert!(form.debug_bounds("plugin-warning-indicator").is_some());
        click(form, "plugin-warning-indicator");
        assert!(form.debug_bounds("plugin-loading-indicator").is_some());
        assert!(logs.pending_reminders().is_empty());
    });
}
