//! Real Windows controls verify update navigation, both palettes/locales and cancelled restart consent.

use super::*;
use crate::app::updates::{self, Phase};
use anyhow::{Context as _, ensure};
use gpui_base::test_support::{ElementSnapshot, snapshots};
use gpui_kit::{
    AnyWindowHandle, App, AsyncApp, ElementId, EntityInputHandler as _, Focusable as _,
    MouseButton, MouseDownEvent, MouseUpEvent, PlatformInput, QuitMode, Window,
};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};

#[path = "../window_toolbar/native/surface.rs"]
mod surface;

/// A bounded child owns every HWND/profile and saves D3D screenshots outside the repository.
#[test]
#[ignore = "opens isolated Windows/D3D windows; requires NANOBUG_HOST_UPDATE_OUTPUT"]
fn host_update_native_navigation_and_cancelled_restart() {
    let output = PathBuf::from(
        std::env::var_os("NANOBUG_HOST_UPDATE_OUTPUT").expect("provide native output directory"),
    );
    std::fs::create_dir_all(&output).unwrap();
    if std::env::var_os("NANOBUG_HOST_UPDATE_CHILD").is_none() {
        let log = std::fs::File::create(output.join("native-child.log")).unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "app::projects::tests::native_host_updates::host_update_native_navigation_and_cancelled_restart", "--ignored", "--nocapture", "--test-threads=1"])
            .env("NANOBUG_HOST_UPDATE_CHILD", "1").env("RUST_MIN_STACK", "8388608")
            .stdout(log.try_clone().unwrap()).stderr(log).spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(
                    status.success(),
                    "native updater validation failed; see {}",
                    output.join("native-child.log").display()
                );
                return;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("native updater validation exceeded 60 seconds");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    run_native(output);
}

/// Initialize the real renderer and run observations through ordinary local application windows.
fn run_native(output: PathBuf) {
    let profile = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let root = project.path().canonicalize().unwrap();
    std::fs::write(root.join("README.md"), "Native update 中文\n").unwrap();
    let mut session = crate::SessionState::load_in_profile(&root, profile.path());
    session.workspace_trusted = false;
    session.open_tabs = vec![root.join("README.md").display().to_string()];
    session.active_file = session.open_tabs.first().cloned();
    session.try_save().unwrap();
    let result = Rc::new(RefCell::new(None::<Result<(), String>>));
    let observed = result.clone();
    gpui_kit::application()
        .with_assets(crate::AppAssets)
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            typography::init(cx);
            // Install the same retained theme state as production before any native control renders.
            apply_theme(builtin_theme(false), cx);
            editor_ui::init(cx);
            cx.set_reduce_motion(true);
            crate::app::shortcuts::bootstrap::configure(cx, profile.path().join("shortcuts.json"))
                .unwrap();
            cx.spawn(async move |cx| {
                let outcome = exercise(profile.path(), &root, &output, cx).await;
                *observed.borrow_mut() = Some(outcome.map_err(|error| format!("{error:#}")));
                // Destroy owned HWNDs while the renderer and accessibility thread state are alive.
                cx.update(|cx| {
                    for window in cx.windows() {
                        let _ = window.update(cx, |_, window, _| window.remove_window());
                    }
                });
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                cx.update(|cx| cx.quit());
                drop((profile, project));
            })
            .detach();
        });
    result
        .borrow_mut()
        .take()
        .expect("native updater validation did not finish")
        .unwrap();
}

