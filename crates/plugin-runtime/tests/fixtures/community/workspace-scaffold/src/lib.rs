//! An independent scaffolder combines unsaved native text, disk replacement and ordered resources.
//! Only immutable proposal inputs and owned handles are retained; all live document state stays native.
use plugin_protocol::{
    api,
    bindings::{Guest, export},
    ui,
    workspace::{self, Change, FileStamp, FileTarget, Plan},
};
use std::{cell::RefCell, collections::BTreeMap};

thread_local! {
    /// Serial component dispatch keeps command intent local to the currently active instance.
    static STATE: RefCell<Scaffold> = RefCell::new(Scaffold::default());
}

#[derive(Default)]
enum Stage {
    #[default]
    Idle,
    Documents,
    Snapshot(api::DocumentVersion),
    Metadata {
        document: api::DocumentVersion,
        text: String,
        next: usize,
    },
    Batch,
}

/// Recovery references are current-session resources; snapshots never serialize them across reload.
#[derive(Default)]
struct Scaffold {
    task: Option<api::guest::EditorTask>,
    request: Option<api::ResourceHandle>,
    root: Option<api::ResourceHandle>,
    watch: Option<api::ResourceHandle>,
    batch: Option<api::ResourceHandle>,
    stage: Stage,
    stamps: BTreeMap<String, FileStamp>,
    status: String,
    events: Vec<api::Notification>,
}

impl Scaffold {
    /// Replacing an intent stops waiting, and reports no promise of rollback for earlier effects.
    fn start(&mut self, operation: api::EditorOperation, stage: Stage) -> Result<(), api::Failure> {
        self.cancel();
        let handle = api::guest::editor(operation, 120_000)?;
        self.request = Some(handle.clone());
        self.task = Some(api::guest::EditorTask::from_accepted(handle));
        self.stage = stage;
        Ok(())
    }

    fn cancel(&mut self) {
        if let Some(request) = self.request.take() {
            let _ = api::guest::cancel_request(&request, api::CancelMode::StopWaiting);
        }
        self.task = None;
    }

    /// Portable targets retain the original workspace resource instead of inferring native paths.
    fn target(&self, path: &str) -> FileTarget {
        FileTarget {
            root: self.root.as_ref().unwrap().clone(),
            path: path.into(),
        }
    }

    fn advance(&mut self, value: api::EditorValue) -> Result<(), api::Failure> {
        match (std::mem::take(&mut self.stage), value) {
            (Stage::Documents, api::EditorValue::Documents { documents, active }) => {
                let document = documents
                    .into_iter()
                    .find(|info| active.as_ref() == Some(&info.document))
                    .ok_or_else(|| {
                        api::Failure::new(api::ErrorCode::NotFound, "Open community-live.txt first")
                    })?
                    .document;
                self.start(
                    api::EditorOperation::ReadDocument {
                        document: document.clone(),
                        range: None,
                    },
                    Stage::Snapshot(document),
                )?;
            }
            (Stage::Snapshot(document), api::EditorValue::DocumentSnapshot(snapshot)) => {
                self.start_metadata(document, snapshot.text, 0)?;
            }
            (
                Stage::Metadata {
                    document,
                    text,
                    next,
                },
                api::EditorValue::Workspace(workspace::Value::Metadata(metadata)),
            ) => {
                self.stamps.insert(metadata.target.path, metadata.stamp);
                if next + 1 < 3 {
                    self.start_metadata(document, text, next + 1)?;
                } else {
                    self.submit(document, text)?;
                }
            }
            (Stage::Batch, api::EditorValue::Workspace(workspace::Value::Batch(receipt))) => {
                self.status = serde_json::to_string(&receipt).unwrap();
            }
            (Stage::Idle, value) => {
                self.status = serde_json::to_string(&Ok::<_, api::Failure>(value)).unwrap();
            }
            _ => {
                return Err(api::Failure::new(
                    api::ErrorCode::InvalidState,
                    "Unexpected scaffolding result",
                ));
            }
        }
        Ok(())
    }

    fn start_metadata(
        &mut self,
        document: api::DocumentVersion,
        text: String,
        next: usize,
    ) -> Result<(), api::Failure> {
        let path = [
            "community-disk.txt",
            "community-template.txt",
            "community-remove.txt",
        ][next];
        self.start(
            api::EditorOperation::Workspace {
                operation: workspace::Operation::Stat {
                    target: self.target(path),
                },
            },
            Stage::Metadata {
                document,
                text,
                next,
            },
        )
    }

