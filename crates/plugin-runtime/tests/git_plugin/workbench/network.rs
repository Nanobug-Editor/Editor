//! Actual package remote operations stay inside disposable repositories and loopback peers.
//! Remote helpers are test-only fixtures; the guest still executes Git through the public Manager.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[path = "../../support/cloud_http.rs"]
mod endpoint;
#[path = "network/success.rs"]
mod success;

/// A cached upstream already contains a newer commit: accidentally integrating after a failed
/// fetch would therefore change both HEAD and disk, rather than passing as a no-op pull.
fn tracking_repository(remote_url: &str) -> tempfile::TempDir {
    require_isolated_git_environment();
    let repository = repository();
    let root = repository.path();
    let hooks = root.join(".git/fixture-hooks");
    std::fs::create_dir(&hooks).unwrap();
    for (key, value) in [
        ("credential.helper", ""),
        ("credential.interactive", "false"),
        ("credential.useHttpPath", "true"),
        ("core.askPass", ""),
        ("http.proxy", ""),
        ("http.followRedirects", "false"),
        ("http.cookieFile", ""),
        ("http.saveCookies", "false"),
        ("http.extraHeader", ""),
        ("protocol.allow", "never"),
        ("protocol.http.allow", "always"),
    ] {
        git(root, &["config", "--local", key, value]);
    }
    git(
        root,
        &[
            "config",
            "--local",
            "core.hooksPath",
            hooks.to_str().unwrap(),
        ],
    );
    // Pin the entire generated URL against broader user insteadOf/header rules. The final
    // empty Authorization header suppresses libcurl authentication headers for this exact peer.
    git(
        root,
        &[
            "config",
            "--local",
            &format!("url.{remote_url}.insteadOf"),
            remote_url,
        ],
    );
    let header = format!("http.{remote_url}.extraHeader");
    git(root, &["config", "--local", &header, ""]);
    git(
        root,
        &["config", "--local", "--add", &header, "Authorization:"],
    );

    std::fs::write(root.join("tracked.txt"), "local baseline\n").unwrap();
    git(root, &["add", "--", "tracked.txt"]);
    git(root, &["commit", "-m", "local baseline"]);
    let base = git(root, &["rev-parse", "HEAD"]).trim().to_owned();
    std::fs::write(
        root.join("incoming.txt"),
        "cached upstream must not integrate\n",
    )
    .unwrap();
    git(root, &["add", "--", "incoming.txt"]);
    git(root, &["commit", "-m", "cached remote advance"]);
    let incoming = git(root, &["rev-parse", "HEAD"]).trim().to_owned();
    git(root, &["reset", "--hard", &base]);
    git(root, &["remote", "add", "upstream-http", remote_url]);
    git(
        root,
        &["config", "--local", "remote.upstream-http.proxy", ""],
    );
    git(
        root,
        &["config", "--local", "branch.main.remote", "upstream-http"],
    );
    git(
        root,
        &["config", "--local", "branch.main.merge", "refs/heads/main"],
    );
    git(
        root,
        &["update-ref", "refs/remotes/upstream-http/main", &incoming],
    );
    repository
}

fn text(manager: &mut Manager, id: &str) -> String {
    branch_menu::repository_text(manager, id)
}

/// The socket may intentionally outlive a UI callback, so this action must not call idle first.
fn press(manager: &mut Manager, node: &str) {
    if matches!(node, "fetch" | "pull" | "push" | "cancel-process") {
        branch_menu::context_press(manager, node, None);
        return;
    }
    let scene = &manager.live["git"].views["history"];
    assert!(
        !scene
            .active_node(node)
            .expect("visible network action")
            .disabled
    );
    let revision = scene.revision;
    manager
        .event(
            "git",
            Some("history".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: node.into(),
                action: ui::Action::Click,
            }),
        )
        .unwrap();
}