/// Empty and active roots share one ready candidate; cancelling the latter's dirty prompt preserves both.
async fn exercise(
    profile: &Path,
    root: &Path,
    output: &Path,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let (empty, empty_host) = cx.update(|cx| open(profile, None, cx))?;
    let service = cx.update(|cx| updates::ensure(profile, cx));
    let deadline = Instant::now() + Duration::from_secs(5);
    while cx.update(|cx| service.read(cx).phase == Phase::Loading) {
        ensure!(
            Instant::now() < deadline,
            "profile update initialization stalled"
        );
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    cx.update(|cx| {
        updates::tests::ready_fixture(profile, cx);
    });
    for (dark, locale, percent, label) in [
        (false, "zh-CN", 100, "light-zh-100"),
        (true, "en", 150, "dark-en-150"),
    ] {
        rust_i18n::set_locale(locale);
        empty.update(cx, |_, _, cx| apply_theme(builtin_theme(dark), cx))?;
        let dpi = empty.update(cx, |_, window, _| surface::DpiRequest::new(window, percent))??;
        dpi.apply();
        frame(empty, cx).await?;
        empty.update(cx, |_, window, cx| {
            click(window, "application-menu-trigger", cx)
        })??;
        frame(empty, cx).await?;
        empty.update(cx, |_, window, cx| {
            surface::capture(window, &output.join(format!("{label}-menu.png")))?;
            click(window, "native-menu-item-host-update", cx)
        })??;
        frame(empty, cx).await?;
        let settings: AnyWindowHandle = cx
            .update(|cx| empty_host.read(cx).dialog_window)
            .context("empty menu did not open settings")?
            .into();
        frame(settings, cx).await?;
        settings.update(cx, |_, window, _| {
            let primary = control(window, "host-update-primary")?;
            ensure!(
                primary.label() == Some(rust_i18n::t!("updates.restart").as_ref()),
                "wrong update action label"
            );
            for id in [
                "host-update-check",
                "host-update-primary",
                "host-update-automatic",
                "host-update-preview",
            ] {
                let bounds = control(window, id)?.bounds();
                ensure!(
                    bounds.left() >= px(0.)
                        && bounds.right() <= window.viewport_size().width
                        && bounds.bottom() <= window.viewport_size().height,
                    "update control clipped: {id}"
                );
            }
            surface::capture(window, &output.join(format!("{label}-settings.png")))
        })??;
        settings.update(cx, |_, window, cx| click(window, "app-dialog-close", cx))??;
        frame(empty, cx).await?;
    }
    rust_i18n::set_locale("zh-CN");
    let (active, active_host) = cx.update(|cx| open(profile, Some(root.to_owned()), cx))?;
    frame(active, cx).await?;
    active.update(cx, |_, window, cx| {
        let app = active_host
            .read(cx)
            .editor
            .clone()
            .context("restricted editor did not open")?;
        app.update(cx, |app, cx| {
            app.editor.update(cx, |editor, cx| {
                editor.focus_handle(cx).focus(window, cx);
                editor.replace_text_in_range(None, "未保存更新验收", window, cx);
            });
        });
        click(window, "application-menu-trigger", cx)
    })??;
    frame(active, cx).await?;
    active.update(cx, |_, window, cx| {
        click(window, "native-menu-item-host-update", cx)
    })??;
    frame(active, cx).await?;
    let settings: AnyWindowHandle = cx
        .update(|cx| {
            active_host
                .read(cx)
                .editor
                .as_ref()
                .unwrap()
                .read(cx)
                .dialog_window
        })
        .context("active menu did not open settings")?
        .into();
    frame(settings, cx).await?;
    settings.update(cx, |_, window, cx| click(window, "host-update-primary", cx))??;
    frame(active, cx).await?;
    active.update(cx, |_, window, cx| {
        click(window, "project-decision-cancel", cx)
    })??;
    frame(active, cx).await?;
    cx.update(|cx| {
        ensure!(
            service.read(cx).phase == Phase::Ready,
            "cancelled exit left update armed"
        );
        ensure!(
            active_host.read(cx).editor.is_some(),
            "cancelled update destroyed the editor"
        );
        ensure!(
            empty_host.read(cx).editor.is_none(),
            "empty window gained a workspace"
        );
        Ok::<_, anyhow::Error>(())
    })?;
    ensure!(
        std::fs::read_to_string(root.join("README.md"))? == "Native update 中文\n",
        "cancelled update wrote or discarded the document"
    );
    std::fs::write(
        output.join("native-result.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"palettes":["light","dark"],"locales":["zh-CN","en"],"scale_percent":[100,150],"empty_and_project_menu":true,"cancelled_restart_retains_documents":true}),
        )?,
    )?;
    Ok(())
}

/// Open a real root with the supplied isolated profile, optionally restoring its restricted session.
fn open(
    profile: &Path,
    root: Option<PathBuf>,
    cx: &mut App,
) -> anyhow::Result<(AnyWindowHandle, Entity<ProjectWindow>)> {
    let mut options = editor_ui::window::TitleBar::window_options();
    options.window_bounds = Some(gpui_kit::WindowBounds::Windowed(
        gpui_kit::Bounds::centered(None, size(px(1100.), px(760.)), cx),
    ));
    gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| {
            let mut host = ProjectWindow::new(profile.to_owned(), window, cx);
            if let Some(root) = root {
                host.open_path(root, window, cx);
            }
            host
        })
    })
}

/// Let Windows messages and deferred UI callbacks settle before collecting a fresh native frame.
async fn frame(handle: AnyWindowHandle, cx: &mut AsyncApp) -> anyhow::Result<()> {
    cx.background_executor()
        .timer(Duration::from_millis(130))
        .await;
    handle.update(cx, |_, window, cx| window.draw(cx).clear(cx))?;
    Ok(())
}

/// Select one visible painted control by its production element identity.
fn control(window: &Window, id: &str) -> anyhow::Result<ElementSnapshot> {
    snapshots(window)
        .into_iter()
        .find(|node| {
            node.visible()
                && node
                    .path()
                    .iter()
                    .any(|part| matches!(part, ElementId::Name(name) if name.as_ref() == id))
        })
        .with_context(|| format!("native update control {id} missing"))
}

/// Activate only this owned window and send ordinary pointer press/release at the painted bounds.
fn click(window: &mut Window, id: &str, cx: &mut App) -> anyhow::Result<()> {
    let position = control(window, id)?.bounds().center();
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
