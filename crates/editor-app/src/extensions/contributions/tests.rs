//! Packaged declarations enter through the same bounded immutable loader as native views.
use super::*;

/// A project's service failure cannot withdraw a globally installed user's immutable theme from another project.
#[gpui_kit::gpui::test]
fn a_project_failure_preserves_the_shared_user_theme(cx: &mut gpui_kit::TestAppContext) {
    let store = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let original = crate::extensions::language_tests::packages::language_package("mixed-resources");
    let mut files = original.files;
    files
        .get_mut("plugin.toml")
        .unwrap()
        .extend_from_slice(b"\n[theme]\nfile = \"theme.json\"\n");
    let mut palette = crate::theme::builtin_theme(false).clone();
    palette.name = "Shared user palette".into();
    files.insert(
        "theme.json".into(),
        serde_json::to_vec(&ThemeFile {
            schema_version: 1,
            themes: vec![palette],
        })
        .unwrap(),
    );
    let package = crate::extensions::language_tests::packages::repack(files).unwrap();
    let mut manager = Manager::open(store.path().into(), Default::default()).unwrap();
    manager.install(&package, Default::default()).unwrap();
    let mut entries = manager.published_entries();
    let first = crate::language::providers::Providers::new(store.path(), a.path());
    let second = crate::language::providers::Providers::new(store.path(), b.path());
    // Both native project views borrow the same application-owned user resource authority.
    let (_first_catalog, _second_catalog) =
        cx.update(|cx| (CatalogView::new(&first, cx), CatalogView::new(&second, cx)));
    refresh_project_entries(&second, store.path(), &entries, true);
    assert_eq!(theme(false).unwrap().name, "Shared user palette");
    entries[0].error = Some("A workspace provider failed".into());
    refresh_project_entries(&first, store.path(), &entries, true);
    assert_eq!(theme(false).unwrap().name, "Shared user palette");
    assert!(
        first
            .language_for_path(&a.path().join("source.novel"))
            .is_none()
    );
    assert_eq!(
        second
            .language_for_path(&b.path().join("source.novel"))
            .as_deref(),
        Some("novel")
    );
    // Only an actual global package disable withdraws the user resource from every project.
    manager.disable("mixed-resources").unwrap();
    refresh_project_entries(&second, store.path(), &manager.published_entries(), true);
    assert!(theme(false).is_none());
}

/// A differently named resource-only provider uses the same catalog and stable source identity.
#[test]
fn an_independent_declaration_offers_distinct_sources_without_language_rules() {
    let example = Contribution::read(
        &crate::test_packages::root("run-target-example"),
        "plugin.toml",
        "run-target-example",
        &"b".repeat(64),
    )
    .unwrap();
    assert_eq!(example.run_targets.len(), 1);
    let project = tempfile::tempdir().unwrap();
    for directory in ["one", "two"] {
        std::fs::create_dir_all(project.path().join(directory)).unwrap();
        std::fs::write(
            project.path().join(directory).join("native-tool.toml"),
            "[tool]\nname=\"Same label\"\nprogram=\"tool.exe\"\n",
        )
        .unwrap();
    }
    let mut catalog = Catalog::default();
    catalog.plugins.insert("run-target-example".into(), example);
    *CATALOG.write().unwrap() = Arc::new(catalog);
    let targets = discover_run_targets(project.path()).unwrap();
    assert_eq!(targets.len(), 2);
    assert_ne!(
        targets[0].id, targets[1].id,
        "equal labels in different files are different targets"
    );
    assert!(
        targets
            .iter()
            .all(|target| target.provider == "run-target-example"
                && target.target_type == "native-tool"
                && target.program == "tool.exe")
    );
    assert!(
        discover_run_targets(tempfile::tempdir().unwrap().path())
            .unwrap()
            .is_empty()
    );
    *CATALOG.write().unwrap() = Arc::new(Catalog::default());
}

