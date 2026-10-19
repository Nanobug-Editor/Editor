//! Await production actor publications and native frames without inventing readiness or blocking UI threads.
use super::*;
use std::collections::BTreeMap;

/// Handles retain the real project and actor; this adapter owns observations, never document text.
pub(super) struct Harness {
    pub case: Case,
    pub fixture: Rc<Fixture>,
    pub main: AnyWindowHandle,
    pub app: Entity<EditorApp>,
    pub worker: Arc<Worker>,
    evidence: RefCell<serde_json::Value>,
    /// Logging is rate limited independently from readiness and never schedules synthetic frames.
    last_frame_log: Cell<Option<Instant>>,
}
impl Harness {
    pub fn new(
        case: Case,
        fixture: Rc<Fixture>,
        main: AnyWindowHandle,
        app: Entity<EditorApp>,
        worker: Arc<Worker>,
    ) -> Self {
        Self {
            case,
            fixture,
            main,
            app,
            worker,
            last_frame_log: Cell::new(None),
            evidence: RefCell::new(serde_json::json!({
                "case": case.name,
                "workflow": if exclusive::requested() {
                    "exclusive-dock-only"
                } else if sidebar::requested() {
                    "shared-sidebar-only"
                } else if caption_controls::requested() {
                    "caption-controls-loaded-commit"
                } else if std::env::var_os("NANOBUG_GIT_NATIVE_IDLE_RETENTION").is_some() {
                    "idle-refresh-state-only"
                } else if std::env::var_os("NANOBUG_GIT_NATIVE_HISTORY_ONLY").is_some() {
                    "history-keyboard-only"
                } else {
                    "complete-workbench"
                },
                "theme": if case.dark { "dark" } else { "light" },
                "locale": case.locale,
                "requested_scale_percent": case.percent,
                "dpi_method": "WM_DPICHANGED sent only to this process's own ordinary HWND; OS display settings unchanged",
                "os_ime_candidates_verified": false,
                "shutdown_complete": false,
                "screenshots": {},
                "observations": {}
            })),
        }
    }

    /// Record fixed fixture assertions and actual geometry alongside this candidate's native images.
    pub fn record(&self, name: &str, value: serde_json::Value) {
        self.evidence.borrow_mut()["observations"][name] = value;
    }

    /// Persist partial progress before returning an error or starting cleanup, without a success receipt.
    pub fn checkpoint(&self, name: &str, value: serde_json::Value) -> anyhow::Result<()> {
        self.record(name, value);
        self.save_evidence(false)
    }

    /// The image and metadata come from one current D3D target after the same native scene draw.
    pub fn capture(
        &self,
        target: AnyWindowHandle,
        name: &str,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        let result = target.update(cx, |_, window, cx| {
            window.draw(cx).clear(cx);
            windows::capture(window, &self.fixture.output.join(name))
        })??;
        ensure!(
            (result["scale_factor"].as_f64().unwrap_or_default() - self.case.percent as f64 / 100.)
                .abs()
                < 0.001,
            "image uses an unintended DPI: {result}"
        );
        self.evidence.borrow_mut()["screenshots"][name] = result;
        self.save_evidence(false)
    }

    /// Only the native child writes this receipt after every resource owner acknowledges shutdown.
    pub fn finish_evidence(&self) -> anyhow::Result<()> {
        // This scenario deliberately exercises successful UI flows. A functional outcome must not
        // conceal an error reminder left by cold observations or a rejected host operation.
        let errors = self
            .worker
            .state
            .lock()
            .unwrap()
            .logs
            .records("git")
            .into_iter()
            .filter(|record| record.level == plugin_runtime::logs::LogLevel::Error)
            .map(|record| format!("{}: {}", record.source, record.message))
            .collect::<Vec<_>>();
        self.checkpoint("unexpected_error_logs", serde_json::json!(errors))?;
        ensure!(errors.is_empty(), "unexpected plugin errors: {errors:?}");
        self.save_evidence(true)
    }

