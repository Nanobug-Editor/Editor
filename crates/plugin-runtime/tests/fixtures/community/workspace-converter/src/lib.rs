//! Independent text conversion uses paged discovery and reads authorized unsaved native buffers.
//! Conversion policy stays in this guest; the host owns preview, version checks, disk IO and recovery.
use plugin_protocol::{
    api,
    bindings::{Guest, export},
    interaction, ui,
    workspace::{self, Change, FileTarget, Plan},
};
use std::{cell::RefCell, collections::VecDeque};

thread_local! {
    /// Queue entries are immutable proposal inputs; this plugin never owns an editable Rope or Undo stack.
    static STATE: RefCell<Converter> = RefCell::new(Converter::default());
}

#[derive(Default)]
enum Stage {
    #[default]
    Idle,
    Discovery,
    Read,
    Selection,
    Batch,
}

#[derive(Default)]
struct Converter {
    task: Option<api::guest::EditorTask>,
    request: Option<api::ResourceHandle>,
    /// A diagnostic picker publishes only its own eventual grant result, separately from business work.
    probe_selection: Option<api::ResourceHandle>,
    root: Option<api::ResourceHandle>,
    watch: Option<api::ResourceHandle>,
    selection: Option<api::ResourceHandle>,
    batch: Option<api::ResourceHandle>,
    stage: Stage,
    pending: VecDeque<FileTarget>,
    changes: Vec<Change>,
    status: String,
    events: Vec<api::Notification>,
}

impl Converter {
    fn start(&mut self, operation: api::EditorOperation, stage: Stage) -> Result<(), api::Failure> {
        self.cancel();
        let request = api::guest::editor(operation, 120_000)?;
        self.request = Some(request.clone());
        self.task = Some(api::guest::EditorTask::from_accepted(request));
        self.stage = stage;
        Ok(())
    }

    /// Cancellation affects subsequent work or waiting only; a retained receipt describes prior effects.
    fn cancel(&mut self) {
        if let Some(request) = self.request.take() {
            let _ = api::guest::cancel_request(&request, api::CancelMode::StopWaiting);
        }
        self.task = None;
    }

    fn discover(&mut self, cursor: Option<workspace::Cursor>) -> Result<(), api::Failure> {
        self.start(
            api::EditorOperation::Workspace {
                operation: workspace::Operation::Discover {
                    root: self.root.as_ref().unwrap().clone(),
                    query: api::FileQuery {
                        include: vec!["community-*.txt".into()],
                        exclude: Vec::new(),
                        max_results: 256,
                    },
                    page_size: 2,
                    cursor,
                },
            },
            Stage::Discovery,
        )
    }

    fn read_next(&mut self) -> Result<(), api::Failure> {
        if let Some(target) = self.pending.pop_front() {
            self.start(
                api::EditorOperation::Workspace {
                    operation: workspace::Operation::Read { target },
                },
                Stage::Read,
            )
        } else if self.changes.is_empty() {
            self.status = "No conversion needed / 无需转换".into();
            Ok(())
        } else {
            self.submit("Convert workspace text / 转换工作区文本")
        }
    }

    fn submit(&mut self, title: &str) -> Result<(), api::Failure> {
        let plan = Plan {
            origin: None,
            title: title.into(),
            changes: std::mem::take(&mut self.changes),
        };
        let (request, batch) = workspace::submit(plan, 120_000)?;
        self.request = Some(request.clone());
        self.task = Some(api::guest::EditorTask::from_accepted(request));
        self.batch = Some(batch);
        self.stage = Stage::Batch;
        self.status = "Review the native preview / 请确认原生预览".into();
        Ok(())
    }

