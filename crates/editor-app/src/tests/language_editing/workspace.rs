//! A packaged real TypeScript engine drives native discovery, preview and grouped document Undo.
use super::*;

/// Poll actual package demands and native frames; selectors identify user-visible Base controls.
fn await_control(
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    visual: &mut VisualTestContext,
    selector: &'static str,
) {
    let deadline = std::time::Instant::now() + Duration::from_secs(90);
    loop {
        crate::extensions::lsp_tests::publish(app, manager, visual);
        pump(visual, 30);
        if visual.debug_bounds(selector).is_some() {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Missing {selector}; status={}",
            visual.update(|_, cx| app.read(cx).status.clone())
        );
    }
}

/// Mouse activation follows the same Base button path as a user confirmation or recovery action.
fn click(visual: &mut VisualTestContext, selector: &'static str) {
    let bounds = visual.debug_bounds(selector).unwrap();
    visual.simulate_mouse_move(bounds.center(), None, gpui_kit::Modifiers::default());
    visual.simulate_click(bounds.center(), gpui_kit::Modifiers::default());
    visual.run_until_parked();
}

#[gpui::test]
#[ignore = "set NANOBUG_TYPESCRIPT_PACKAGE to independently built TypeScript ZIP; uses its real TS engine"]
fn real_typescript_rename_opens_unknown_targets_previews_cancels_and_group_undo(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let model = "export const value = 1;\n";
    let consumer = "import { value } from './model';\nconsole.log(value);\n";
    std::fs::write(
        directory.path().join("tsconfig.json"),
        "{\"compilerOptions\":{\"strict\":true},\"include\":[\"*.ts\"]}",
    )
    .unwrap();
    std::fs::write(directory.path().join("model.ts"), model).unwrap();
    std::fs::write(directory.path().join("consumer.ts"), consumer).unwrap();
    std::fs::write(
        directory.path().join("math.ts"),
        "export function double(value: number) { return value * 2; }\n",
    )
    .unwrap();
    std::fs::write(directory.path().join("completion.ts"), "").unwrap();
    std::fs::write(
        directory.path().join("types.ts"),
        "export type Quantity = number;\n",
    )
    .unwrap();
    let package = Package::read(&PathBuf::from(
        std::env::var("NANOBUG_TYPESCRIPT_PACKAGE").expect("build the independent TS package"),
    ))
    .unwrap();
    let (app, visual, mut manager) = shell(cx, directory.path(), vec![package]);
    let model_path = directory.path().join("model.ts").canonicalize().unwrap();
    let consumer_path = directory.path().join("consumer.ts").canonicalize().unwrap();
    let dependency_path = directory.path().join("types.ts").canonicalize().unwrap();
    open(&app, &dependency_path, visual);
    open(&app, &model_path, visual);
    let deadline = std::time::Instant::now() + Duration::from_secs(90);
    loop {
        crate::extensions::lsp_tests::publish(&app, &mut manager, visual);
        pump(visual, 250);
        if visual.update(|_, cx| app.read(cx).language_server_for_path(&model_path).is_some()) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "TS package must prepare its declared provider"
        );
    }
    // A later readonly target rejects the entire discovery reply without applying the valid source.
    let original_permissions = std::fs::metadata(&consumer_path).unwrap().permissions();
    let mut readonly = original_permissions.clone();
    readonly.set_readonly(true);
    std::fs::set_permissions(&consumer_path, readonly).unwrap();
    immediate_caret(&app, model.find("value").unwrap() + 1, visual);
    visual.simulate_keystrokes("f2");
    await_control(&app, &mut manager, visual, "editor-rename-prompt");
    visual.simulate_input("blocked");
    visual.simulate_keystrokes("enter");
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        pump(visual, 20);
        if visual.update(|_, cx| app.read(cx).status.contains("readonly")) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "readonly target must reject the complete rename"
        );
    }
    assert_text(&app, model, visual);
    assert!(visual.debug_bounds("workspace-plan-confirm").is_none());
    std::fs::set_permissions(&consumer_path, original_permissions).unwrap();
    for stage in ["cancel", "dependency conflict", "apply"] {
        immediate_caret(&app, model.find("value").unwrap() + 1, visual);
        visual.simulate_keystrokes("f2");
        await_control(&app, &mut manager, visual, "editor-rename-prompt");
        visual.simulate_input("renamed");
        visual.simulate_keystrokes("enter");
        await_control(&app, &mut manager, visual, "workspace-plan-confirm");
        assert_text(&app, model, visual);
        assert_eq!(std::fs::read_to_string(&consumer_path).unwrap(), consumer);
        assert!(
            visual.update(|_, cx| app
                .read(cx)
                .tabs
                .iter()
                .any(|tab| tab.path() == consumer_path)),
            "unknown target opens for recomputation before preview"
        );
        if stage == "dependency conflict" {
            visual.update(|window, cx| {
                let editor = app
                    .read(cx)
                    .tabs
                    .iter()
                    .find(|tab| tab.path() == dependency_path)
                    .unwrap()
                    .text
                    .as_ref()
                    .unwrap()
                    .editor
                    .clone();
                editor.update(cx, |editor, cx| {
                    editor.set_selected_range(0..0, cx);
                    editor.replace("// changed while previewing\n", window, cx);
                });
            });
            pump(visual, 200);
            assert!(
                visual.debug_bounds("workspace-plan-confirm").is_none(),
                "a readonly input revision cancels the whole preview"
            );
            assert_text(&app, model, visual);
            continue;
        }
        click(
            visual,
            if stage == "cancel" {
                "workspace-plan-cancel"
            } else {
                "workspace-plan-confirm"
            },
        );
        pump(visual, 100);
        if stage == "cancel" {
            assert_text(&app, model, visual);
        }
    }
    await_text(
        &app,
        "export const renamed = 1;\n",
        visual,
        "complete rename applies after preview",
    );
    let modified = visual.update(|_, cx| {
        let app = app.read(cx);
        app.tabs
            .iter()
            .find(|tab| tab.path() == consumer_path)
            .unwrap()
            .text
            .as_ref()
            .unwrap()
            .editor
            .read(cx)
            .text()
            .to_string()
    });
    assert_eq!(
        modified,
        "import { renamed } from './model';\nconsole.log(renamed);\n"
    );
    assert_eq!(std::fs::read_to_string(&model_path).unwrap(), model);
    assert_eq!(std::fs::read_to_string(&consumer_path).unwrap(), consumer);
    await_control(&app, &mut manager, visual, "workspace-batch-undo");
    click(visual, "workspace-batch-undo");
    await_text(&app, model, visual, "group Undo restores the source");
    assert_eq!(
        visual.update(|_, cx| app
            .read(cx)
            .tabs
            .iter()
            .find(|tab| tab.path() == consumer_path)
            .unwrap()
            .text
            .as_ref()
            .unwrap()
            .editor
            .read(cx)
            .text()
            .to_string()),
        consumer
    );
    // The actual selected item is resolved lazily; its import and insertion form one local Undo.
    let completion_path = directory
        .path()
        .join("completion.ts")
        .canonicalize()
        .unwrap();
    open(&app, &completion_path, visual);
    let deadline = std::time::Instant::now() + Duration::from_secs(90);
    loop {
        crate::extensions::lsp_tests::publish(&app, &mut manager, visual);
        pump(visual, 30);
        if visual.update(|_, cx| {
            app.read(cx)
                .editor
                .read(cx)
                .lsp()
                .completion_provider
                .is_some()
        }) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "completion source must finish provider preparation before user input"
        );
    }
    visual.simulate_input("doub");
    await_control(&app, &mut manager, visual, "editor-completion-row-0");
    visual.simulate_keystrokes("enter");
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        pump(visual, 20);
        let inserted = text(&app, visual);
        if inserted.contains("import") && inserted.trim_end().ends_with("double") {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "selected completion must insert its main text and auto-import together: {inserted:?}; {}",
            visual.update(|_, cx| app.read(cx).status.clone())
        );
    }
    assert_eq!(std::fs::read_to_string(&completion_path).unwrap(), "");
    visual.simulate_keystrokes("ctrl-z");
    await_text(
        &app,
        "doub",
        visual,
        "one local Undo restores both completion and import",
    );
    manager.shutdown();
}
