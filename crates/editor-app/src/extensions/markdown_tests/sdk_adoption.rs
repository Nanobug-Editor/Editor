//! The actual independent SDK candidate crosses Manager, native text transactions and hot replacement without restart.
use super::*;
use harness::NativeMarkdown;

/// SDK adoption tests require an explicit new archive; no old frozen package may silently satisfy this seam.
fn candidate() -> Package {
    use sha2::Digest as _;
    let path = std::env::var_os("NANOBUG_MARKDOWN_SDK_PACKAGE").expect("Select the actual new ZIP");
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(
        format!("{:x}", sha2::Sha256::digest(&bytes)),
        std::env::var("NANOBUG_MARKDOWN_SDK_SHA256").unwrap()
    );
    Package::from_bytes(&bytes).unwrap()
}

/// Compatible hot-replacement baseline reuses the real migrated guest under a lower test version.
/// Historical editor.layout 1 bytes remain immutable and must be rejected before guest execution.
fn compatible_baseline(package: &Package) -> Package {
    let legacy = std::fs::read(crate::test_packages::archive("markdown-0.18.2")).unwrap();
    assert!(Package::from_bytes(&legacy).is_err());
    let mut files = package.files.clone();
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["version"] = "0.18.5-test".into();
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let declaration = String::from_utf8(files["plugin.toml"].clone()).unwrap();
    files.insert(
        "plugin.toml".into(),
        declaration
            .replacen(
                &format!("version = \"{}\"", package.manifest.version),
                "version = \"0.18.5-test\"",
                1,
            )
            .into_bytes(),
    );
    Package::from_files(files).unwrap()
}

/// New helpers retain one transaction/Undo, and a failed candidate cannot lose the open unsaved session.
#[gpui::test]
#[ignore = "actual independently built Markdown SDK ZIP; set NANOBUG_MARKDOWN_SDK_PACKAGE and SHA256"]
fn markdown_sdk_adoption_hot_update_retains_document_undo_and_retires_old_resources(
    cx: &mut TestAppContext,
) {
    let package = candidate();
    let old = compatible_baseline(&package);
    let (mut fixture, ui) = NativeMarkdown::mount_package(cx, &[("notes.md", "磁盘正文\n")], &old);
    fixture.open("notes.md", ui);
    fixture.focus_editor(ui);
    ui.simulate_keystrokes("ctrl-a");
    ui.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("未保存中文\n".into())));
    ui.simulate_keystrokes("ctrl-v");
    fixture.settle(ui);
    let process = std::process::id();
    let editor = ui.update(|_, cx| fixture.app.read(cx).editor.clone());
    let before = ui.update(|_, cx| editor.read(cx).text().to_string());
    assert_eq!(before, "未保存中文\n");
    let observations = fixture.manager.runtime_observations();
    observations.start_capture();
    // Keep a real old request in flight, and retain a sentinel in its actual managed private directory.
    let revision = fixture.manager.live["markdown"].views["preview"].revision;
    fixture
        .manager
        .event(
            "markdown",
            Some("preview".into()),
            PluginEvent::Ui(protocol::ui::UiEvent {
                revision,
                node: "format-bold".into(),
                action: protocol::ui::Action::Click,
            }),
        )
        .unwrap();
    let old_request = fixture.manager.take_editor_requests().pop().unwrap().1;
    let sentinel = old_request.data_root().join("sdk-adoption-preserved.txt");
    std::fs::create_dir_all(sentinel.parent().unwrap()).unwrap();
    std::fs::write(&sentinel, "原有受管数据").unwrap();
    // Static package admission succeeds; WASM preparation fails before replacing the live old instance.
    let mut broken = package.files.clone();
    broken.insert(
        "markdown.wasm".into(),
        b"\0asm\x0d\0\x01\0invalid component".to_vec(),
    );
    let broken = Package::from_files(broken).unwrap();
    let original = fixture.manager.live["markdown"].views["preview"].clone();
    assert!(
        fixture
            .manager
            .install(&broken, broken.manifest.permissions.clone())
            .is_err()
    );
    assert!(Arc::ptr_eq(
        &original,
        &fixture.manager.live["markdown"].views["preview"]
    ));
    assert_eq!(std::fs::read_to_string(&sentinel).unwrap(), "原有受管数据");
    fixture
        .manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    fixture.settle(ui);
    assert_eq!(
        fixture.manager.installed["markdown"].manifest.version,
        package.manifest.version
    );
    assert!(
        !old_request.begin(),
        "successful replacement retires pending old work"
    );
    assert_eq!(std::fs::read_to_string(&sentinel).unwrap(), "原有受管数据");
    assert_eq!(std::process::id(), process);
    assert_eq!(
        ui.update(|_, cx| fixture.app.read(cx).editor.clone()),
        editor
    );
    assert_eq!(
        ui.update(|_, cx| editor.read(cx).text().to_string()),
        before
    );
    assert_eq!(
        std::fs::read_to_string(fixture.directory.path().join("notes.md")).unwrap(),
        "磁盘正文\n"
    );
    fixture.focus_editor(ui);
    ui.simulate_keystrokes("ctrl-home");
    fixture.click("plugin-ui-format-heading", ui);
    assert_eq!(
        ui.update(|_, cx| editor.read(cx).text().to_string()),
        "# 未保存中文\n"
    );
    fixture.focus_editor(ui);
    ui.simulate_keystrokes("ctrl-z");
    fixture.settle(ui);
    assert_eq!(
        ui.update(|_, cx| editor.read(cx).text().to_string()),
        before,
        "the format action is one Undo"
    );
    ui.simulate_keystrokes("ctrl-z");
    fixture.settle(ui);
    assert_eq!(
        ui.update(|_, cx| editor.read(cx).text().to_string()),
        "磁盘正文\n",
        "Undo from before update remains alive"
    );
    ui.simulate_keystrokes("ctrl-y ctrl-y");
    fixture.settle(ui);
    assert_eq!(
        ui.update(|_, cx| editor.read(cx).text().to_string()),
        "# 未保存中文\n"
    );
    fixture.manager.disable("markdown").unwrap();
    fixture.settle(ui);
    fixture.manager.uninstall("markdown", false).unwrap();
    fixture.settle(ui);
    let snapshot = observations.snapshot(Some("markdown"));
    let owners: Vec<_> = snapshot
        .owners
        .iter()
        .filter(|owner| owner.owner.resource_owner)
        .collect();
    assert!(
        owners.len() >= 2,
        "both old and new runtime incarnations must be observed"
    );
    assert!(owners.iter().all(|owner| {
        let counts = owner.resources.as_ref().unwrap();
        counts.retired
            && counts.handles == 0
            && counts.requests == 0
            && counts.subscriptions == 0
            && counts.timers == 0
    }));
    assert!(ui.debug_bounds("editor-source-toolbar").is_none());
    assert_eq!(
        ui.update(|_, cx| editor.read(cx).text().to_string()),
        "# 未保存中文\n"
    );
}

