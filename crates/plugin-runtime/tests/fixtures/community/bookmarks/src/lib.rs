//! User bookmarks use native selection and host-tracked point anchors, never a text copy or Undo stack.
use plugin_protocol::{
    api,
    bindings::{Guest, export},
    decorations as marks, ui,
};
use std::cell::RefCell;
thread_local! { static STATE: RefCell<Bookmarks> = RefCell::new(Bookmarks::default()); }
#[derive(Default)]
enum Stage {
    #[default]
    Idle,
    List,
    Recover,
    Selection,
    Existing,
    Publish,
    Read,
}
#[derive(Default)]
struct Bookmarks {
    collection: Option<api::ResourceHandle>,
    document: Option<api::DocumentVersion>,
    task: Option<api::guest::EditorTask>,
    /// Observe close across terminal stream failures without allocating an unbounded succession of roots.
    subscription: Option<api::ResourceHandle>,
    stage: Stage,
    point: usize,
    next_id: u64,
    status: String,
}
impl Bookmarks {
    fn start(&mut self, operation: api::EditorOperation, stage: Stage) -> Result<(), api::Failure> {
        if let Some(task) = self.task.take() {
            let _ = task.cancel(api::CancelMode::StopWaiting);
        }
        self.task = Some(api::guest::EditorTask::start(operation, 30_000)?);
        self.stage = stage;
        Ok(())
    }
    /// Publication carries one exact native document revision and only current point metadata.
    fn publish(&mut self, positions: Vec<marks::Position>) -> Result<(), api::Failure> {
        let document = self.document.clone().ok_or_else(|| {
            api::Failure::new(api::ErrorCode::InvalidState, "Choose a document first")
        })?;
        let mut markers = positions
            .into_iter()
            .filter_map(|position| {
                position.range.map(|range| marks::Marker {
                    id: position.id,
                    anchor: marks::Anchor::Point {
                        offset: range.start,
                        affinity: marks::Affinity::Right,
                    },
                    label: ui::LocalizedText::new("User bookmark", "用户书签"),
                    icon: Some(ui::ToolIcon::new("icons/marker.svg", "icons/marker.svg")),
                    style: marks::Style::Accent,
                })
            })
            .collect::<Vec<_>>();
        if let Some(index) = markers.iter().position(|marker| matches!(marker.anchor, marks::Anchor::Point { offset, .. } if offset == self.point)) { markers.remove(index); }
        else {
            self.next_id += 1;
            markers.push(marks::Marker { id: format!("bookmark-{}", self.next_id), anchor: marks::Anchor::Point { offset: self.point, affinity: marks::Affinity::Right },
                label: ui::LocalizedText::new("User bookmark", "用户书签"), icon: Some(ui::ToolIcon::new("icons/marker.svg", "icons/marker.svg")), style: marks::Style::Accent });
        }
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
        )
    }
    fn advance(&mut self, value: api::EditorValue) -> Result<(), api::Failure> {
        match (std::mem::take(&mut self.stage), value) {
            (Stage::Recover, api::EditorValue::Documents { documents, .. }) => {
                if !documents.iter().any(|info| {
                    self.document
                        .as_ref()
                        .is_some_and(|old| old.id == info.document.id)
                }) {
                    if let Some(handle) = self.collection.take() {
                        let _ = api::guest::close_resource(handle);
                    }
                    self.document = None;
                }
            }
            (
                Stage::List,
                api::EditorValue::Documents {
                    active: Some(document),
                    ..
                },
            ) => {
                if self
                    .document
                    .as_ref()
                    .is_some_and(|old| old.id != document.id)
                {
                    if let Some(handle) = self.collection.take() {
                        let _ = api::guest::close_resource(handle);
                    }
                }
                self.document = Some(document.clone());
                self.start(
                    api::EditorOperation::ReadDocumentSelection { document },
                    Stage::Selection,
                )?;
            }
            (
                Stage::Selection,
                api::EditorValue::DocumentSelection {
                    document, range, ..
                },
            ) => {
                self.document = Some(document);
                self.point = range.start;
                if let Some(handle) = &self.collection {
                    self.start(
                        api::EditorOperation::Decorations {
                            operation: marks::Operation::Read {
                                handle: handle.clone(),
                            },
                        },
                        Stage::Existing,
                    )?;
                } else {
                    self.publish(Vec::new())?;
                }
            }
            (
                Stage::Existing,
                api::EditorValue::Decorations {
                    document,
                    positions,
                    ..
                },
            ) => {
                self.document = Some(document);
                self.publish(positions)?;
            }
            (
                Stage::Publish,
                api::EditorValue::Decorations {
                    handle,
                    document,
                    positions,
                },
            ) => {
                self.collection = Some(handle);
                self.document = Some(document);
                self.status = format!("{} bookmarks / 书签", positions.len());
            }
            (Stage::Read, value @ api::EditorValue::Decorations { .. }) => {
                self.status = serde_json::to_string(&value).unwrap()
            }
            _ => {
                return Err(api::Failure::new(
                    api::ErrorCode::NotFound,
                    "Open a text document and choose its bookmark position",
                ));
            }
        }
        Ok(())
    }
    fn input(&mut self, input: api::Input) -> Result<(), api::Failure> {
        match input {
            api::Input::Activate => {
                self.subscription = Some(api::guest::subscribe_document_events()?);
                self.status = "Bookmark current caret with Bookmark current position / 使用“标记当前位置”标记当前位置".into();
            }
            api::Input::Event {
                event: api::Notification::Command { id, arguments, .. },
                ..
            } => match id.as_str() {
                "bookmark-line" => self.start(api::EditorOperation::ListDocuments, Stage::List)?,
                "read-bookmarks" => {
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
                        self.start(api::EditorOperation::ListDocuments, Stage::Recover)?;
                    }
                } else if let api::Notification::DocumentEvent {
                    event:
                        api::DocumentEvent {
                            kind: api::DocumentEventKind::Closed(document),
                            ..
                        },
                    ..
                } = &event
                {
                    if self
                        .document
                        .as_ref()
                        .is_some_and(|old| old.id == document.id)
                    {
                        if let Some(task) = self.task.take() {
                            let _ = task.cancel(api::CancelMode::StopWaiting);
                        }
                        if let Some(handle) = self.collection.take() {
                            let _ = api::guest::close_resource(handle);
                        }
                        self.document = None;
                        self.status = "Bookmark document closed / 书签文档已关闭".into();
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
struct BookmarkGuest;
impl Guest for BookmarkGuest {
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
                            "bookmark-status",
                            state.status.clone(),
                        )),
                    }],
                    ..Default::default()
                })
            })
        })
    }
}
export!(BookmarkGuest);
