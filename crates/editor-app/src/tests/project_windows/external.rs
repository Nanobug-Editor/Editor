//! External startup dispatch preserves mounted project documents and reuses real directory owners.
use super::*;

/// A stalled local sender must not retain the GUI's profile owner after its final shutdown.
#[test]
fn external_startup_shutdown_releases_owner_during_partial_request() {
    use crate::app::startup::{self, Admission};
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::{Duration, Instant};
    let profile = tempfile::tempdir().unwrap();
    let Admission::Primary(server) = startup::enter(profile.path(), None).unwrap() else {
        panic!("isolated GUI must own its profile")
    };
    // Discover the real local endpoint just as an external launch does, without inspecting its worker state.
    let endpoint: serde_json::Value =
        serde_json::from_slice(&std::fs::read(profile.path().join("gui/endpoint.json")).unwrap())
            .unwrap();
    let address = ("127.0.0.1", endpoint["port"].as_u64().unwrap() as u16);
    let mut probe = TcpStream::connect(address).unwrap();
    probe
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    probe.write_all(&[2, 0, 0, 0, b'{', b'}']).unwrap();
    let mut length = [0u8; 4];
    probe.read_exact(&mut length).unwrap();
    let mut rejection = vec![0; u32::from_le_bytes(length) as usize];
    probe.read_exact(&mut rejection).unwrap();
    assert!(
        serde_json::from_slice::<serde_json::Value>(&rejection)
            .unwrap()
            .get("Rejected")
            .is_some()
    );
    drop(probe);
    let mut slow = TcpStream::connect(address).unwrap();
    slow.write_all(&1024u32.to_le_bytes()).unwrap();
    slow.write_all(b"{").unwrap();
    // Finite trickle input exposes per-read timeout resets without leaving a failed regression hung forever.
    let sender = std::thread::spawn(move || {
        for _ in 0..6 {
            std::thread::sleep(Duration::from_millis(300));
            if slow.write_all(b" ").is_err() {
                return;
            }
        }
        std::thread::sleep(Duration::from_secs(2));
    });
    std::thread::sleep(Duration::from_millis(100));
    let start = Instant::now();
    drop(server);
    let elapsed = start.elapsed();
    sender.join().unwrap();
    assert!(
        elapsed < Duration::from_secs(1),
        "final shutdown waited for a partial external request: {elapsed:?}"
    );
    assert!(
        matches!(
            startup::enter(profile.path(), None).unwrap(),
            Admission::Primary(_)
        ),
        "a subsequent launch must become the real profile owner"
    );
}

