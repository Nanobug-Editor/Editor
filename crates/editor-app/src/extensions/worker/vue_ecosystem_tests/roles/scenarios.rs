//! Package-only role composition, mapped native edits and invalid-candidate recovery scenarios.
use super::*;

/// A real replacement removes only one optional role, retiring its original mapped native context.
#[gpui::test]
#[ignore = "set real TS/Vue, NANOBUG_ECOSYSTEM_COMBINED_PACKAGE, NANOBUG_ECOSYSTEM_WITHOUT_STYLE_PACKAGE and NANOBUG_VUE_FIXTURES"]
fn four_ecosystem_roles_map_native_queries_edits_replacement_and_removal(cx: &mut TestAppContext) {
    let project = project();
    let path = project.path().join("App.vue").canonicalize().unwrap();
    let disk = std::fs::read_to_string(&path).unwrap();
    let config = std::fs::read_to_string(project.path().join("tsconfig.json")).unwrap();
    let combined = Package::read(&input("NANOBUG_ECOSYSTEM_COMBINED_PACKAGE")).unwrap();
    let replacement = Package::read(&input("NANOBUG_ECOSYSTEM_WITHOUT_STYLE_PACKAGE")).unwrap();
    assert_eq!(combined.manifest.id, replacement.manifest.id);
    assert_eq!(combined.manifest.version, "0.1.2");
    assert_eq!(replacement.manifest.version, "0.1.3");
    let id = combined.manifest.id.clone();
    let (fixture, visual) = setup_for_workspace(
        cx,
        &[
            "NANOBUG_TYPESCRIPT_PACKAGE",
            "NANOBUG_VUE_PACKAGE",
            "NANOBUG_ECOSYSTEM_COMBINED_PACKAGE",
        ],
        input("NANOBUG_VUE_FIXTURES"),
    );
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "cold role catalog",
        30,
        |app, _| {
            app.providers
                .language_servers()
                .get("vue")
                .is_some_and(Option::is_some)
        },
    );
    {
        let state = fixture.actor.state.lock().unwrap();
        assert!(
            state.language_document_bindings.is_empty(),
            "installation must not invent a source binding"
        );
        assert!(
            state
                .language_services
                .values()
                .flatten()
                .all(|plan| plan.process_count() == 0),
            "cold role installation must not start an analysis process"
        );
    }
    open_dirty(&fixture, &path, visual);
    wait_effects(&fixture, visual, true);
    eprintln!("S4 phase: initial four-role original diagnostics PASS");
    let original = capture(&fixture, &path, visual);
    for symbol in ["count", "shade"] {
        eprintln!("S4 phase: initial {symbol} mapped definition");
        original_definition(&fixture, &path, symbol, visual);
        eprintln!("S4 phase: initial {symbol} mapped references");
        references(&fixture, &path, symbol, visual);
        mapped_rename(&fixture, &path, symbol, visual);
        eprintln!("S4 phase: initial {symbol} mapped rename and Undo PASS");
    }
    assert_source(&fixture, visual, &path, &disk, &config);
    // Hold an actual style-mapped native preview across a real contributor replacement. Its
    // original authority must die before any late consent can apply that now-missing mapping.
    pending_mapped_rename(&fixture, &path, "shade", visual);
    fixture.actor.tx.send(Work::Install(replacement)).unwrap();
    wait_retired(&fixture, visual, &original);
    assert_pending_mapping_withdrawn(&fixture, &path, visual);
    wait_effects(&fixture, visual, false);
    let reduced = capture(&fixture, &path, visual);
    assert!(
        !Arc::ptr_eq(&original.0, &reduced.0),
        "replacement must publish a new native view"
    );
    // The same template mapping and script types stay valid when only the style role is missing.
    original_definition(&fixture, &path, "count", visual);
    references(&fixture, &path, "count", visual);
    mapped_rename(&fixture, &path, "count", visual);
    // Uninstall is a separate lifecycle boundary: a pending template-mapped proposal cannot
    // survive merely because TypeScript and Vue themselves remain installed and functional.
    pending_mapped_rename(&fixture, &path, "count", visual);
    fixture.actor.tx.send(Work::Uninstall(id, false)).unwrap();
    wait_retired(&fixture, visual, &reduced);
    assert_pending_mapping_withdrawn(&fixture, &path, visual);
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "unextended native recovery",
        180,
        |app, cx| {
            assert_preparation(app, &fixture.actor);
            app.language_server_for_path(&path)
                .is_some_and(|server| server.is_active())
                && app.editor.read(cx).lsp().hover_provider.is_some()
                && role_effects(app, cx, None)
        },
    );
    actual_script_hover(&fixture, visual);
    assert_source(&fixture, visual, &path, &disk, &config);
    assert_eq!(
        std::fs::read_dir(project.path()).unwrap().count(),
        4,
        "analysis must not generate configuration or helper files in this project"
    );
    shutdown(&fixture, visual);
}

