//! Source-free native delivery keeps explicit import, trust, providers and historical choices observable.
use super::*;
mod fixture;
use fixture::{Fresh, archive, catalog, delivered, independent_preview};

/// Opening a recognized suffix keeps plain-text authority without recommending a package, even beside old bundle files.
#[gpui::test]
fn fresh_official_file_stays_plain_text_without_automatic_install_consent(cx: &mut TestAppContext) {
    let bytes = std::fs::read(crate::test_packages::archive("toml")).unwrap();
    let (mut fixture, ui) = Fresh::mount(cx, true, &bytes, "toml");
    fixture.open("notes.toml", ui);
    assert!(
        !fixture.consent(ui),
        "fresh installations never recommend shipped packages"
    );
    assert!(fixture.native.manager.installed.is_empty() && fixture.native.manager.live.is_empty());
    assert!(ui.debug_bounds("editor-source-toolbar").is_none());
    assert_eq!(
        ui.update(|_, cx| fixture
            .native
            .app
            .read(cx)
            .editor
            .read(cx)
            .language_name()
            .to_string()),
        "text"
    );
}

/// An explicit import requires real consent, publishes grammar/preview and preserves a disabled preference on restart.
#[gpui::test]
#[ignore = "opt-in actual reviewed Markdown WASM and native lifecycle"]
fn fresh_markdown_explicit_import_retains_disabled_choice(cx: &mut TestAppContext) {
    let bytes = delivered();
    let (mut fixture, ui) = Fresh::mount(cx, true, &bytes, "md");
    fixture.open("notes.md", ui);
    assert!(!fixture.consent(ui));
    fixture.request_import(&bytes, ui);
    assert!(fixture.consent(ui));
    assert!(fixture.native.manager.installed.is_empty() && fixture.native.manager.live.is_empty());
    assert!(ui.debug_bounds("editor-plugin-layout").is_none());
    fixture.click_consent("plugin-install-confirm", ui);
    fixture.settle(ui);
    assert_eq!(
        fixture.native.manager.installed["markdown"]
            .manifest
            .version,
        "0.17.1"
    );
    assert!(ui.debug_bounds("editor-plugin-layout").is_some());
    assert!(ui.debug_bounds("editor-source-toolbar").is_some());
    assert!(
        ui.debug_bounds("plugin-tool-markdown/preview/display-sync")
            .is_some()
    );
    assert_eq!(
        ui.update(|_, cx| fixture
            .native
            .app
            .read(cx)
            .editor
            .read(cx)
            .language_name()
            .to_string()),
        "markdown"
    );
    fixture.native.manager.disable("markdown").unwrap();
    fixture.settle(ui);
    assert!(ui.debug_bounds("editor-plugin-layout").is_none());
    let (mut fixture, ui) = fixture.reopen(cx);
    fixture.open("notes.md", ui);
    assert!(!fixture.native.manager.installed["markdown"].enabled);
    assert!(!fixture.native.manager.live.contains_key("markdown"));
    assert!(!fixture.consent(ui));
}

/// Cancelling an explicit import creates no installation or automatic retry when old bundle bytes change.
#[gpui::test]
#[ignore = "opt-in actual reviewed Markdown native consent"]
fn fresh_markdown_cancel_survives_restart_and_changed_legacy_bundle(cx: &mut TestAppContext) {
    let bytes = delivered();
    let (mut fixture, ui) = Fresh::mount(cx, true, &bytes, "md");
    fixture.open("notes.md", ui);
    fixture.request_import(&bytes, ui);
    fixture.click_consent("plugin-install-cancel", ui);
    fixture.settle(ui);
    assert!(fixture.native.manager.installed.is_empty());
    let mut files = Package::from_bytes(&bytes).unwrap().files;
    files.insert(
        "README.md".into(),
        b"Changed old bundle; opening a file never authorizes it.".to_vec(),
    );
    let changed = language_tests::packages::repack(files).unwrap();
    assert_ne!(changed.digest, Package::from_bytes(&bytes).unwrap().digest);
    catalog(fixture.native.directory.path(), &archive(&changed), "md");
    let (mut fixture, ui) = fixture.reopen(cx);
    fixture.open("notes.md", ui);
    assert!(!fixture.consent(ui));
    assert!(fixture.native.manager.installed.is_empty() && fixture.native.manager.live.is_empty());
}

