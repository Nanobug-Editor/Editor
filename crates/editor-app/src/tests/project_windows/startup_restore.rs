//! A private Windows/D3D child verifies restored documents and settings through painted native controls.

use super::*;
use crate::app::startup::preferences::{self, Preferences};
use anyhow::{Context as _, ensure};
use gpui_base::test_support::{ElementSnapshot, snapshots};
use gpui_kit::{
    AnyWindowHandle, App, AsyncApp, ElementId, MouseButton, MouseDownEvent, MouseUpEvent,
    PlatformInput, QuitMode, Window,
};
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};

/// Keep native platform TLS out of other tests and bound the child without using a real user profile.
#[test]
#[ignore = "requires a real Windows desktop/D3D11; opens an isolated restored project and settings"]
fn startup_restore_native_settings_and_document() {
    const CHILD_ENV: &str = "NANOBUG_STARTUP_RESTORE_CHILD";
    if std::env::var_os(CHILD_ENV).is_none() {
        let output = tempfile::tempfile().unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "app::projects::tests::native_startup_restore::startup_restore_native_settings_and_document",
                "--ignored", "--nocapture", "--test-threads=1",
            ])
            .env(CHILD_ENV, "1")
            .env("RUST_MIN_STACK", "8388608")
            .stdout(output.try_clone().unwrap())
            .stderr(output.try_clone().unwrap())
            .spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("native startup restoration exceeded 30 seconds");
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        use std::io::{Read as _, Seek as _};
        let mut output = output;
        output.rewind().unwrap();
        let mut log = String::new();
        output.read_to_string(&mut log).unwrap();
        assert!(
            status.success(),
            "native startup restoration: {status}\n{log}"
        );
        assert!(log.contains("native startup restoration verified"), "{log}");
        return;
    }
    run_native();
}

/// Restore a real restricted session through the same target selection and open route as main.
fn run_native() {
    let profile = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let root = project.path().canonicalize().unwrap();
    std::fs::write(root.join("README.md"), "Native restored 中文🙂\n").unwrap();
    let mut session = crate::SessionState::load_in_profile(&root, profile.path());
    session.workspace_trusted = false;
    session.open_tabs = vec![root.join("README.md").display().to_string()];
    session.active_file = session.open_tabs.first().cloned();
    session.try_save().unwrap();
    preferences::remember_project(profile.path(), &root).unwrap();
    let profile_path = profile.path().to_owned();
    let result = Rc::new(RefCell::new(None));
    let observed = result.clone();
    rust_i18n::set_locale("zh-CN");
    gpui_kit::application()
        .with_assets(crate::AppAssets)
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            typography::init(cx);
            apply_theme(builtin_theme(false), cx);
            editor_ui::init(cx);
            crate::app::shortcuts::bootstrap::configure(cx, profile_path.join("shortcuts.json"))
                .unwrap();
            let target = preferences::target(&profile_path, None).unwrap();
            let mut options = editor_ui::window::TitleBar::window_options();
            options.window_bounds = Some(gpui_kit::WindowBounds::Windowed(
                gpui_kit::Bounds::centered(None, size(px(1000.), px(750.)), cx),
            ));
            let (handle, host) = gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| {
                    let mut host = ProjectWindow::new(profile_path.clone(), window, cx);
                    host.open_path(target, window, cx);
                    host
                })
            })
            .unwrap();
            let handle: AnyWindowHandle = handle.into();
            cx.spawn(async move |cx| {
                let outcome = exercise(handle, &host, &profile_path, &root, cx).await;
                let passed = outcome.is_ok();
                *observed.borrow_mut() = Some(outcome);
                if passed {
                    handle
                        .update(cx, |_, window, cx| {
                            host.update(cx, |host, cx| {
                                host.request_close(
                                    crate::app::projects::CloseIntent::Window,
                                    window,
                                    cx,
                                );
                            })
                        })
                        .unwrap();
                } else {
                    cx.update(|cx| cx.quit());
                }
            })
            .detach();
        });
    result
        .borrow_mut()
        .take()
        .expect("native scenario was not observed")
        .unwrap();
    assert!(
        Preferences::load(profile.path())
            .unwrap()
            .restore_last_project
    );
    eprintln!("native startup restoration verified");
}

