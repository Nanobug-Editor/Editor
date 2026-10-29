//! Actual native file-tree input must keep one live Git incarnation through sustained switching.
use super::*;
mod large_sources;

/// Construct a finite changed-file tree and enough history rows to exercise retained log drawing.
fn repository(root: &Path) {
    for args in [
        vec!["init", "-b", "main"],
        vec!["config", "core.autocrlf", "false"],
        vec!["config", "commit.gpgsign", "false"],
        vec!["config", "user.name", "Repeated tree selection"],
        vec!["config", "user.email", "tree-selection@example.invalid"],
    ] {
        assert!(git(root, &args));
    }
    for revision in 0..32 {
        for index in 0..12 {
            let file = format!("file-{index:02}.txt");
            std::fs::write(root.join(&file), content(&file, revision)).unwrap();
        }
        assert!(git(root, &["add", "--", "*.txt"]));
        assert!(git(
            root,
            &[
                "commit",
                "-m",
                &format!("Change twelve files {revision:02}")
            ]
        ));
    }
}

/// Each file has a visible differing first line and enough unchanged rows for native layout.
fn content(file: &str, revision: usize) -> String {
    let mut text = format!("revision {revision}: {file}\n");
    for row in 0..160 {
        text.push_str(&format!("unchanged row {row:03}: {file}\n"));
    }
    text
}

/// Resolve the current retained tree identity; no guest node naming scheme is reproduced here.
fn click_file(manager: &Manager, visual: &mut VisualTestContext, file: &str) {
    let node = manager.live["git"]
        .tree_resources()
        .into_iter()
        .flat_map(|tree| tree.snapshot().nodes.into_values())
        .find(|node| !node.branch && node.label == file)
        .expect("the complete latest commit file tree is published")
        .id;
    press(visual, format!("plugin-tree-row-{node}"), 1);
}

/// Read-only peak evidence distinguishes resource growth and excessive callbacks from UI replacement.
#[derive(Default, Debug)]
struct Peaks {
    /// Largest measured guest callback, independent of elapsed time on the test machine.
    fuel: u64,
    /// Largest live handle count sampled at an actual native publication.
    handles: usize,
    /// Largest number of outstanding public requests during switching.
    requests: usize,
    /// Largest owned tree/document/decoration count during source replacement.
    projections: usize,
}

impl Peaks {
    /// Check one actual publication without clearing diagnostic state or hiding a replacement.
    fn inspect(&mut self, manager: &Manager, incarnation: &str) {
        let diagnostics = manager.diagnostics("git");
        assert_eq!(
            manager.instance_id("git"),
            Some(incarnation),
            "tree input stopped or replaced the Git incarnation: {diagnostics:?}"
        );
        assert!(diagnostics.is_empty(), "tree input fault: {diagnostics:?}");
        let instance = &manager.live["git"];
        assert!(
            instance.error.is_none(),
            "guest error: {:?}",
            instance.error
        );
        let resources = instance.resource_counts();
        assert!(!resources.retired, "the visible tree's owner was retired");
        self.handles = self.handles.max(resources.handles);
        self.requests = self.requests.max(resources.requests);
        self.projections = self.projections.max(resources.projections);
        assert_no_inline_stream_failure(manager);
        let anomalies = manager
            .runtime_logs()
            .records("git")
            .into_iter()
            .filter(|record| record.level != plugin_runtime::LogLevel::Info)
            .collect::<Vec<_>>();
        assert!(
            anomalies.is_empty(),
            "ordinary tree input must not produce stream loss or runtime warnings: {anomalies:?}"
        );
    }

    /// Fold the current bounded capture before starting another interval, preserving its fuel peak.
    fn capture(&mut self, manager: &Manager) {
        let evidence = manager.runtime_observations().snapshot(Some("git"));
        self.fuel = self.fuel.max(
            evidence
                .records
                .iter()
                .filter_map(|record| record.fuel)
                .max()
                .unwrap_or(0),
        );
    }
}

/// Physical clicks replace native sources repeatedly; no injected stream failure can make this pass.
#[gpui::test]
#[ignore = "actual independent Git ZIP and native comparison; set NANOBUG_GIT_CANDIDATE"]
fn git_history_tree_stress_keeps_one_instance_and_no_stream_loss(cx: &mut TestAppContext) {
    if !run_isolated_test(
        "extensions::windows_tests::history_preview::ownership::stress::git_history_tree_stress_keeps_one_instance_and_no_stream_loss",
    ) {
        return;
    }
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let root = path.parent().unwrap();
        repository(root);
        let package = Package::read(Path::new(
            &std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate ZIP"),
        ))
        .unwrap();
        let profile = tempfile::tempdir().unwrap();
        let env = visual.update(|_, cx| environment(root, cx));
        let mut manager = Manager::open(profile.path().into(), env).unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        package_ui_test_support::demand(
            &mut manager,
            "git",
            plugin_runtime::DemandReason::View("history".into()),
        )
        .unwrap();
        flush(&mut manager, &app, visual);
        select_row(&mut manager, 0);
        flush(&mut manager, &app, visual);
        let incarnation = manager.instance_id("git").unwrap().to_owned();
        let observations = manager.runtime_observations();
        let mut peaks = Peaks::default();
        let mut clicks = 0;
        observations.start_capture();
        for step in 0..96 {
            // These first three visible sibling rows avoid synthetic tree events or scroll jumps.
            let file = format!("file-{:02}.txt", step % 3);
            click_file(&manager, visual, &file);
            clicks += 1;
            let expected = content(&file, 31);
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                flush_checked(&mut manager, &app, visual, |manager, _| {
                    peaks.inspect(manager, &incarnation);
                });
                let visible = visual.update(|_, cx| {
                    let state = app.read(cx);
                    state.comparison_views.len() == 1
                        && state.editor.read(cx).value().as_str() == expected
                });
                if visible {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "switch {step} did not show the exact last requested file {file}; {peaks:?}"
                );
            }
            if step % 12 == 11 {
                // Three additional native presses arrive before query completion. The final row
                // returns to this iteration's exact target so delayed results cannot hide a loss.
                for offset in [1, 2, 0] {
                    let next = format!("file-{:02}.txt", (step + offset) % 3);
                    click_file(&manager, visual, &next);
                    clicks += 1;
                }
                flush_checked(&mut manager, &app, visual, |manager, _| {
                    peaks.inspect(manager, &incarnation);
                });
                visual.update(|_, cx| {
                    assert_eq!(
                        app.read(cx).editor.read(cx).value().as_str(),
                        expected,
                        "rapid tree input must keep the final requested file"
                    );
                });
                peaks.capture(&manager);
                observations.start_capture();
            }
        }
        peaks.capture(&manager);
        observations.stop_capture();
        assert_eq!(clicks, 120);
        eprintln!("Git actual tree stress: clicks={clicks}, {peaks:?}");
        manager.disable("git").unwrap();
        flush(&mut manager, &app, visual);
    });
}