/// Trust withdrawal before processing an accepted import prevents execution and language publication.
#[gpui::test]
#[ignore = "opt-in actual reviewed Markdown and trust withdrawal"]
fn fresh_markdown_restricted_workspace_and_revoked_consent_never_execute_guest(
    cx: &mut TestAppContext,
) {
    let bytes = delivered();
    let (mut fixture, ui) = Fresh::mount(cx, false, &bytes, "md");
    fixture.open("notes.md", ui);
    assert!(!fixture.consent(ui));
    ui.update(|_, cx| {
        fixture
            .native
            .app
            .update(cx, |app, cx| app.set_workspace_trusted(true, cx))
    });
    fixture.settle(ui);
    assert!(
        !fixture.consent(ui),
        "lifting trust does not recommend a plugin"
    );
    fixture.request_import(&bytes, ui);
    fixture.click_consent("plugin-install-confirm", ui);
    ui.update(|_, cx| {
        fixture
            .native
            .app
            .update(cx, |app, cx| app.set_workspace_trusted(false, cx))
    });
    fixture.settle(ui);
    assert!(fixture.native.manager.installed.is_empty() && fixture.native.manager.live.is_empty());
    assert!(ui.debug_bounds("editor-plugin-layout").is_none());
}

/// Switching or closing an active file never gives its old bundle implicit installation authority.
#[gpui::test]
fn fresh_markdown_switch_and_close_never_queue_automatic_installation(cx: &mut TestAppContext) {
    for close in [false, true] {
        let (mut fixture, ui) = Fresh::mount(cx, true, &delivered(), "md");
        fixture.open("notes.md", ui);
        ui.update(|window, cx| {
            fixture.native.app.update(cx, |app, cx| {
                if close {
                    app.close_tab(app.active_path.clone().unwrap(), window, cx);
                } else {
                    app.open_file(
                        fixture.native.directory.path().join("other.txt"),
                        window,
                        cx,
                    );
                }
            })
        });
        fixture.settle(ui);
        assert!(!fixture.consent(ui));
        assert!(
            fixture.native.manager.installed.is_empty() && fixture.native.manager.live.is_empty()
        );
        assert!(ui.debug_bounds("editor-plugin-layout").is_none());
    }
}

/// An independently named language provider stays selected instead of acquiring the former default.
#[gpui::test]
fn fresh_markdown_keeps_alternative_source_provider(cx: &mut TestAppContext) {
    let (mut fixture, ui) = Fresh::mount(cx, true, &delivered(), "md");
    let mut files = language_package().files;
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["id"] = serde_json::json!("alternate-prose");
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let declaration = String::from_utf8(files["plugin.toml"].clone())
        .unwrap()
        .replacen("id = \"markdown\"", "id = \"alternate-prose\"", 1);
    files.insert("plugin.toml".into(), declaration.into_bytes());
    let alternate = language_tests::packages::repack(files).unwrap();
    fixture
        .native
        .manager
        .install(&alternate, alternate.manifest.permissions.clone())
        .unwrap();
    language_tests::publish_languages(&fixture.native.app, &fixture.native.manager, ui);
    fixture.open("notes.md", ui);
    // The fixture's recording transport has no actor; publish its real Manager's resources after file activation too.
    language_tests::publish_languages(&fixture.native.app, &fixture.native.manager, ui);
    assert!(!fixture.consent(ui));
    assert!(!fixture.native.manager.installed.contains_key("markdown"));
    assert!(fixture.native.manager.installed["alternate-prose"].enabled);
    assert_eq!(
        ui.update(|_, cx| fixture
            .native
            .app
            .read(cx)
            .editor
            .read(cx)
            .language_name()
            .to_string()),
        "markdown"
    );
}

/// Returning after an inactive file cannot turn an old catalog into consent; explicit import remains available.
#[gpui::test]
#[ignore = "opt-in actual reviewed Markdown after source switching"]
fn fresh_markdown_return_requires_an_explicit_import(cx: &mut TestAppContext) {
    let bytes = delivered();
    let (mut fixture, ui) = Fresh::mount(cx, true, &bytes, "md");
    fixture.open("notes.md", ui);
    fixture.open("other.txt", ui);
    fixture.open("notes.md", ui);
    assert!(!fixture.consent(ui));
    assert!(fixture.native.manager.installed.is_empty());
    fixture.request_import(&bytes, ui);
    fixture.click_consent("plugin-install-confirm", ui);
    fixture.settle(ui);
    assert!(ui.debug_bounds("editor-plugin-layout").is_some());
}

/// Corrupt old recommendation metadata is ignored and cannot trigger consent, execution or fabricated status.
#[gpui::test]
fn fresh_markdown_bad_legacy_catalog_or_choice_metadata_never_offers_execution(
    cx: &mut TestAppContext,
) {
    for bad_choices in [false, true] {
        let (mut fixture, ui) = Fresh::mount(cx, true, &delivered(), "md");
        let root = fixture.native.directory.path();
        if bad_choices {
            std::fs::write(
                root.join(".runtime-plugin-test/bundle-choices.json"),
                b"{\"version\":99,\"entries\":{}}",
            )
            .unwrap();
        } else {
            std::fs::write(
                root.join(".bundled-plugin-test/bundle-defaults.json"),
                b"{invalid}",
            )
            .unwrap();
        }
        fixture.open("notes.md", ui);
        assert!(!fixture.consent(ui));
        assert!(
            fixture.native.manager.installed.is_empty() && fixture.native.manager.live.is_empty()
        );
        assert!(ui.debug_bounds("editor-source-toolbar").is_none());
        assert!(ui.debug_bounds("plugin-error-indicator").is_none());
    }
}

