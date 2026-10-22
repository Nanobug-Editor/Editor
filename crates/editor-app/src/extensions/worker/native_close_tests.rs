//! Real Win32 caption-close messages must not stall a failed independent plugin window or its parent.
use super::*;
use crate::*;
use plugin_runtime::plugin_protocol as protocol;
use raw_window_handle::RawWindowHandle;
use serde_json::json;
use std::{
    cell::RefCell,
    process::{Command, Stdio},
    rc::Rc,
    time::{Duration, Instant},
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    IsWindow, PostMessageW, SC_CLOSE, WM_CLOSE, WM_SYSCOMMAND,
};

const CHILD: &str = "extensions::worker::native_close_tests::failed_window_caption_child";
pub(super) mod caption_mouse;
mod loading;

/// Bound the real UI-thread check externally: a blocked Windows callback cannot run an async timeout.
#[test]
#[ignore = "requires Windows desktop/D3D11; sends WM_CLOSE only to the private child's own window"]
fn failed_window_caption_close_keeps_the_native_message_loop_responsive() {
    run_child(None, 15);
}

/// Reproduce restored details with the actual installed package and production asynchronous actor.
#[test]
#[ignore = "requires Windows desktop/D3D11 and NANOBUG_GIT_CANDIDATE; uses private non-Git project/profile"]
fn git_non_repository_details_tree_and_caption_remain_responsive() {
    run_child(
        Some(std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("independent candidate ZIP")),
        120,
    );
}

