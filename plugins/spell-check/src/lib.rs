//! A small independent spelling policy publishes native diagnostics and ordinary single-file plans.
//! It owns immutable observations only; native EditorState owns current text and all Undo history.
use plugin_protocol::{
    api,
    bindings::{Guest, export},
    diagnostics, ui, workspace,
};
use std::{cell::RefCell, collections::VecDeque};

thread_local! { static STATE: RefCell<Checker> = RefCell::new(Checker::default()); }

/// A single outstanding editor task and coalesced document queue keep publication order bounded.
#[derive(Default)]
struct Checker {
    collection: Option<api::ResourceHandle>,
    task: Option<api::guest::EditorTask>,
    pending: VecDeque<api::DocumentVersion>,
    reading: Option<api::DocumentVersion>,
    clear: bool,
    status: String,
}
impl Checker {
    /// Lazy allocation happens only after activation, under the installed diagnostics grant.
    fn collection(&mut self) -> Result<api::ResourceHandle, api::Failure> {
        if self.collection.is_none() {
            self.collection = Some(diagnostics::create("Spelling / 拼写".into())?);
        }
        Ok(self.collection.as_ref().unwrap().clone())
    }
    /// The host's task owns correlation and deadlines; this guest does not spin or block for text.
    fn start(&mut self, operation: api::EditorOperation) -> Result<(), api::Failure> {
        self.task = Some(api::guest::EditorTask::start(operation, 30_000)?);
        Ok(())
    }
    /// New revisions replace only their document's pending observation, never another checker's set.
    fn enqueue(&mut self, document: api::DocumentVersion) -> Result<(), api::Failure> {
        if document.path.starts_with("virtual://") {
            return Ok(());
        }
        self.pending.retain(|old| old.id != document.id);
        if self
            .reading
            .as_ref()
            .is_some_and(|old| old.id == document.id)
        {
            if let Some(task) = self.task.take() {
                let _ = task.cancel(api::CancelMode::StopWaiting);
            }
            self.reading = None;
        }
        if self.pending.len() >= 128 {
            return Err(api::Failure::new(
                api::ErrorCode::LimitExceeded,
                "Spelling queue exceeds 128 documents",
            ));
        }
        self.pending.push_back(document);
        self.next()
    }
    /// Read the next exact native version rather than substituting saved filesystem contents.
    fn next(&mut self) -> Result<(), api::Failure> {
        if self.task.is_none()
            && let Some(document) = self.pending.pop_front()
        {
            self.reading = Some(document.clone());
            self.start(api::EditorOperation::ReadDocument {
                document,
                range: None,
            })?;
        }
        Ok(())
    }
    /// The intentionally small dictionary demonstrates plugin-owned policy, not a host language branch.
    fn check(&self, snapshot: &api::DocumentSnapshot) -> Vec<diagnostics::Diagnostic> {
        snapshot
            .text
            .match_indices("teh")
            .filter(|(start, _)| {
                snapshot.text[..*start]
                    .chars()
                    .next_back()
                    .is_none_or(|ch| !ch.is_alphanumeric())
                    && snapshot.text[*start + 3..]
                        .chars()
                        .next()
                        .is_none_or(|ch| !ch.is_alphanumeric())
            })
            .take(256)
            .map(|(start, _)| diagnostics::Diagnostic {
                id: format!("spelling-{start}"),
                severity: diagnostics::Severity::Warning,
                range: api::TextRange {
                    start,
                    end: start + 3,
                },
                message: "Possible spelling: teh → the / 拼写建议".into(),
                code: Some("spelling".into()),
                related: vec![diagnostics::Related {
                    message: "Spelling source / 拼写来源".into(),
                    resource: snapshot.info.resource.clone(),
                    position: api::TextPosition {
                        line: 0,
                        character: 0,
                    },
                }],
                fixes: vec![diagnostics::Fix {
                    title: "Replace with the / 替换为 the".into(),
                    plan: workspace::Plan {
                        title: "Correct spelling / 修正拼写".into(),
                        origin: Some(snapshot.info.document.clone()),
                        changes: vec![workspace::Change::EditDocument {
                            document: snapshot.info.document.clone(),
                            edits: vec![api::TextEdit {
                                range: api::TextRange {
                                    start,
                                    end: start + 3,
                                },
                                text: "the".into(),
                            }],
                            selection: api::TextRange {
                                start: start + 3,
                                end: start + 3,
                            },
                        }],
                    },
                }],
            })
            .collect()
    }
    /// Enumerations and reads advance through public EditorRequest notifications.
    fn complete(&mut self, value: api::EditorValue) -> Result<(), api::Failure> {
        match value {
            api::EditorValue::Documents { documents, active } => {
                if self.clear {
                    self.clear = false;
                    if let Some(document) = active {
                        let collection = self.collection()?;
                        self.task = Some(diagnostics::publish(diagnostics::Publication::Clear {
                            collection,
                            document,
                        })?);
                    }
                } else {
                    self.pending = documents
                        .into_iter()
                        .filter(|info| !info.document.path.starts_with("virtual://"))
                        .map(|info| info.document)
                        .collect();
                    self.next()?;
                }
            }
            api::EditorValue::DocumentSnapshot(snapshot) => {
                let diagnostics = self.check(&snapshot);
                self.status = format!("Spelling problems / 拼写问题: {}", diagnostics.len());
                let collection = self.collection()?;
                self.task = Some(diagnostics::publish(diagnostics::Publication::Publish {
                    collection,
                    document: snapshot.info.document,
                    diagnostics,
                })?);
            }
            api::EditorValue::Unit => {
                self.reading = None;
                self.next()?;
            }
            _ => {
                return Err(api::Failure::new(
                    api::ErrorCode::InvalidState,
                    "Unexpected checker completion",
                ));
            }
        }
        Ok(())
    }
    /// Domain commands and diagnostic inspection both use the ordinary installed guest transport.
    fn input(&mut self, input: api::Input) -> Result<(), api::Failure> {
        match input {
            api::Input::Activate => {
                self.collection()?;
                api::guest::subscribe_documents()?;
            }
            api::Input::Event {
                event: api::Notification::Command { id, arguments, .. },
                ..
            } => match id.as_str() {
                "probe" => {
                    let result = arguments
                        .ok_or_else(|| {
                            api::Failure::new(
                                api::ErrorCode::InvalidRequest,
                                "Probe requires an operation",
                            )
                        })
                        .and_then(|args| {
                            serde_json::from_value(args).map_err(|error| {
                                api::Failure::new(api::ErrorCode::InvalidRequest, error.to_string())
                            })
                        })
                        .and_then(api::guest::request);
                    self.status = serde_json::to_string(&result).unwrap();
                }
                "check" | "clear" => {
                    if let Some(task) = self.task.take() {
                        let _ = task.cancel(api::CancelMode::StopWaiting);
                    }
                    self.pending.clear();
                    self.reading = None;
                    self.clear = id == "clear";
                    self.start(api::EditorOperation::ListDocuments)?;
                }
                _ => {}
            },
            api::Input::Event { event, .. } => {
                if let api::Notification::Document { change, .. } = &event {
                    if change.closed {
                        self.pending.retain(|old| old.id != change.document.id);
                    } else {
                        self.enqueue(change.document.clone())?;
                    }
                }
                match self.task.as_mut().and_then(|task| task.update(&event)) {
                    Some(api::RequestUpdate::Completed { result }) => {
                        self.task = None;
                        if let Err(error) = result.and_then(|value| self.complete(value)) {
                            self.status = error.to_string();
                            self.reading = None;
                            self.next()?;
                        }
                    }
                    Some(api::RequestUpdate::Cancelled { reason, .. }) => {
                        self.task = None;
                        self.reading = None;
                        self.status = format!("{reason:?}");
                        self.next()?;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(())
    }
}
/// This checker restores policy from public document observations, never serialized live handles.
struct Spelling;
impl Guest for Spelling {
    /// Failures stay inspectable in native plugin UI; they never claim an edit succeeded.
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| {
            // No durable policy state exists: handles, pending work and text belong to this incarnation.
            if matches!(input, api::Input::Snapshot) {
                return Ok(api::Output {
                    snapshot: Some(Default::default()),
                    ..Default::default()
                });
            }
            STATE.with(|state| {
                let mut state = state.borrow_mut();
                if let Err(error) = state.input(input) {
                    state.status = error.to_string();
                }
                Ok(api::Output {
                    views: vec![api::View {
                        panel: "checker".into(),
                        document: ui::Document::new(ui::Node::text(
                            "checker-status",
                            state.status.clone(),
                        )),
                    }],
                    ..Default::default()
                })
            })
        })
    }
}
export!(Spelling);