/// An opaque public-SDK preview enters only by explicit consent and uninstall never offers it again.
#[gpui::test]
#[ignore = "opt-in actual frozen public-SDK preview and native lifecycle"]
fn explicit_import_installs_an_independent_sdk_preview_with_an_opaque_identity(
    cx: &mut TestAppContext,
) {
    let package = independent_preview("opaque");
    let bytes = archive(&package);
    let (mut fixture, ui) = Fresh::mount(cx, true, &bytes, "opaque");
    fixture.open("notes.opaque", ui);
    assert!(!fixture.consent(ui));
    fixture.request_import(&bytes, ui);
    assert!(fixture.native.manager.live.is_empty());
    fixture.click_consent("plugin-install-confirm", ui);
    fixture.settle(ui);
    assert!(
        fixture
            .native
            .manager
            .installed
            .contains_key("bundled-opaque-preview")
    );
    assert!(!fixture.native.manager.installed.contains_key("markdown"));
    assert!(ui.debug_bounds("plugin-ui-bundle-greeting").is_some());
    assert!(ui.debug_bounds("editor-plugin-layout").is_some());
    fixture
        .native
        .manager
        .uninstall("bundled-opaque-preview", true)
        .unwrap();
    fixture.settle(ui);
    assert!(ui.debug_bounds("plugin-ui-bundle-greeting").is_none());
    let (mut fixture, ui) = fixture.reopen(cx);
    fixture.open("notes.opaque", ui);
    assert!(!fixture.consent(ui));
    assert!(fixture.native.manager.installed.is_empty());
}

/// Existing enabled and disabled previews keep their persisted selection across an editor restart.
#[gpui::test]
#[ignore = "opt-in actual frozen public-SDK preview and retained data"]
fn fresh_markdown_keeps_an_independent_preview_provider_across_restart(cx: &mut TestAppContext) {
    for disabled in [false, true] {
        let (mut fixture, ui) = Fresh::mount(cx, true, &delivered(), "md");
        let alternative = independent_preview("md");
        fixture
            .native
            .manager
            .install(&alternative, alternative.manifest.permissions.clone())
            .unwrap();
        fixture.settle(ui);
        fixture.open("notes.md", ui);
        assert!(!fixture.consent(ui));
        assert!(!fixture.native.manager.installed.contains_key("markdown"));
        assert!(ui.debug_bounds("plugin-ui-bundle-greeting").is_some());
        if disabled {
            fixture
                .native
                .manager
                .disable("bundled-opaque-preview")
                .unwrap();
            fixture.settle(ui);
            assert!(ui.debug_bounds("editor-plugin-layout").is_none());
        }
        let (mut fixture, ui) = fixture.reopen(cx);
        fixture.open("notes.md", ui);
        assert!(!fixture.consent(ui));
        assert!(!fixture.native.manager.installed.contains_key("markdown"));
        assert_eq!(
            fixture.native.manager.installed["bundled-opaque-preview"].enabled,
            !disabled
        );
        assert_eq!(
            ui.debug_bounds("plugin-ui-bundle-greeting").is_some(),
            !disabled
        );
    }
}

/// A→B→A in one frame cannot revive automatic consent; the user's following explicit action can still install.
#[gpui::test]
#[ignore = "opt-in actual reviewed Markdown after same-frame tab activation"]
fn fresh_markdown_same_frame_return_waits_for_explicit_consent(cx: &mut TestAppContext) {
    let bytes = delivered();
    let (mut fixture, ui) = Fresh::mount(cx, true, &bytes, "md");
    fixture.open("notes.md", ui);
    ui.update(|window, cx| {
        fixture.native.app.update(cx, |app, cx| {
            app.open_file(
                fixture.native.directory.path().join("other.txt"),
                window,
                cx,
            );
            app.open_file(fixture.native.directory.path().join("notes.md"), window, cx);
        })
    });
    fixture.settle(ui);
    assert!(!fixture.consent(ui));
    assert!(fixture.native.manager.installed.is_empty());
    fixture.request_import(&bytes, ui);
    fixture.click_consent("plugin-install-confirm", ui);
    fixture.settle(ui);
    assert!(ui.debug_bounds("editor-plugin-layout").is_some());
}