/// Process polling keeps the same public native-request boundary as the shared fixture. Network
/// operations may enumerate buffers, but must never save or replace any native editor text.
fn wait_until(manager: &mut Manager, description: &str, ready: impl Fn(&Manager) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        manager.poll();
        for (_, request) in manager.take_editor_requests() {
            match request.operation() {
                api::EditorOperation::ListDocuments => {
                    request.finish(Ok(api::EditorValue::Documents {
                        documents: vec![],
                        active: None,
                    }));
                }
                api::EditorOperation::SetPanelVisibility { panel, visible } => {
                    request.finish(Ok(api::EditorValue::PanelVisibility {
                        panel: panel.clone(),
                        visible: *visible,
                    }));
                }
                operation => panic!("network action cannot edit or save a buffer: {operation:?}"),
            }
        }
        if ready(manager) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {description}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Compare actual refs/index/worktree bytes, without consulting guest-private state.
fn unchanged(root: &Path, head: &str, upstream: &str, index: &str) {
    assert_eq!(git(root, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(root, &["rev-parse", "@{upstream}"]), upstream);
    assert_eq!(git(root, &["ls-files", "--stage"]), index);
    assert!(git(root, &["status", "--porcelain"]).is_empty());
    assert_eq!(
        std::fs::read_to_string(root.join("tracked.txt")).unwrap(),
        "local baseline\n"
    );
    assert!(!root.join("incoming.txt").exists());
    assert!(!root.join(".git/MERGE_HEAD").exists());
    assert!(!root.join(".git/rebase-merge").exists());
    assert!(!root.join(".git/rebase-apply").exists());
}

#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; loopback Git HTTP failures"]
fn git_package_http_authentication_failure_never_integrates_or_reports_a_successful_fetch() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::network::git_package_http_authentication_failure_never_integrates_or_reports_a_successful_fetch",
    ) {
        return;
    }
    let server = endpoint::Server::new(|stream, _| {
        endpoint::response(stream, 401, b"fixture authentication denied\n");
    });
    let remote = server.url("/repo.git").replace("localhost", "127.0.0.1");
    let repository = tracking_repository(&remote);
    let root = repository.path();
    let head = git(root, &["rev-parse", "HEAD"]);
    let upstream = git(root, &["rev-parse", "@{upstream}"]);
    let index = git(root, &["ls-files", "--stage"]);
    assert_ne!(head, upstream);
    let (_profile, mut manager) = activate(&candidate(), root);
    let sampled = text(&mut manager, "repo-fetch-time");
    assert!(sampled.contains("Not fetched yet"));

    for action in ["fetch", "pull"] {
        let before = server.count();
        click(&mut manager, "history", action);
        assert!(
            server.count() > before,
            "{action} never reached the loopback peer"
        );
        unchanged(root, &head, &upstream, &index);
        assert_eq!(text(&mut manager, "repo-fetch-time"), sampled);
        let diagnostic = text(&mut manager, "status").to_ascii_lowercase();
        assert!(
            diagnostic.contains("401")
                || diagnostic.contains("authentication")
                || diagnostic.contains("could not read username")
                || diagnostic.contains("unable to get password"),
            "the real authentication failure must remain visible: {diagnostic}"
        );
        assert!(manager.live["git"].views["history"].dialog.is_none());
        assert_eq!(manager.live["git"].process_count(), 0);
        let failed_requests = server.count();
        manager.invoke_command("git", "refresh", json!({})).unwrap();
        idle(&mut manager);
        assert_eq!(
            server.count(),
            failed_requests,
            "local refresh cannot retry authentication"
        );
        assert_eq!(text(&mut manager, "repo-fetch-time"), sampled);
    }
    assert!(server.captured.lock().unwrap().iter().all(|capture| {
        capture
            .headers
            .starts_with("GET /repo.git/info/refs?service=git-upload-pack ")
            && !capture
                .headers
                .to_ascii_lowercase()
                .contains("\nauthorization:")
    }));
    manager.disable("git").unwrap();
    assert!(!manager.live.contains_key("git"));
    assert_eq!(manager.resource_count(), 0);
}