/// Invalid independently installed members fail visibly, then removing them restores real analysis.
#[gpui::test]
#[ignore = "set real TS/Vue/combined and ecosystem CONFLICT/CYCLE/CONFIGURATION_CONFLICT/BAD_MAPPING package inputs"]
fn conflicting_and_irreversible_package_contributions_reject_and_recover(cx: &mut TestAppContext) {
    let project = project();
    let path = project.path().join("App.vue").canonicalize().unwrap();
    let (fixture, visual) = setup_for_workspace(
        cx,
        &[
            "NANOBUG_TYPESCRIPT_PACKAGE",
            "NANOBUG_VUE_PACKAGE",
            "NANOBUG_ECOSYSTEM_COMBINED_PACKAGE",
        ],
        input("NANOBUG_VUE_FIXTURES"),
    );
    open_dirty(&fixture, &path, visual);
    wait_effects(&fixture, visual, true);
    for (input_name, expected) in [
        ("NANOBUG_ECOSYSTEM_CONFLICT_PACKAGE", "exclusive"),
        ("NANOBUG_ECOSYSTEM_CYCLE_PACKAGE", "cycle"),
        (
            "NANOBUG_ECOSYSTEM_CONFIGURATION_CONFLICT_PACKAGE",
            "configuration conflict",
        ),
        ("NANOBUG_ECOSYSTEM_BAD_MAPPING_PACKAGE", "mapping"),
    ] {
        let package = Package::read(&input(input_name)).unwrap();
        let id = package.manifest.id.clone();
        fixture.actor.tx.send(Work::Install(package)).unwrap();
        wait(
            &fixture.app,
            &fixture.actor,
            visual,
            "explicit role preparation failure",
            180,
            |app, _| {
                app.language_service_states.values().any(|state| {
                    if let crate::app::language_servers::ServiceLoadState::Failed(error) = state {
                        let matched = error.to_lowercase().contains(expected);
                        if matched {
                            eprintln!("S4 contribution {input_name}: explicit refusal {error}");
                        }
                        matched
                    } else {
                        false
                    }
                })
            },
        );
        let (rejected_generation, rejected_owner, rejected_errors) = visual.update(|_, cx| {
            let state = fixture.actor.state.lock().unwrap();
            let errors = fixture
                .app
                .read(cx)
                .language_service_states
                .iter()
                .filter_map(|(key, state)| {
                    if let crate::app::language_servers::ServiceLoadState::Failed(error) = state {
                        Some((key.clone(), error.clone()))
                    } else {
                        None
                    }
                })
                .collect();
            (
                state.configuration_revision,
                state.availabilities.get("vue").unwrap().clone(),
                errors,
            )
        });
        assert_eq!(
            visual.update(|_, cx| fixture.app.read(cx).editor.read(cx).text().to_string()),
            SOURCE,
            "a rejected contribution cannot apply a prefix of a mapped edit"
        );
        assert_eq!(
            std::fs::read_to_string(project.path().join("state.ts")).unwrap(),
            STATE_SOURCE
        );
        assert!(visual.debug_bounds("workspace-plan-confirm").is_none());
        fixture
            .actor
            .tx
            .send(Work::Uninstall(id.clone(), false))
            .unwrap();
        recover_after_rejection(
            &fixture,
            &id,
            rejected_generation,
            rejected_owner,
            rejected_errors,
            visual,
        );
        original_definition(&fixture, &path, "count", visual);
    }
    shutdown(&fixture, visual);
}

/// One original recovery budget spans removal publication and fresh native analysis. Only the
/// captured rejected owner/error may remain while its exporter still exists; a new failure fails.
fn recover_after_rejection(
    fixture: &Fixture,
    removed_id: &str,
    rejected_generation: u64,
    rejected_owner: plugin_runtime::Availability,
    rejected_errors: std::collections::BTreeMap<String, String>,
    visual: &mut VisualTestContext,
) {
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut published = false;
    loop {
        wait(
            &fixture.app,
            &fixture.actor,
            visual,
            "role recovery native frame",
            1,
            |_, _| true,
        );
        let ready = visual.update(|window, cx| {
            fixture.app.update(cx, |app, cx| {
                if published {
                    assert_preparation(app, &fixture.actor);
                    return app.editor.read(cx).lsp().definition_provider.is_some()
                        && role_effects(app, cx, Some(true));
                }
                let (removed, generation, owner) = {
                    let state = fixture.actor.state.lock().unwrap();
                    (
                        !state
                            .entries
                            .iter()
                            .any(|entry| entry.manifest.id == removed_id),
                        state.configuration_revision,
                        state.availabilities.get("vue").cloned(),
                    )
                };
                if removed && generation > rejected_generation {
                    eprintln!(
                        "S4 contribution {removed_id}: removal published {rejected_generation}->{generation}; old_owner={rejected_owner:?}; current_owner={owner:?}; native_generation={}; catalog_generation={}; pending_sync={}",
                        app.provider_generation,
                        app.providers.feature_generation(),
                        app.pending_contribution_sync
                    );
                    // Polling publishes the catalog before its deferred native reconciliation.
                    // Use the same explicit production frame boundary as WindowsPlatform tests;
                    // a cached TestAppContext render is not evidence that this publication ran.
                    app.sync_runtime_contributions(window, cx);
                    eprintln!(
                        "S4 contribution {removed_id}: native removal adopted; native_generation={}; catalog_generation={}; services={:?}",
                        app.provider_generation,
                        app.providers.feature_generation(),
                        app.language_service_states
                    );
                    published = true;
                    return false;
                }
                for (key, state) in &app.language_service_states {
                    if let crate::app::language_servers::ServiceLoadState::Failed(error) = state {
                        assert!(
                            owner.as_ref() == Some(&rejected_owner)
                                && rejected_errors.get(key) == Some(error),
                            "new role recovery failure before removal publication: {key}: {error}"
                        );
                    }
                }
                false
            })
        });
        if ready {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "role removal publication and fresh mapped recovery exceeded its original 180s budget"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