    fn save_evidence(&self, shutdown: bool) -> anyhow::Result<()> {
        let mut evidence = self.evidence.borrow_mut();
        evidence["candidate_sha256"] = self.fixture.package_digest.clone().into();
        evidence["shutdown_complete"] = shutdown.into();
        fs::write(
            self.fixture.output.join("evidence.json"),
            serde_json::to_vec_pretty(&*evidence)?,
        )?;
        Ok(())
    }

    /// Commands keep the production admission guard; their immutable native target is never replaced.
    pub fn command(
        &self,
        id: &str,
        args: serde_json::Value,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        self.main
            .update(cx, |_, window, cx| {
                self.app.update(cx, |app, cx| {
                    app.invoke_plugin_command("git", id, args, window, cx)
                })
            })?
            .map_err(anyhow::Error::msg)
    }

    /// Restored visible panels demand their guest through the normal production view path.
    /// Do not enqueue an extra cold command and mistake another demand's view for its completion.
    pub async fn activate(&self, cx: &mut AsyncApp) -> anyhow::Result<()> {
        eprintln!("native {}: wait visible-panel activation", self.case.name);
        self.settle(cx).await?;
        // Exercise manual refresh only after first birth, using its actually painted native button.
        self.click_button(self.main, "refresh", cx).await
    }

    /// Require the current public guest Node and matching host/worker/window. The click adapter
    /// additionally checks Base's focus-handle metadata, which distinguishes an enabled native
    /// button from a temporarily disabled form activation; actual effects remain mandatory.
    pub fn node_current(&self, target: AnyWindowHandle, id: &str, cx: &App) -> bool {
        let revisions = self
            .worker
            .state
            .lock()
            .unwrap()
            .views
            .iter()
            .map(|(key, scene)| (key.clone(), scene.revision))
            .collect::<BTreeMap<_, _>>();
        let app = self.app.read(cx);
        app.plugin_panels.iter().any(|(key, panel)| {
            let panel = panel.read(cx);
            let owns_window = if panel.work_window {
                app.plugin_windows
                    .get(key)
                    .is_some_and(|window| AnyWindowHandle::from(*window) == target)
            } else {
                target == self.main && !panel.editor_preview
            };
            owns_window
                && panel.visible.get()
                && panel.current_document().is_some_and(|scene| {
                    revisions.get(key) == Some(&scene.revision)
                        && scene.active_node(id).is_some_and(|node| !node.disabled)
                        // A prior native edit may precede its document-event/read receipt.
                        // Scene equality alone cannot authorize proposals for an older source.
                        && scene.active_native_editor().is_none_or(|expected| {
                            (0..app.tabs.len()).any(|index| {
                                app.plugin_document_version(index)
                                    .is_ok_and(|actual| &actual == expected)
                            })
                        })
                })
        })
    }