/// Check the actual restored text, checkbox semantics, geometry, pointer activation and persisted switch.
async fn exercise(
    main: AnyWindowHandle,
    host: &Entity<ProjectWindow>,
    profile: &std::path::Path,
    root: &std::path::Path,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    cx.background_executor()
        .timer(Duration::from_millis(150))
        .await;
    main.update(cx, |_, window, cx| {
        let editor = host
            .read(cx)
            .editor
            .as_ref()
            .context("project was not restored")?
            .read(cx);
        ensure!(editor.workspace.root() == root && !editor.session_state.workspace_trusted);
        ensure!(editor.editor.read(cx).value().to_string() == "Native restored 中文🙂\n");
        window.draw(cx).clear(cx);
        click_native(window, "open-settings", cx)
    })??;
    cx.background_executor()
        .timer(Duration::from_millis(100))
        .await;
    // Observe and draw through an untyped handle so the Root entity is not leased during its own draw.
    let settings: AnyWindowHandle = cx
        .update(|cx| {
            host.read(cx)
                .editor
                .as_ref()
                .unwrap()
                .read(cx)
                .dialog_window
        })
        .context("native settings did not open")?
        .into();
    for dark in [false, true] {
        settings.update(cx, |_, window, cx| {
            apply_theme(builtin_theme(dark), cx);
            window.refresh();
        })?;
        cx.background_executor()
            .timer(Duration::from_millis(100))
            .await;
        settings.update(cx, |_, window, cx| {
            window.draw(cx).clear(cx);
            let checkbox = native_control(window, "settings-restore-last-project-checkbox")?;
            ensure!(checkbox.checked() == Some(!dark) && checkbox.disabled() != Some(true));
            let bounds = checkbox.bounds();
            ensure!(bounds.size.width > px(100.) && bounds.size.height > px(10.));
            ensure!(
                bounds.right() <= window.viewport_size().width
                    && bounds.bottom() <= window.viewport_size().height
            );
            if let Some(directory) = std::env::var_os("NANOBUG_STARTUP_CAPTURE_DIR") {
                let directory = PathBuf::from(directory);
                std::fs::create_dir_all(&directory)?;
                window.render_to_image()?.save(directory.join(if dark {
                    "settings-dark.png"
                } else {
                    "settings-light.png"
                }))?;
            }
            click_native(window, "settings-restore-last-project-checkbox", cx)
        })??;
        ensure!(Preferences::load(profile)?.restore_last_project == dark);
        ensure!(preferences::target(profile, None).is_some() == dark);
    }
    settings.update(cx, |_, window, cx| {
        window.draw(cx).clear(cx);
        click_native(window, "app-dialog-close", cx)
    })??;
    Ok(())
}

/// Find one visible Base control by its stable production identity after a real native draw.
fn native_control(window: &Window, id: &str) -> anyhow::Result<ElementSnapshot> {
    snapshots(window)
        .into_iter()
        .find(|node| {
            node.visible()
                && node
                    .path()
                    .iter()
                    .any(|part| matches!(part, ElementId::Name(name) if name.as_ref() == id))
        })
        .with_context(|| format!("native control {id} was not painted"))
}

/// Send ordinary pointer events at the painted control; Base owns focus and toggle behavior.
fn click_native(window: &mut Window, id: &str, cx: &mut App) -> anyhow::Result<()> {
    let position = native_control(window, id)?.bounds().center();
    window.activate_window();
    window.dispatch_event(
        PlatformInput::MouseDown(MouseDownEvent {
            position,
            button: MouseButton::Left,
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        }),
        cx,
    );
    window.dispatch_event(
        PlatformInput::MouseUp(MouseUpEvent {
            position,
            button: MouseButton::Left,
            modifiers: Default::default(),
            click_count: 1,
        }),
        cx,
    );
    Ok(())
}