/// Drive a real secondary transport while pumping only the production GPUI executor and native views.
fn deliver(
    visual: &mut VisualTestContext,
    profile: &std::path::Path,
    target: Option<std::path::PathBuf>,
) -> Result<bool, String> {
    let profile = profile.to_owned();
    let request = std::thread::spawn(move || {
        crate::app::startup::enter(&profile, target)
            .map(|admission| matches!(admission, crate::app::startup::Admission::Delivered))
            .map_err(|error| error.to_string())
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while !request.is_finished() && std::time::Instant::now() < deadline {
        visual
            .executor()
            .advance_clock(std::time::Duration::from_millis(40));
        visual.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(3));
    }
    assert!(
        request.is_finished(),
        "startup delivery must return within its bounded attempt"
    );
    request.join().unwrap()
}

/// Shared admission handles concurrent aliases, native file targets, invalid paths, close decisions and profile isolation.
#[gpui::test]
fn external_startup_preserves_documents_across_concurrent_and_failed_requests(
    cx: &mut TestAppContext,
) {
    use crate::app::startup::{self, Admission};
    let profile = tempfile::tempdir().unwrap();
    let projects = tempfile::tempdir().unwrap();
    let a = projects.path().join("项目 A & [甲]");
    let b = projects.path().join("项目 B & [乙]");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let file_a = a.join("编辑 A.txt");
    let file_b = b.join("编辑 B.txt");
    std::fs::write(&file_a, "A original").unwrap();
    std::fs::write(&file_b, "B original").unwrap();
    let Admission::Primary(server) = startup::enter(profile.path(), None).unwrap() else {
        panic!("missing primary owner")
    };
    with_home(cx, profile.path(), |visual, host| {
        pick_restricted_project(visual, &a);
        visual.update(|window, cx| {
            let editor = host.read(cx).editor.clone().unwrap();
            editor.update(cx, |app, cx| {
                app.open_file(file_a.clone(), window, cx);
                app.editor.update(cx, |editor, cx| {
                    editor.set_value("A unsaved 中文🙂", window, cx);
                    cx.emit(gpui_base::input::InputEvent::Change);
                });
            });
            startup::attach(server, profile.path().to_owned(), cx);
        });
        visual.run_until_parked();
        assert_eq!(deliver(visual, profile.path(), None), Ok(true));
        assert_eq!(deliver(visual, profile.path(), None), Ok(true));
        assert_eq!(
            visual.update(|_, cx| cx.windows().len()),
            1,
            "repeat no-argument launch foregrounds the existing editor"
        );
        let requests: Vec<_> = (0..8)
            .map(|_| {
                let profile = profile.path().to_owned();
                let target = b.join(".");
                std::thread::spawn(move || {
                    matches!(
                        startup::enter(&profile, Some(target)).unwrap(),
                        Admission::Delivered
                    )
                })
            })
            .collect();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while requests.iter().any(|request| !request.is_finished())
            && std::time::Instant::now() < deadline
        {
            visual
                .executor()
                .advance_clock(std::time::Duration::from_millis(40));
            visual.run_until_parked();
            std::thread::sleep(std::time::Duration::from_millis(3));
        }
        assert!(requests.into_iter().all(|request| request.join().unwrap()));
        assert_eq!(
            visual.update(|_, cx| cx.windows().len()),
            2,
            "concurrent requests reserve just one real B directory"
        );
        assert_eq!(
            deliver(visual, profile.path(), Some(file_b.clone())),
            Ok(true)
        );
        let b_handle = visual.update(|_, cx| {
            let identity =
                platform_windows::directory_identity::DirectoryIdentity::open(&b).unwrap();
            super::super::controller::owner(&identity, cx)
                .unwrap()
                .handle
        });
        let mut b_visual = VisualTestContext::from_window(b_handle, &visual.cx);
        b_visual.run_until_parked();
        click(&mut b_visual, "project-trust-restricted");
        assert!(b_visual.debug_bounds("editor-shell").is_some());
        let active_b = b_visual.update(|_, cx| {
            let identity =
                platform_windows::directory_identity::DirectoryIdentity::open(&b).unwrap();
            let entry = super::super::controller::owner(&identity, cx).unwrap();
            entry
                .host
                .upgrade()
                .unwrap()
                .read(cx)
                .editor
                .as_ref()
                .unwrap()
                .read(cx)
                .active_path
                .clone()
                .unwrap()
        });
        assert_eq!(
            active_b.canonicalize().unwrap(),
            file_b.canonicalize().unwrap()
        );
        assert!(
            deliver(
                visual,
                profile.path(),
                Some(projects.path().join("missing"))
            )
            .is_err()
        );
        visual.dispatch_action(crate::app::projects::CloseWindow);
        visual.run_until_parked();
        assert!(deliver(visual, profile.path(), Some(file_a.clone())).is_err());
        assert!(visual.debug_bounds("project-decision-cancel").is_some());
        click(visual, "project-decision-cancel");
        visual.update(|_, cx| crate::app::projects::quit_application(cx));
        visual.run_until_parked();
        assert!(deliver(visual, profile.path(), Some(b.clone())).is_err());
        click(visual, "project-decision-cancel");
        assert_eq!(visual.update(|_, cx| cx.windows().len()), 2);
        assert_eq!(
            visual.update(|_, cx| {
                host.read(cx)
                    .editor
                    .as_ref()
                    .unwrap()
                    .read(cx)
                    .editor
                    .read(cx)
                    .value()
                    .to_string()
            }),
            "A unsaved 中文🙂"
        );
        let isolated = tempfile::tempdir().unwrap();
        let Admission::Primary(independent) =
            startup::enter(isolated.path(), Some(b.clone())).unwrap()
        else {
            panic!("another profile must remain independently owned")
        };
        drop(independent);
        assert_eq!(visual.update(|_, cx| cx.windows().len()), 2);
    });
}

/// A second ordinary launch succeeds only after the primary dispatched it to a real project window.
#[gpui::test]
fn external_startup_delivery_waits_for_real_window_dispatch(cx: &mut TestAppContext) {
    use crate::app::startup::{self, Admission};
    let profile = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let server = match startup::enter(profile.path(), None).unwrap() {
        Admission::Primary(server) => server,
        Admission::Delivered => panic!("the isolated profile must have no prior application"),
    };
    with_home(cx, profile.path(), |visual, _| {
        visual.update(|_, cx| startup::attach(server, profile.path().to_owned(), cx));
        let selected_profile = profile.path().to_owned();
        let target = project.path().to_owned();
        let secondary = std::thread::spawn(move || {
            matches!(
                startup::enter(&selected_profile, Some(target)).unwrap(),
                Admission::Delivered
            )
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(12);
        while !secondary.is_finished() && std::time::Instant::now() < deadline {
            visual
                .executor()
                .advance_clock(std::time::Duration::from_millis(40));
            visual.run_until_parked();
            std::thread::sleep(std::time::Duration::from_millis(3));
        }
        assert!(
            secondary.is_finished(),
            "external delivery must have a bounded result"
        );
        assert!(
            secondary.join().unwrap(),
            "a second launch must not allocate another profile owner"
        );
        visual.run_until_parked();
        assert_eq!(
            visual.update(|_, cx| cx.windows().len()),
            2,
            "successful delivery must correspond to a dispatched native project window"
        );
    });
}

/// Repeating an external project request must activate A while preserving A's unsaved text and B.
#[gpui::test]
fn external_project_request_reuses_owner_and_preserves_peer(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    std::fs::write(a.path().join("a.txt"), "A original").unwrap();
    with_home(cx, profile.path(), |visual, host| {
        pick_restricted_project(visual, a.path());
        visual.update(|window, cx| {
            let editor = host.read(cx).editor.clone().unwrap();
            editor.update(cx, |app, cx| {
                app.open_file(a.path().join("a.txt"), window, cx);
                app.editor.update(cx, |editor, cx| {
                    editor.set_value("A unsaved 中文", window, cx);
                    cx.emit(gpui_base::input::InputEvent::Change);
                });
            });
        });
        visual.run_until_parked();
        visual.update(|_, cx| {
            crate::app::projects::open_request(
                profile.path().to_owned(),
                Some(b.path().to_owned()),
                cx,
            )
            .unwrap();
        });
        visual.run_until_parked();
        assert_eq!(visual.update(|_, cx| cx.windows().len()), 2);
        visual.update(|_, cx| {
            crate::app::projects::open_request(
                profile.path().to_owned(),
                Some(a.path().join("a.txt")),
                cx,
            )
            .unwrap();
        });
        visual.run_until_parked();
        assert_eq!(
            visual.update(|_, cx| cx.windows().len()),
            2,
            "an external request must not leave an extra native home behind"
        );
        assert_eq!(
            visual.update(|_, cx| {
                host.read(cx)
                    .editor
                    .as_ref()
                    .unwrap()
                    .read(cx)
                    .editor
                    .read(cx)
                    .value()
                    .to_string()
            }),
            "A unsaved 中文"
        );
    });
}
