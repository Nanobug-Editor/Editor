//! Observe actual native lazy-page delivery in an empty project without inventing manager events.
use super::*;
mod retry;
pub(super) use retry::click as retry;

/// Install the frozen candidate in disposable storage; the real actor alone activates its guest.
pub(super) fn prepare(workspace: &Path, profile: &Path) -> bool {
    let Some(candidate) = std::env::var_os("NANOBUG_CLOSE_GIT_PACKAGE") else {
        return false;
    };
    let package = Package::read(Path::new(&candidate)).unwrap();
    let mut manager = Manager::open(
        extensions::profile_runtime_root(profile),
        Environment {
            workspace: workspace.display().to_string(),
            os: "windows".into(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    manager.shutdown_checked().unwrap();
    true
}

/// Draw the ordinary restored surface, then inspect its resource-issued root page and real actor logs.
pub(super) async fn observe(
    worker: &Worker,
    main: gpui_kit::AnyWindowHandle,
    details: gpui_kit::AnyWindowHandle,
    cx: &mut gpui_kit::AsyncApp,
) -> bool {
    let deadline = Instant::now() + Duration::from_secs(90);
    let mut last = String::new();
    loop {
        main.update(cx, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
        details
            .update(cx, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
        let (ready, failed, diagnostic) = {
            let state = worker.state.lock().unwrap();
            let root = state
                .views
                .get("git/commit-details")
                .and_then(|view| view.active_node("files"))
                .and_then(|node| match &node.kind {
                    protocol::ui::Kind::Tree { handle } => {
                        state.trees.iter().find(|tree| tree.handle() == handle)
                    }
                    _ => None,
                })
                .and_then(|tree| tree.snapshot().branches.get(&None).cloned());
            let ready = root
                .as_ref()
                .is_some_and(|root| root.loaded && !root.loading && root.error.is_none());
            let failed = root.as_ref().is_some_and(|root| root.error.is_some());
            let diagnostic = format!(
                "root={:?}; logs={:?}",
                root.as_ref().map(|root| (
                    root.loaded,
                    root.loading,
                    &root.error,
                    root.children.len()
                )),
                state
                    .logs
                    .records("git")
                    .into_iter()
                    .filter(|record| record.level > plugin_runtime::logs::LogLevel::Info
                        || matches!(record.source.as_str(), "host.ui.stale" | "host.ui.retired"))
                    .rev()
                    .take(6)
                    .map(|record| (record.source.clone(), record.message.clone()))
                    .collect::<Vec<_>>()
            );
            (ready, failed, diagnostic)
        };
        if diagnostic != last {
            eprintln!("native loading: {diagnostic}");
            last = diagnostic;
        }
        if ready {
            eprintln!("native loading: restored non-Git details root completed");
            return true;
        }
        if failed || Instant::now() >= deadline {
            return false;
        }
        cx.background_executor()
            .timer(Duration::from_millis(50))
            .await;
    }
}