/// The parent bounds only its own child and preserves the exact native progress on failure.
fn run_child(package: Option<std::ffi::OsString>, seconds: u64) {
    let directory = tempfile::tempdir().unwrap();
    let log = std::fs::File::create(directory.path().join("native-close.log")).unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            CHILD,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("RUST_BACKTRACE", "1")
        .env_remove("NANOBUG_CLOSE_GIT_PACKAGE");
    if let Some(package) = package {
        command.env("NANOBUG_CLOSE_GIT_PACKAGE", package);
    }
    let mut child = command
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .stdin(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let log = std::fs::read_to_string(directory.path().join("native-close.log")).unwrap();
    assert!(
        status.is_some_and(|status| status.success()),
        "native caption close stalled or failed: {status:?}\\n{log}"
    );
    assert!(
        log.contains("native caption: close verified and all owned HWNDs retired"),
        "missing close receipt: {log}"
    );
    eprintln!("{log}");
}

/// Use the actual EditorApp, declared window, caption gate and WindowsPlatform without a real user profile.
#[test]
#[ignore = "private child of the bounded caption-close test"]
fn failed_window_caption_child() {
    let directory = tempfile::tempdir().unwrap();
    let workspace_path = directory.path().join("workspace");
    let profile = directory.path().join("profile");
    std::fs::create_dir_all(&workspace_path).unwrap();
    let actual = loading::prepare(&workspace_path, &profile);
    let mut session = SessionState::load_in_profile(&workspace_path, &profile);
    if actual {
        session.workspace_trusted = true;
        for panel in ["history", "staging", "commit-details"] {
            session.plugin_panel_visibility.insert(
                format!("git/{panel}"),
                panel == "commit-details"
                    || std::env::var_os("NANOBUG_CLOSE_ONLY_DETAILS").is_none(),
            );
        }
    }
    let key = if actual {
        "git/commit-details"
    } else {
        "caption-error.fixture/details"
    };
    let workspace = Workspace::open(&workspace_path).unwrap();
    let completed = Rc::new(RefCell::new(None));
    let verdict = completed.clone();
    gpui_kit::application().with_assets(AppAssets)
        .with_quit_mode(gpui_kit::QuitMode::Explicit).run(move |cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        extensions::init(cx);
        editor_ui::init(cx);
        crate::app::shortcuts::bootstrap::configure(cx, profile.join("shortcuts.json")).unwrap();
        let actor = actual.then(|| Arc::new(Worker::start_background(
            extensions::profile_runtime_root(&profile), extensions::environment(&workspace_path, cx), true
        )));
        let (main, app) = gpui_kit::open_window(Default::default(), cx, move |window, cx| {
            cx.new(|cx| EditorApp::new_with_session(workspace, None, Some(session), window, cx))
        }).unwrap();
        let main = gpui_kit::AnyWindowHandle::from(main);
        app.read(cx).extensions.clone().update(cx, |owner, cx| {
            if let Some(actor) = &actor {
                owner.worker = actor.clone();
                owner.poll(cx);
                return;
            }
            let manifest: protocol::Manifest = serde_json::from_value(json!({
                "id":"caption-error.fixture","name":"Caption failure","version":"1.0.0","protocol":7,
                "permissions":["ui.panels"],"storage_limit":1024,
                "api":{"base":"^1","required":{"ui.native":">=1.1,<2","ui.windows":"^1"}},
                "panels":[{"id":"details","title":"Timeout close reproduction","position":"window","default_visible":true}]
            })).unwrap();
            let mut state = owner.worker.state.lock().unwrap();
            state.entries = vec![Installed {
                grants: manifest.permissions.clone(), manifest, digest:"caption-error".into(), enabled:true,
                project_enabled:Default::default(), replaced_by:None, global_enabled:None,
                retired_ui_contract:false, error:Some("timeout".into()),
            }];
            drop(state);
            owner.poll(cx);
        });
        let owner = app.downgrade();
        drop(app);
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_millis(50)).await;
            let (details, main_hwnd) = main.update(cx, |_, window, cx| {
                window.draw(cx).clear(cx);
                let app = owner.upgrade().unwrap();
                let handle = gpui_kit::AnyWindowHandle::from(app.read(cx).plugin_windows[key]);
                (handle, hwnd(window))
            }).unwrap();
            let details_hwnd = details.update(cx, |_, window, cx| {
                window.draw(cx).clear(cx);
                hwnd(window)
            }).unwrap();
            let loaded = if let Some(actor) = &actor {
                loading::observe(actor, main, details, cx).await
            } else { true };
            if actual && !loaded {
                loading::retry(details, cx).await;
            }
            let mouse_action = std::env::var("NANOBUG_CAPTION_MOUSE").ok();
            let mouse_ok = if let Some(action) = &mouse_action {
                let result = caption_mouse::exercise(action, details, cx).await;
                eprintln!("actual caption scenario {action}: {result:#?}");
                result.is_ok()
            } else { true };
            if mouse_action.as_deref() != Some("close") || !mouse_ok {
                let (message, parameter) = if actual { (WM_SYSCOMMAND, SC_CLOSE as usize) } else { (WM_CLOSE, 0) };
                eprintln!("native caption: posting owned details close command {message}");
                // The legacy control is unchanged; mouse failures use this only for cleanup and remain failures.
                assert_ne!(unsafe { PostMessageW(details_hwnd as _, message, parameter, 0) }, 0);
            }
            let mut closed = false;
            for _ in 0..100 {
                cx.background_executor().timer(Duration::from_millis(10)).await;
                closed = main.update(cx, |_, window, cx| {
                    window.draw(cx).clear(cx);
                    owner.upgrade().unwrap().read(cx).plugin_windows.is_empty()
                }).unwrap() && !alive(details_hwnd);
                if closed { break; }
            }
            eprintln!("native caption: message loop responsive, details retired={closed}");
            let cleanup = main.update(cx, |_, _, cx| {
                owner.update(cx, |app, cx| app.prepare_project_shutdown(cx)).unwrap().unwrap()
            }).unwrap();
            let cleanup = cleanup.await;
            for _ in 0..2 {
                main.update(cx, |_, window, cx| { window.blur(cx); window.draw(cx).clear(cx); }).unwrap();
                cx.background_executor().timer(Duration::from_millis(20)).await;
            }
            main.update(cx, |_, window, _| window.remove_window()).unwrap();
            let mut retired = false;
            for _ in 0..100 {
                cx.background_executor().timer(Duration::from_millis(10)).await;
                retired = cx.update(|cx| cx.windows().is_empty()) && !alive(main_hwnd) && !alive(details_hwnd);
                if retired { break; }
            }
            *verdict.borrow_mut() = Some(loaded && mouse_ok && closed && retired && cleanup.is_ok());
            if closed && retired { eprintln!("native caption: close verified and all owned HWNDs retired"); }
            cx.update(|cx| cx.quit());
        }).detach();
    });
    assert_eq!(
        *completed.borrow(),
        Some(true),
        "tree must finish loading and caption close must preserve a responsive parent"
    );
}

/// Borrow only this child window's native identity; no discovery or input targets other applications.
fn hwnd(window: &Window) -> isize {
    match raw_window_handle::HasWindowHandle::window_handle(window)
        .unwrap()
        .as_raw()
    {
        RawWindowHandle::Win32(handle) => handle.hwnd.get(),
        _ => unreachable!("Windows-only reproduction"),
    }
}

/// Query a previously borrowed HWND after ordinary close; never force-destroy native owners.
fn alive(hwnd: isize) -> bool {
    // SAFETY: IsWindow reads the opaque native identity without dereferencing it.
    unsafe { IsWindow(hwnd as _) != 0 }
}