    /// `active_node` excludes disabled targets. Join that public admission to the actually painted
    /// Base button and the same host publication before counting stable native turns.
    pub async fn settle(&self, cx: &mut AsyncApp) -> anyhow::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(90);
        let mut previous = None;
        let mut stable = 0;
        let mut last_progress = Instant::now();
        let mut captured = false;
        eprintln!("native {}: settle begin", self.case.name);
        loop {
            self.frame(self.main, cx).await?;
            let (ready, scenes) = {
                let state = self.worker.state.lock().unwrap();
                (
                    state
                        .views
                        .get("git/history")
                        .and_then(|view| view.active_node("refresh"))
                        .is_some_and(|node| !node.disabled)
                        && state.processes.get("git").copied().unwrap_or(0) == 0,
                    state
                        .views
                        .iter()
                        .map(|(key, scene)| (key.clone(), scene.revision))
                        .collect::<BTreeMap<_, _>>(),
                )
            };
            let native_ready = self.main.update(cx, |_, window, cx| {
                let owner = self.app.read(cx);
                let actual = owner
                    .plugin_panels
                    .get("git/history")
                    .and_then(|panel| panel.read(cx).current_document());
                actual
                    .as_ref()
                    .is_some_and(|scene| scenes.get("git/history") == Some(&scene.revision))
                    && input::button(window, "refresh")
                        .is_ok_and(|button| button.disabled() != Some(true))
            })?;
            if !captured && ready {
                // Save real idle-worker pixels even if native publication is lagging. This is
                // diagnostic output and never counts as a passing workflow receipt.
                self.dump_diagnostics("debug-workbench", cx)?;
                captured = true;
            }
            stable = if ready && native_ready && previous.as_ref() == Some(&scenes) {
                stable + 1
            } else {
                0
            };
            previous = Some(scenes);
            if stable >= 2 {
                eprintln!("native {}: settle ready", self.case.name);
                return Ok(());
            }
            if last_progress.elapsed() >= Duration::from_secs(5) {
                eprintln!(
                    "native {}: settling; native_ready={native_ready}; {}",
                    self.case.name,
                    self.diagnostic()
                );
                last_progress = Instant::now();
            }
            if Instant::now() >= deadline {
                self.dump_diagnostics("timeout-workbench", cx)?;
            }
            ensure!(
                Instant::now() < deadline,
                "workbench never became actionable: {}; native_ready={native_ready}",
                self.diagnostic()
            );
        }
    }

    /// Native tree pages and code measurements settle independently from the Git serial queue.
    pub async fn until(
        &self,
        target: AnyWindowHandle,
        label: &str,
        mut predicate: impl FnMut(&mut Window, &mut App) -> anyhow::Result<bool>,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(45);
        let mut last_progress = Instant::now();
        eprintln!("native {}: wait {label}", self.case.name);
        loop {
            self.frame(target, cx).await?;
            if target.update(cx, |_, window, cx| predicate(window, cx))?? {
                eprintln!("native {}: ready {label}", self.case.name);
                return Ok(());
            }
            if last_progress.elapsed() >= Duration::from_secs(5) {
                eprintln!(
                    "native {}: waiting {label}; {}",
                    self.case.name,
                    self.diagnostic()
                );
                last_progress = Instant::now();
            }
            if Instant::now() >= deadline {
                self.dump_window_diagnostics(target, "timeout-wait", cx)?;
            }
            ensure!(
                Instant::now() < deadline,
                "{label} timed out: {}",
                self.diagnostic()
            );
        }
    }

    /// Native actions enqueue asynchronous guest work. Observe its actual repository effect on a
    /// background thread while ordinary native frames keep the actor and current windows moving.
    /// An idle scene alone is insufficient: the input event may not have reached the guest yet.
    pub async fn until_repository(
        &self,
        label: &str,
        predicate: impl Fn(&Path) -> anyhow::Result<bool> + Send + Sync + 'static,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(45);
        let predicate = Arc::new(predicate);
        let mut last_progress = Instant::now();
        eprintln!("native {}: wait {label}", self.case.name);
        loop {
            self.frame(self.main, cx).await?;
            let root = self.fixture.workspace.clone();
            let observe = predicate.clone();
            let complete = cx
                .background_executor()
                .spawn(async move { observe(&root) })
                .await
                .with_context(|| format!("repository observation {label} failed"))?;
            if complete {
                eprintln!("native {}: ready {label}", self.case.name);
                return Ok(());
            }
            if last_progress.elapsed() >= Duration::from_secs(5) {
                eprintln!(
                    "native {}: waiting {label}; {}",
                    self.case.name,
                    self.diagnostic()
                );
                last_progress = Instant::now();
            }
            if Instant::now() >= deadline {
                self.dump_diagnostics("timeout-repository", cx)?;
                return Err(anyhow!("{label} timed out: {}", self.diagnostic()));
            }
        }
    }

    /// Await a genuine platform callback outside all entity/App borrows. Every work window draws through
    /// its own Root and production source mount; no fake next-frame delivery or alternate editor exists.
    pub async fn frame(&self, target: AnyWindowHandle, cx: &mut AsyncApp) -> anyhow::Result<()> {
        let trace = self
            .last_frame_log
            .get()
            .is_none_or(|last| last.elapsed() >= Duration::from_secs(5));
        if trace {
            self.last_frame_log.set(Some(Instant::now()));
            eprintln!("native {}: frame poll/draw begin", self.case.name);
        }
        self.main.update(cx, |_, window, cx| {
            self.app
                .read(cx)
                .extensions
                .clone()
                .update(cx, |owner, cx| owner.poll(cx));
            self.app.update(cx, |app, cx| {
                app.sync_plugin_panels(window, cx);
                cx.notify();
            });
            window.draw(cx).clear(cx);
        })?;
        if trace {
            eprintln!("native {}: main frame drawn", self.case.name);
        }
        let windows = cx.update(|cx| {
            self.app
                .read(cx)
                .plugin_windows
                .values()
                .copied()
                .map(AnyWindowHandle::from)
                .collect::<Vec<_>>()
        });
        for separate in windows {
            separate.update(cx, |_, window, cx| {
                window.draw(cx).clear(cx);
            })?;
        }
        if trace {
            eprintln!("native {}: native next-frame wait", self.case.name);
        }
        let (send, receive) = futures::channel::oneshot::channel();
        target.update(cx, |_, window, _| {
            window.on_next_frame(move |_, _| {
                let _ = send.send(());
            })
        })?;
        let timeout = cx.background_executor().timer(Duration::from_secs(5));
        match futures::future::select(receive, Box::pin(timeout)).await {
            futures::future::Either::Left((result, _)) => {
                if trace {
                    eprintln!("native {}: native next-frame delivered", self.case.name);
                }
                result.context("native frame callback dropped")
            }
            futures::future::Either::Right(_) => Err(anyhow!("native frame callback timed out")),
        }
    }

    /// WM_DPICHANGED reenters platform callbacks, so send it only after releasing the GPUI update borrow.
    pub async fn dpi(&self, target: AnyWindowHandle, cx: &mut AsyncApp) -> anyhow::Result<()> {
        eprintln!("native {}: DPI inspect begin", self.case.name);
        let current = target.update(cx, |_, window, _| window.scale_factor())?;
        eprintln!(
            "native {}: DPI actual={current}, requested={} percent",
            self.case.name, self.case.percent
        );
        if (current - self.case.percent as f32 / 100.).abs() < 0.001 {
            // capture() still verifies physical client/D3D/logical sizes independently; no
            // platform transition is needed when the native window already has the requested scale.
            eprintln!("native {}: DPI already matches", self.case.name);
            return Ok(());
        }
        eprintln!("native {}: DPI prepare", self.case.name);
        let request = target.update(cx, |_, window, _| {
            windows::DpiRequest::for_window(window, self.case.percent)
        })??;
        eprintln!("native {}: DPI apply begin", self.case.name);
        request.apply();
        eprintln!("native {}: DPI apply returned", self.case.name);
        self.until(
            target,
            "native DPI resize",
            |window, _| Ok((window.scale_factor() - self.case.percent as f32 / 100.).abs() < 0.001),
            cx,
        )
        .await
    }

    /// Persist real pixels and bounded public observation metadata without document values or environment.
    fn dump_diagnostics(&self, label: &str, cx: &mut AsyncApp) -> anyhow::Result<()> {
        self.dump_window_diagnostics(self.main, label, cx)
    }

    /// Tree expansion is a resource epoch, not merely Base's current chevron. Capture both sides
    /// so a rejected page request or a missing native reconciliation remains distinguishable.
    fn dump_window_diagnostics(
        &self,
        target: AnyWindowHandle,
        label: &str,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        let (revisions, resources, errors) = {
            let state = self.worker.state.lock().unwrap();
            let revisions = state
                .views
                .iter()
                .map(|(key, scene)| (key.clone(), scene.revision))
                .collect::<BTreeMap<_, _>>();
            let resources = state.trees.iter().map(|resource| {
                let snapshot = resource.snapshot();
                let mut surfaces = Vec::new();
                for (panel, view) in &state.views {
                    view.root.visit(&mut |node| {
                        if matches!(&node.kind, plugin_runtime::plugin_protocol::ui::Kind::Tree { handle } if handle == resource.handle()) {
                            surfaces.push(format!("{panel}/{}", node.id));
                        }
                    });
                }
                let nodes = snapshot.nodes.iter().take(128).map(|(id, node)| serde_json::json!({"id": id, "label": node.label, "branch": node.branch, "disabled": node.disabled})).collect::<Vec<_>>();
                let branches = snapshot.branches.iter().take(128).map(|(parent, branch)| serde_json::json!({"parent": parent, "loaded": branch.loaded, "loading": branch.loading, "error": branch.error, "children": branch.children, "next": branch.next})).collect::<Vec<_>>();
                serde_json::json!({"handle": resource.handle(), "surfaces": surfaces, "live": resource.is_live(), "epoch": snapshot.epoch, "revision": snapshot.revision, "expanded": snapshot.expanded, "nodes": nodes, "branches": branches})
            }).collect::<Vec<_>>();
            // Rejected native callbacks are Info, but explain a page left loading after its
            // captured revision expired. Preserve them with warnings/errors instead of hiding
            // the admission failure behind the later tree timeout; keep each record's real level.
            let errors = state.logs.records("git").into_iter().filter(|record| {
                record.level > plugin_runtime::logs::LogLevel::Info
                    || matches!(record.source.as_str(), "host.ui.stale" | "host.ui.retired")
            }).rev().take(12).map(|record| serde_json::json!({"level": format!("{:?}", record.level), "source": record.source, "message": record.message})).collect::<Vec<_>>();
            (revisions, resources, errors)
        };
        let details = target.update(cx, |_, window, cx| -> anyhow::Result<serde_json::Value> {
            window.draw(cx).clear(cx);
            let image = windows::capture(window, &self.fixture.output.join(format!("{label}.png")))?;
            let widgets = gpui_base::test_support::snapshots(window).into_iter().take(160).map(|node| serde_json::json!({
                "role": format!("{:?}", node.role()), "path": format!("{:?}", node.path()),
                "label": node.label(), "bounds": format!("{:?}", node.bounds()),
                "visible": node.visible(), "disabled": node.disabled(), "selected": node.selected(), "expanded": node.expanded(),
            })).collect::<Vec<_>>();
            let panels = self.app.read(cx).plugin_panels.iter().map(|(key, panel)| {
                let panel = panel.read(cx);
                serde_json::json!({"key": key, "active": panel.active, "surface": panel.surface_id, "visible": panel.visible.get(), "host_revision": panel.current_document().map(|scene| scene.revision), "worker_revision": revisions.get(key), "bounds": format!("{:?}", panel.bounds)})
            }).collect::<Vec<_>>();
            Ok(serde_json::json!({"image": image, "widgets": widgets, "panels": panels, "tree_resources": resources, "plugin_errors": errors}))
        })??;
        fs::write(
            self.fixture.output.join(format!("{label}.json")),
            serde_json::to_vec_pretty(&details)?,
        )?;
        eprintln!("native {}: saved {label}.png/json", self.case.name);
        Ok(())
    }

    /// Status diagnostics describe bounded metadata only; they never dump document contents or credentials.
    fn diagnostic(&self) -> String {
        let state = self.worker.state.lock().unwrap();
        format!(
            "ready={} processes={:?} pending_editor={} scenes={:?} status={:?}",
            state.ready,
            state.processes.get("git"),
            state.editor_requests.len(),
            state
                .views
                .iter()
                .map(|(id, view)| (
                    id,
                    view.revision,
                    view.active_node("refresh").map(|node| node.disabled)
                ))
                .collect::<Vec<_>>(),
            state.status.as_ref().map(|status| &status.message)
        )
    }
}
