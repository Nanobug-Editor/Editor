//! Coverage is an independent range provider; formal document events rescan current unsaved snapshots.
//! Deleted ranges are rebuilt after Undo from source meaning, without retaining text or a marker history.
use plugin_protocol::{
    api,
    bindings::{Guest, export},
    decorations as marks, ui,
};
use std::cell::RefCell;
thread_local! { static STATE: RefCell<Coverage> = RefCell::new(Coverage::default()); }
#[derive(Default)]
enum Stage {
    #[default]
    Idle,
    List,
    Recover,
    Scan,
    Publish,
    Read,
}
#[derive(Default)]
struct Coverage {
    source: Option<api::DocumentVersion>,
    collection: Option<api::ResourceHandle>,
    task: Option<api::guest::EditorTask>,
    /// Retain the stream handle so overflow recovery does not leak its persistent root.
    subscription: Option<api::ResourceHandle>,
    stage: Stage,
    enabled: bool,
    status: String,
}
impl Coverage {
    fn start(&mut self, operation: api::EditorOperation, stage: Stage) -> Result<(), api::Failure> {
        if let Some(task) = self.task.take() {
            let _ = task.cancel(api::CancelMode::StopWaiting);
        }
        self.task = Some(api::guest::EditorTask::start(operation, 30_000)?);
        self.stage = stage;
        Ok(())
    }
    fn scan(&mut self, document: api::DocumentVersion) -> Result<(), api::Failure> {
        self.source = Some(document.clone());
        self.start(
            api::EditorOperation::ReadDocument {
                document,
                range: None,
            },
            Stage::Scan,
        )
    }
    /// Domain lines explicitly declare COVERED/UNCOVERED; plain lines receive no artificial marker.
    fn markers(snapshot: &api::DocumentSnapshot) -> Vec<marks::Marker> {
        let mut offset = 0;
        let mut markers = Vec::new();
        for (line, contents) in snapshot.text.split_inclusive('\n').enumerate() {
            let text = contents.trim_end_matches(['\r', '\n']);
            if text.contains("COVERED") && !text.is_empty() && markers.len() < 1024 {
                let uncovered = text.contains("UNCOVERED");
                markers.push(marks::Marker {
                    id: format!("coverage-line-{line}"),
                    anchor: marks::Anchor::Range {
                        range: api::TextRange {
                            start: offset,
                            end: offset + text.len(),
                        },
                    },
                    label: if uncovered {
                        ui::LocalizedText::new("Uncovered line", "未覆盖行")
                    } else {
                        ui::LocalizedText::new("Covered line", "已覆盖行")
                    },
                    icon: Some(ui::ToolIcon::new("icons/marker.svg", "icons/marker.svg")),
                    style: if uncovered {
                        marks::Style::Warning
                    } else {
                        marks::Style::Success
                    },
                });
            }
            offset += contents.len();
        }
        markers
    }
    fn advance(&mut self, value: api::EditorValue) -> Result<(), api::Failure> {
        match (std::mem::take(&mut self.stage), value) {
            (
                Stage::List,
                api::EditorValue::Documents {
                    active: Some(document),
                    ..
                },
            ) => {
                if self
                    .source
                    .as_ref()
                    .is_some_and(|old| old.id != document.id)
                {
                    if let Some(handle) = self.collection.take() {
                        let _ = api::guest::close_resource(handle);
                    }
                }
                self.scan(document)?;
            }
            (Stage::Recover, api::EditorValue::Documents { documents, .. }) => {
                // Recover this collection's document, never silently follow a different active editor.
                if let Some(info) = documents.into_iter().find(|info| {
                    self.source
                        .as_ref()
                        .is_some_and(|old| old.id == info.document.id)
                }) {
                    self.scan(info.document)?;
                } else {
                    if let Some(handle) = self.collection.take() {
                        let _ = api::guest::close_resource(handle);
                    }
                    self.source = None;
                }
            }
            (Stage::Scan, api::EditorValue::DocumentSnapshot(snapshot)) => {
                let document = snapshot.info.document.clone();
                let markers = Self::markers(&snapshot);
                let operation = self
                    .collection
                    .as_ref()
                    .map(|handle| marks::Operation::Replace {
                        handle: handle.clone(),
                        document: document.clone(),
                        markers: markers.clone(),
                    })
                    .unwrap_or(marks::Operation::Create { document, markers });
                self.start(
                    api::EditorOperation::Decorations { operation },
                    Stage::Publish,
                )?;
            }
            (
                Stage::Publish,
                api::EditorValue::Decorations {
                    handle,
                    document,
                    positions,
                },
            ) => {
                self.source = Some(document);
                self.collection = Some(handle);
                self.status = format!("{} coverage ranges / 覆盖率范围", positions.len());
            }
            (Stage::Read, value @ api::EditorValue::Decorations { .. }) => {
                self.status = serde_json::to_string(&value).unwrap()
            }
            _ => {
                return Err(api::Failure::new(
                    api::ErrorCode::NotFound,
                    "Open a text document before scanning coverage",
                ));
            }
        }
        Ok(())
    }
    fn input(&mut self, input: api::Input) -> Result<(), api::Failure> {
        match input {
            api::Input::Activate => {
                self.subscription = Some(api::guest::subscribe_document_events()?);
                self.status =
                    "Scan lines labelled COVERED or UNCOVERED / 扫描 COVERED 或 UNCOVERED 行"
                        .into();
            }
            api::Input::Event {
                event: api::Notification::Command { id, arguments, .. },
                ..
            } => match id.as_str() {
                "scan-coverage" => {
                    self.enabled = true;
                    self.start(api::EditorOperation::ListDocuments, Stage::List)?;
                }
                "read-coverage" => {
                    if let Some(handle) = &self.collection {
                        self.start(
                            api::EditorOperation::Decorations {
                                operation: marks::Operation::Read {
                                    handle: handle.clone(),
                                },
                            },
                            Stage::Read,
                        )?;
                    }
                }
                "probe" => {
                    let operation =
                        serde_json::from_value::<api::Operation>(arguments.unwrap_or_default())
                            .map_err(|error| {
                                api::Failure::new(api::ErrorCode::InvalidRequest, error.to_string())
                            })?;
                    self.status = serde_json::to_string(&api::guest::request(operation)).unwrap();
                }
                _ => {}
            },
            api::Input::Event { event, .. } => {
                if let api::Notification::SubscriptionFailed { subscription, .. } = &event {
                    if self.subscription.as_ref() == Some(subscription) {
                        if let Some(old) = self.subscription.take() {
                            let _ = api::guest::close_resource(old);
                        }
                        self.subscription = Some(api::guest::subscribe_document_events()?);
                        if self.enabled {
                            self.start(api::EditorOperation::ListDocuments, Stage::Recover)?;
                        }
                    }
                } else if let api::Notification::DocumentEvent { event, .. } = &event {
                    match &event.kind {
                        api::DocumentEventKind::ContentChanged(info)
                            if self.enabled
                                && self
                                    .source
                                    .as_ref()
                                    .is_some_and(|old| old.id == info.document.id) =>
                        {
                            self.scan(info.document.clone())?
                        }
                        api::DocumentEventKind::Closed(document)
                            if self
                                .source
                                .as_ref()
                                .is_some_and(|old| old.id == document.id) =>
                        {
                            if let Some(task) = self.task.take() {
                                let _ = task.cancel(api::CancelMode::StopWaiting);
                            }
                            if let Some(handle) = self.collection.take() {
                                let _ = api::guest::close_resource(handle);
                            }
                            self.source = None;
                            self.status = "Coverage document closed / 覆盖率文档已关闭".into();
                        }
                        _ => {}
                    }
                } else if let Some(update) = self.task.as_mut().and_then(|task| task.update(&event))
                {
                    match update {
                        api::RequestUpdate::Completed { result } => {
                            self.task = None;
                            self.advance(result?)?;
                        }
                        api::RequestUpdate::Cancelled { reason, .. } => {
                            self.task = None;
                            self.status = format!("{reason:?}");
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
struct CoverageGuest;
impl Guest for CoverageGuest {
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
                        panel: "presentation".into(),
                        document: ui::Document::new(ui::Node::text(
                            "coverage-status",
                            state.status.clone(),
                        )),
                    }],
                    ..Default::default()
                })
            })
        })
    }
}
export!(CoverageGuest);