#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; loopback Git HTTP cancellation"]
fn git_package_cancelling_stalled_fetch_and_pull_closes_the_socket_and_releases_owned_processes() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::network::git_package_cancelling_stalled_fetch_and_pull_closes_the_socket_and_releases_owned_processes",
    ) {
        return;
    }
    let started = Arc::new(AtomicUsize::new(0));
    let eof = Arc::new(AtomicUsize::new(0));
    let (accepted, closed) = (started.clone(), eof.clone());
    let server = endpoint::Server::start(
        "http",
        |stream| {
            stream
                .set_read_timeout(Some(Duration::from_secs(30)))
                .unwrap();
            Box::new(stream)
        },
        move |stream, _| {
            // A valid service preamble with an incomplete bounded body keeps Git's HTTP child
            // waiting for data. Windows may close an aborted HTTP transfer with TCP reset rather
            // than FIN; both are peer termination. A read timeout never counts as cancellation.
            if stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/x-git-upload-pack-advertisement\r\nContent-Length: 1024\r\nConnection: close\r\n\r\n001e# service=git-upload-pack\n0000").is_err()
                || stream.flush().is_err()
            {
                return;
            }
            accepted.fetch_add(1, Ordering::Release);
            let mut byte = [0];
            match stream.read(&mut byte) {
                Ok(0) => {
                    closed.fetch_add(1, Ordering::Release);
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::ConnectionReset
                            | std::io::ErrorKind::ConnectionAborted
                            | std::io::ErrorKind::BrokenPipe
                    ) =>
                {
                    eprintln!("cancelled Git peer closed with {:?}", error.kind());
                    closed.fetch_add(1, Ordering::Release);
                }
                other => eprintln!("stalled Git peer did not close: {other:?}"),
            }
        },
    );
    let remote = server.url("/repo.git").replace("localhost", "127.0.0.1");
    let repository = tracking_repository(&remote);
    let root = repository.path();
    let head = git(root, &["rev-parse", "HEAD"]);
    let upstream = git(root, &["rev-parse", "@{upstream}"]);
    let index = git(root, &["ls-files", "--stage"]);
    let (_profile, mut manager) = activate(&candidate(), root);
    let sampled = text(&mut manager, "repo-fetch-time");

    for action in ["fetch", "pull"] {
        let previous = started.load(Ordering::Acquire);
        press(&mut manager, action);
        wait_until(&mut manager, "a stalled Git HTTP connection", |manager| {
            started.load(Ordering::Acquire) > previous && manager.live["git"].process_count() > 0
        });
        unchanged(root, &head, &upstream, &index);
        let waiting = started.load(Ordering::Acquire);
        press(&mut manager, "cancel-process");
        wait_until(&mut manager, "cancelled Git HTTP peer closure", |manager| {
            eof.load(Ordering::Acquire) >= waiting && manager.live["git"].process_count() == 0
        });
        idle(&mut manager);
        unchanged(root, &head, &upstream, &index);
        assert_eq!(text(&mut manager, "repo-fetch-time"), sampled);
        let requests = server.count();
        manager.invoke_command("git", "refresh", json!({})).unwrap();
        idle(&mut manager);
        assert_eq!(
            server.count(),
            requests,
            "local recovery cannot restart a cancelled fetch"
        );
        assert_eq!(manager.live["git"].process_count(), 0);
        assert!(
            !manager.live["git"].views["history"]
                .active_node("fetch")
                .unwrap()
                .disabled
        );
    }
    assert_eq!(eof.load(Ordering::Acquire), started.load(Ordering::Acquire));
    manager.disable("git").unwrap();
    assert!(!manager.live.contains_key("git"));
    assert_eq!(manager.resource_count(), 0);
}