/// Poll the existing actor/frame seam until the actual selected provider paints its latest source-bound root.
fn wait_markdown_outline(
    fixture: &mut NativeMarkdown,
    ui: &mut gpui_kit::VisualTestContext,
    root: &str,
) {
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        fixture.settle(ui);
        let ready = ui.update(|_, cx| {
            fixture
                .app
                .read(cx)
                .outline
                .tree
                .read(cx)
                .entry(0)
                .is_some_and(|entry| entry.item().label.as_ref() == root)
        });
        if ready && ui.debug_bounds("outline-node-/0/0").is_some() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Markdown outline failed to paint the requested source-bound tree"
        );
        std::thread::sleep(Duration::from_millis(15));
    }
}

/// Public structure discovery, hierarchy, definition clicks, unsaved refresh and retirement work in the native outline.
#[gpui::test]
#[ignore = "actual Markdown outline ZIP; set NANOBUG_MARKDOWN_SDK_PACKAGE and SHA256"]
fn markdown_sdk_adoption_outline_uses_native_tree_and_precise_unsaved_definitions(
    cx: &mut TestAppContext,
) {
    let package = candidate();
    let source = "# 中文根\n\n### **中文子标题**\n\n子内容\n\n另一个根\n========\n";
    let (mut fixture, ui) = NativeMarkdown::mount_package(cx, &[("大纲.md", source)], &package);
    crate::extensions::demand_plugin_fixture(
        &mut fixture.manager,
        "markdown",
        plugin_runtime::DemandReason::Language {
            provider: "markdown/structure".into(),
            role: plugin_runtime::LanguageRole::Structure,
        },
    )
    .unwrap();
    fixture.open("大纲.md", ui);
    let toggle = ui.debug_bounds("outline-toggle").unwrap();
    ui.simulate_click(toggle.center(), Default::default());
    wait_markdown_outline(&mut fixture, ui, "中文根");
    let provider = fixture.manager.structure_providers()["markdown/structure"]
        .as_ref()
        .unwrap()
        .clone();
    // Exercise the production worker's fuel/deadline near its independent 1 MiB cap.
    // A document above the preview budget must still expose its native heading tree.
    let mut large = "# 预算上限\n\n".to_string();
    while large.len() + 13 <= 1024 * 1024 {
        large.push_str("正文内容\n");
    }
    let described = provider
        .describe(protocol::language::SourceSnapshot {
            document: protocol::api::DocumentVersion {
                id: "large-markdown-outline".into(),
                path: "large-markdown-outline.md".into(),
                revision: 1,
            },
            text: large,
        })
        .expect("near-cap production structure worker");
    assert_eq!(described.proposal.nodes[0].name, "预算上限");
    let child = ui.debug_bounds("outline-node-/0/0").unwrap();
    ui.simulate_click(child.center(), Default::default());
    ui.run_until_parked();
    assert_eq!(
        ui.update(|_, cx| fixture.app.read(cx).editor.read(cx).cursor()),
        source.find("中文子标题").unwrap()
    );
    fixture.focus_editor(ui);
    ui.simulate_keystrokes("ctrl-home shift-end");
    ui.simulate_input("# 更新根");
    wait_markdown_outline(&mut fixture, ui, "更新根");
    assert_eq!(
        std::fs::read_to_string(fixture.directory.path().join("大纲.md")).unwrap(),
        source
    );
    let child_label = ui.update(|_, cx| {
        fixture
            .app
            .read(cx)
            .outline
            .tree
            .read(cx)
            .entry(1)
            .unwrap()
            .item()
            .label
            .to_string()
    });
    assert_eq!(child_label, "中文子标题");
    fixture.manager.disable("markdown").unwrap();
    fixture.settle(ui);
    assert!(!provider.is_active());
    assert!(ui.debug_bounds("outline-node-/0/0").is_none());
}