    fn advance(&mut self, value: api::EditorValue) -> Result<(), api::Failure> {
        match (std::mem::take(&mut self.stage), value) {
            (Stage::Discovery, api::EditorValue::Workspace(workspace::Value::Page(page))) => {
                if !page.skipped.is_empty() {
                    return Err(api::Failure::new(
                        api::ErrorCode::OperationFailed,
                        "Discovery was incomplete; no plan applied",
                    ));
                }
                self.pending.extend(
                    page.entries
                        .into_iter()
                        .filter(|entry| matches!(entry.stamp, workspace::FileStamp::File { .. }))
                        .map(|entry| entry.target),
                );
                if let Some(cursor) = page.next {
                    self.discover(Some(cursor))?;
                } else {
                    self.read_next()?;
                }
            }
            (
                Stage::Read,
                api::EditorValue::Workspace(workspace::Value::Read { metadata, bytes }),
            ) => {
                let source = String::from_utf8(bytes).map_err(|_| {
                    api::Failure::new(
                        api::ErrorCode::UnsupportedOperation,
                        "Conversion requires UTF-8",
                    )
                })?;
                let converted = source.replace("旧A", "转换A").replace("旧B", "转换B");
                if source != converted {
                    self.changes
                        .push(if let Some(document) = metadata.document {
                            Change::EditDocument {
                                document,
                                edits: vec![api::TextEdit {
                                    range: api::TextRange {
                                        start: 0,
                                        end: source.len(),
                                    },
                                    text: converted,
                                }],
                                selection: api::TextRange { start: 0, end: 0 },
                            }
                        } else {
                            Change::Write {
                                target: metadata.target,
                                expected: Some(metadata.stamp),
                                bytes: converted.into_bytes(),
                            }
                        });
                }
                self.read_next()?;
            }
            (
                Stage::Selection,
                api::EditorValue::Interaction(interaction::Value::Selected(selected)),
            ) => {
                let resource = selected.into_iter().next().ok_or_else(|| {
                    api::Failure::new(api::ErrorCode::NotFound, "No Save target selected")
                })?;
                self.selection = Some(resource.handle.clone());
                self.changes = vec![Change::Write {
                    target: FileTarget {
                        root: resource.handle,
                        path: String::new(),
                    },
                    expected: None,
                    bytes: "Conversion report 中文🙂\r\n".as_bytes().to_vec(),
                }];
                self.submit("Export conversion report / 导出转换报告")?;
            }
            (Stage::Batch, api::EditorValue::Workspace(workspace::Value::Batch(receipt))) => {
                self.status = serde_json::to_string(&receipt).unwrap()
            }
            (Stage::Idle, value) => {
                self.status = serde_json::to_string(&Ok::<_, api::Failure>(value)).unwrap()
            }
            _ => {
                return Err(api::Failure::new(
                    api::ErrorCode::InvalidState,
                    "Unexpected conversion result",
                ));
            }
        }
        Ok(())
    }

    /// Each new business intent releases earlier current-session resources instead of persisting handles.
    fn clear_intent(&mut self) {
        self.cancel();
        if let Some(selection) = self.probe_selection.take() {
            let _ = api::guest::close_resource(selection);
        }
        for handle in [
            self.batch.take(),
            self.watch.take(),
            self.root.take(),
            self.selection.take(),
        ]
        .into_iter()
        .flatten()
        {
            let _ = api::guest::close_resource(handle);
        }
        self.pending.clear();
        self.changes.clear();
    }

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
                        .and_then(|operation: api::Operation| {
                            let selection = matches!(
                                &operation,
                                api::Operation::Editor {
                                    operation: api::EditorOperation::Interaction { operation },
                                    ..
                                } if operation.selection().is_some()
                            );
                            let result = api::guest::request(operation);
                            if selection {
                                if let Ok(api::Value::Accepted(request)) = &result {
                                    self.probe_selection = Some(request.clone());
                                }
                            }
                            result
                        });
                    self.status = serde_json::to_string(&result).unwrap();
                }
                "events" => self.status = serde_json::to_string(&self.events).unwrap(),
                "convert" => {
                    self.clear_intent();
                    self.root = Some(api::guest::open_workspace()?);
                    self.watch = Some(workspace::subscribe(self.root.as_ref().unwrap().clone())?);
                    self.discover(None)?;
                }
                "export" => {
                    self.clear_intent();
                    self.start(
                        api::EditorOperation::Interaction {
                            operation: interaction::Operation::Select {
                                title: "Export conversion report / 导出转换报告".into(),
                                mode: interaction::SelectionMode::Save,
                                multiple: false,
                                suggested_name: Some("conversion-report.txt".into()),
                            },
                        },
                        Stage::Selection,
                    )?;
                }
                "undo" | "status" => {
                    let batch = self.batch.clone().ok_or_else(|| {
                        api::Failure::new(api::ErrorCode::NotFound, "No retained conversion batch")
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
                    self.status =
                        "Stopped waiting; inspect actual effects / 已停止等待，请查看实际结果"
                            .into();
                }
                _ => {}
            },
            api::Input::Event { event, .. } => {
                // Native picker grants arrive after acceptance, through the normal request event.
                if let api::Notification::Request { handle, update } = &event {
                    if self.probe_selection.as_ref() == Some(handle) && update.is_terminal() {
                        self.probe_selection = None;
                        self.status = serde_json::to_string(update).unwrap();
                        return Ok(());
                    }
                }
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
                    // The panel is an observation summary, never an unbounded file-content transcript.
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

struct TextConverter;
impl Guest for TextConverter {
    /// Command errors are user-visible and do not destroy an otherwise recoverable resource.
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
                        panel: "converter".into(),
                        document: ui::Document::new(ui::Node::text(
                            "converter-result",
                            state.status.clone(),
                        )),
                    }],
                    ..Default::default()
                })
            })
        })
    }
}
export!(TextConverter);