/// A declaration that disagrees with its contribution file is reported, not silently accepted.
#[test]
fn a_mismatched_discovery_identity_is_refused() {
    assert!(declaration_matches_contribution("rust-binary", "rust-binary").is_ok());
    let error = declaration_matches_contribution("declared", "other")
        .expect_err("a declaration naming another provider is refused");
    assert!(
        error.contains("other") && error.contains("declared"),
        "{error}"
    );
    // A contribution whose file is absent is reported with the file it named.
    let directory = tempfile::tempdir().unwrap();
    let (providers, failures) = read_run_targets(
        directory.path(),
        &[plugin_schema::RunTargetContribution {
            id: "declared".into(),
            file: PathBuf::from("missing.json"),
        }],
    );
    assert!(providers.is_empty());
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].provider, "declared");
    // The reason comes from the platform and is not asserted word for word, but a plugin that
    // names a file it does not ship has to be reported rather than quietly offering nothing.
    assert!(!failures[0].error.trim().is_empty());
}
/// Startup resource loading must reject old protocols before opening their grammar/theme/icon assets.
#[test]
fn incompatible_resource_records_do_not_publish_contributions() {
    let directory = tempfile::tempdir().unwrap();
    let package =
        crate::extensions::language_tests::packages::language_package("me.custom-language");
    let mut manager = Manager::open(directory.path().into(), Default::default()).unwrap();
    manager.install(&package, Default::default()).unwrap();
    let mut entries = manager.published_entries();
    refresh_entries(directory.path(), &entries);
    assert!(
        plugin_root("me.custom-language").is_some(),
        "current author ID must remain exact"
    );
    entries[0].manifest.protocol = 1;
    entries[0].manifest.api = None;
    refresh_entries(directory.path(), &entries);
    assert!(plugin_root("me.custom-language").is_none());
    assert!(icon_rules().is_empty());
    assert!(
        entries[0].enabled,
        "filtering must not clear the persisted preference"
    );
}

/// JavaScript aliases resolve from the installed manifest and carry valid themed icons.
#[test]
fn javascript_package_resolves_extensions_and_icons() {
    let javascript = Contribution::read(
        &crate::test_packages::root("javascript"),
        "plugin.toml",
        "javascript",
        &"a".repeat(64),
    )
    .unwrap();
    assert_eq!(javascript.assets.len(), 2);
    let mut catalog = Catalog::default();
    catalog.plugins.insert("javascript".to_owned(), javascript);
    for path in ["main.js", "module.mjs", "legacy.cjs", "View.jsx", "MAIN.JS"] {
        let (_, language) = language_for_path_in_catalog(&catalog, Path::new(path)).unwrap();
        assert_eq!(language.id, "javascript");
        let icons = catalog.plugins["javascript"].icons.as_ref().unwrap();
        let extension = Path::new(path).extension().unwrap().to_str().unwrap();
        assert!(
            icons.icons[0]
                .extensions
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(extension))
        );
    }
    // TypeScript and JSON retain their own language identities.
    for path in ["main.ts", "View.tsx", "package.json"] {
        assert!(language_for_path_in_catalog(&catalog, Path::new(path)).is_none());
    }
    catalog.plugins.clear();
    assert!(language_for_path_in_catalog(&catalog, Path::new("main.js")).is_none());
}

/// Fixed official Package resources retain the declaration, icon and grammar behavior after source removal.
#[test]
fn declarative_language_packages_expose_their_distinct_resources() {
    let digest = "a".repeat(64);
    let rust = Contribution::read(
        &crate::test_packages::root("rust"),
        "plugin.toml",
        "rust",
        &digest,
    )
    .unwrap();
    assert_eq!(rust.manifest.language_definitions[0].id, "rust");
    assert!(!rust.assets.is_empty());
    let toml = Contribution::read(
        &crate::test_packages::root("toml"),
        "plugin.toml",
        "toml",
        &digest,
    )
    .unwrap();
    assert_eq!(toml.manifest.language_definitions[0].id, "toml");
    assert!(rust.theme.is_none() && toml.theme.is_none());

    // Both HTML suffixes resolve to one language and ship valid theme-specific icons.
    let html = Contribution::read(
        &crate::test_packages::root("html"),
        "plugin.toml",
        "html",
        &digest,
    )
    .unwrap();
    assert_eq!(html.assets.len(), 2);
    assert_eq!(
        html.icons.as_ref().unwrap().icons[0].extensions,
        ["html", "htm"]
    );
    let mut html_catalog = Catalog::default();
    html_catalog.plugins.insert("html".to_owned(), html);
    for path in ["index.html", "legacy.htm", "INDEX.HTML", "LEGACY.HTM"] {
        let (owner, language) =
            language_for_path_in_catalog(&html_catalog, Path::new(path)).unwrap();
        assert_eq!(owner, "html");
        assert_eq!(language.id, "html");
    }
    for path in ["view.xhtml", "component.vue", "image.svg", "index.html.txt"] {
        assert!(language_for_path_in_catalog(&html_catalog, Path::new(path)).is_none());
    }

    // Known lockfiles use TOML, while a shared .lock suffix cannot select a grammar.
    let mut catalog = Catalog::default();
    catalog.plugins.insert("toml".to_owned(), toml);
    for path in ["config.toml", "Cargo.lock", "uv.lock", "CARGO.LOCK"] {
        assert_eq!(
            language_for_path_in_catalog(&catalog, Path::new(path))
                .map(|(_, language)| language.id),
            Some("toml".to_owned()),
            "{path} should select the TOML plugin"
        );
    }
    for path in ["composer.lock", "Gemfile.lock", "random.lock"] {
        assert!(
            language_for_path_in_catalog(&catalog, Path::new(path)).is_none(),
            "{path} must not be classified by its .lock suffix"
        );
    }
}