    /// Both nonadjacent ranges address the same captured UTF-8 source; the host builds one Atomic edit.
    fn submit(&mut self, document: api::DocumentVersion, text: String) -> Result<(), api::Failure> {
        let mut edits = Vec::new();
        for (from, to) in [("旧A", "生成A"), ("旧B", "生成B")] {
            let start = text.find(from).ok_or_else(|| {
                api::Failure::new(
                    api::ErrorCode::InvalidRequest,
                    "The scaffolding source requires 旧A and 旧B",
                )
            })?;
            edits.push(api::TextEdit {
                range: api::TextRange {
                    start,
                    end: start + from.len(),
                },
                text: to.into(),
            });
        }
        let plan = Plan {
            origin: None,
            title: "Scaffold workspace / 生成工作区".into(),
            changes: vec![
                Change::EditDocument {
                    document,
                    edits,
                    selection: api::TextRange { start: 0, end: 0 },
                },
                Change::CreateDirectory {
                    target: self.target("community-generated"),
                },
                Change::Write {
                    target: self.target("community-disk.txt"),
                    expected: Some(self.stamps["community-disk.txt"].clone()),
                    bytes: "脚手架磁盘 中文🙂\r\n".as_bytes().to_vec(),
                },
                Change::Copy {
                    source: self.target("community-template.txt"),
                    target: self.target("community-generated/copied.txt"),
                    expected_source: self.stamps["community-template.txt"].clone(),
                },
                Change::Rename {
                    source: self.target("community-generated/copied.txt"),
                    target: self.target("community-generated/intermediate.txt"),
                    expected_source: FileStamp::FromPlan { change: 3 },
                },
                Change::Rename {
                    source: self.target("community-generated/intermediate.txt"),
                    target: self.target("community-generated/final.txt"),
                    expected_source: FileStamp::FromPlan { change: 4 },
                },
                Change::Write {
                    target: self.target("community-generated/README.txt"),
                    expected: Some(FileStamp::Missing),
                    bytes: "Scaffold / 脚手架 中文🙂\r\n".as_bytes().to_vec(),
                },
                Change::Delete {
                    target: self.target("community-remove.txt"),
                    expected: self.stamps["community-remove.txt"].clone(),
                },
            ],
        };
        let (request, batch) = workspace::submit(plan, 120_000)?;
        self.request = Some(request.clone());
        self.task = Some(api::guest::EditorTask::from_accepted(request));
        self.batch = Some(batch);
        self.stage = Stage::Batch;
        self.status = "Review the native preview / 请确认原生预览".into();
        Ok(())
    }

    /// Probes are ordinary declared commands using the identical SDK transport as the business plan.
    fn input(&mut self, input: api::Input) -> Result<(), api::Failure> {
        match input {
            api::Input::Event {
                event: api::Notification::Command { id, arguments, .. },
                ..
            } => match id.as_str() {
                "probe" => {
                    let result = arguments
                        .ok_or_else(|| {
                            api::Failure::new(api::ErrorCode::InvalidRequest, "Operation required")
                        })
                        .and_then(|arguments| {
                            serde_json::from_value(arguments).map_err(|error| {
                                api::Failure::new(api::ErrorCode::InvalidRequest, error.to_string())
                            })
                        })
                        .and_then(api::guest::request);
                    self.status = serde_json::to_string(&result).unwrap();
                }
                "events" => self.status = serde_json::to_string(&self.events).unwrap(),
                "scaffold" => {
                    if let Some(watch) = self.watch.take() {
                        let _ = api::guest::close_resource(watch);
                    }
                    if let Some(batch) = self.batch.take() {
                        let _ = api::guest::close_resource(batch);
                    }
                    if let Some(root) = self.root.take() {
                        let _ = api::guest::close_resource(root);
                    }
                    self.root = Some(api::guest::open_workspace()?);
                    self.watch = Some(workspace::subscribe(self.root.as_ref().unwrap().clone())?);
                    self.stamps.clear();
                    self.start(api::EditorOperation::ListDocuments, Stage::Documents)?;
                }
                "undo" | "status" => {
                    let batch = self.batch.clone().ok_or_else(|| {
                        api::Failure::new(api::ErrorCode::NotFound, "No retained batch")
                    })?;
                    let operation = if id == "undo" {
                        workspace::Operation::UndoBatch { batch }
                    } else {
                        workspace::Operation::BatchStatus { batch }
                    };
                    self.start(api::EditorOperation::Workspace { operation }, Stage::Batch)?;
                }
                "cancel" => {
                    self.cancel();
                    self.status = "Stopped waiting; inspect receipt for actual effects / 已停止等待，请查看实际结果".into();
                }
                _ => {}
            },
            api::Input::Event { event, .. } => {
                if matches!(
                    &event,
                    api::Notification::DocumentEvent { .. }
                        | api::Notification::WorkspaceFile { .. }
                        | api::Notification::SubscriptionFailed { .. }
                ) {
                    self.events.push(event.clone());
                    if self.events.len() > 32 {
                        self.events.remove(0);
                    }
                    // Observation display is independent of the native queue and bounded to one small UI node.
                    while serde_json::to_vec(&self.events).unwrap().len() > 24 * 1024 {
                        self.events.remove(0);
                    }
                }
                match self.task.as_mut().and_then(|task| task.update(&event)) {
                    Some(api::RequestUpdate::Completed { result }) => {
                        self.task = None;
                        self.request = None;
                        if let Err(error) = result.and_then(|value| self.advance(value)) {
                            self.status = error.to_string();
                        }
                    }
                    Some(api::RequestUpdate::Cancelled { reason, .. }) => {
                        self.task = None;
                        self.request = None;
                        self.status = format!("{reason:?}; inspect retained batch");
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(())
    }
}

struct Scaffolder;
impl Guest for Scaffolder {
    /// User-command failures remain visible without retiring a valid batch recovery resource.
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| {
            STATE.with(|state| {
                let mut state = state.borrow_mut();
                if let Err(error) = state.input(input) {
                    state.status = error.to_string();
                }
                Ok(api::Output {
                    snapshot: Some(Default::default()),
                    views: vec![api::View {
                        panel: "scaffold".into(),
                        document: ui::Document::new(ui::Node::text(
                            "scaffold-result",
                            state.status.clone(),
                        )),
                    }],
                    ..Default::default()
                })
            })
        })
    }
}
export!(Scaffolder);
