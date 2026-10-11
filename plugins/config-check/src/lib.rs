//! This independent checker finds missing include files and proposes a normal cross-file creation.
//! Source snapshots are immutable; the host owns path authority, preview, conflicts and group Undo.
use plugin_protocol::{
    api,
    bindings::{Guest, export},
    diagnostics, ui, workspace,
};
use std::{cell::RefCell, collections::VecDeque};
thread_local! { static STATE: RefCell<Checker> = RefCell::new(Checker::default()); }

/// The only retained source belongs to one pending stat request, not a mutable document model.
#[derive(Default)]
struct Checker {
    collection: Option<api::ResourceHandle>,
    root: Option<api::ResourceHandle>,
    task: Option<api::guest::EditorTask>,
    pending: VecDeque<api::DocumentVersion>,
    source: Option<api::DocumentSnapshot>,
    reading: Option<api::DocumentVersion>,
    include: Option<(usize, String)>,
    clear: bool,
    status: String,
}
impl Checker {
    /// Both roots are allocated under installation grants and revoked with this ordinary instance.
    fn activate(&mut self) -> Result<(), api::Failure> {
        self.collection = Some(diagnostics::create("Configuration / 配置".into())?);
        let api::Value::Resource(root) = api::guest::request(api::Operation::OpenWorkspace)? else {
            return Err(api::Failure::new(
                api::ErrorCode::InvalidState,
                "Workspace root expected",
            ));
        };
        workspace::subscribe(root.clone())?;
        self.root = Some(root);
        api::guest::subscribe_documents()?;
        Ok(())
    }
    /// Correlated public completions drive work; no blocking guest loop waits for native IO.
    fn start(&mut self, operation: api::EditorOperation) -> Result<(), api::Failure> {
        self.task = Some(api::guest::EditorTask::start(operation, 30_000)?);
        Ok(())
    }
    /// Coalesce repeated source revisions; closing a document cannot make its old snapshot current.
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
            self.source = None;
            self.include = None;
            self.reading = None;
        }
        if self.pending.len() >= 128 {
            return Err(api::Failure::new(
                api::ErrorCode::LimitExceeded,
                "Configuration queue exceeds 128 documents",
            ));
        }
        self.pending.push_back(document);
        self.next()
    }
    /// Read unsaved text at the provided version before interpreting include policy.
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
    /// Publish zero or one policy result, keeping a missing file's exact observed expectation.
    fn publish(&mut self, missing: bool) -> Result<(), api::Failure> {
        let snapshot = self.source.take().unwrap();
        let diagnostics = if missing {
            let (start, path) = self.include.take().unwrap();
            vec![diagnostics::Diagnostic {
                id: format!("missing-include-{start}"),
                severity: diagnostics::Severity::Error,
                range: api::TextRange {
                    start,
                    end: start + path.len(),
                },
                message: format!("Missing configuration / 缺少配置: {path}"),
                code: Some("missing-include".into()),
                related: vec![diagnostics::Related {
                    message: "Include source / 引用来源".into(),
                    resource: snapshot.info.resource.clone(),
                    position: api::TextPosition {
                        line: 0,
                        character: 0,
                    },
                }],
                fixes: vec![diagnostics::Fix {
                    title: "Create default configuration / 创建默认配置".into(),
                    plan: workspace::Plan {
                        title: "Create included configuration / 创建所引用的配置".into(),
                        origin: Some(snapshot.info.document.clone()),
                        changes: vec![workspace::Change::Write {
                            target: workspace::FileTarget {
                                root: self.root.as_ref().unwrap().clone(),
                                path,
                            },
                            expected: Some(workspace::FileStamp::Missing),
                            bytes: b"port=8080\n".to_vec(),
                        }],
                    },
                }],
            }]
        } else {
            self.include = None;
            Vec::new()
        };
        self.status = format!("Configuration problems / 配置问题: {}", diagnostics.len());
        self.task = Some(diagnostics::publish(diagnostics::Publication::Publish {
            collection: self.collection.as_ref().unwrap().clone(),
            document: snapshot.info.document,
            diagnostics,
        })?);
        Ok(())
    }
    /// Include policy and creation data belong to this guest; the host never recognizes their keys.
    fn complete(&mut self, value: api::EditorValue) -> Result<(), api::Failure> {
        match value {
            api::EditorValue::Documents { documents, active } => {
                if self.clear {
                    self.clear = false;
                    if let Some(document) = active {
                        self.task = Some(diagnostics::publish(diagnostics::Publication::Clear {
                            collection: self.collection.as_ref().unwrap().clone(),
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
                let include = snapshot
                    .text
                    .match_indices("include=")
                    .find_map(|(start, _)| {
                        if start != 0 && !snapshot.text[..start].ends_with('\n') {
                            return None;
                        }
                        let start = start + 8;
                        let raw = snapshot.text[start..].lines().next()?;
                        let path = raw.trim().to_owned();
                        // The path policy removes whitespace; its diagnostic must point at those
                        // same complete UTF-8 bytes, not at the untrimmed key's following offset.
                        let start = start + raw.len() - raw.trim_start().len();
                        let resource = api::ResourceIdentity::Local { path: path.clone() };
                        resource.validate().ok()?;
                        Some((start, path))
                    });
                self.source = Some(snapshot);
                self.include = include;
                if let Some((_, path)) = &self.include {
                    self.start(api::EditorOperation::Workspace {
                        operation: workspace::Operation::Stat {
                            target: workspace::FileTarget {
                                root: self.root.as_ref().unwrap().clone(),
                                path: path.clone(),
                            },
                        },
                    })?;
                } else {
                    self.publish(false)?;
                }
            }
            api::EditorValue::Workspace(workspace::Value::Metadata(metadata)) => {
                self.publish(matches!(metadata.stamp, workspace::FileStamp::Missing))?;
            }
            api::EditorValue::Unit => {
                self.reading = None;
                self.next()?;
            }
            _ => {
                return Err(api::Failure::new(
                    api::ErrorCode::InvalidState,
                    "Unexpected configuration completion",
                ));
            }
        }
        Ok(())
    }
    /// Filesystem observations trigger the same public re-enumeration after an accepted fix or Undo.
    fn input(&mut self, input: api::Input) -> Result<(), api::Failure> {
        match input {
            api::Input::Activate => self.activate()?,
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
                    self.source = None;
                    self.include = None;
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
                if matches!(&event, api::Notification::WorkspaceFile { .. }) && self.task.is_none()
                {
                    self.start(api::EditorOperation::ListDocuments)?;
                }
                match self.task.as_mut().and_then(|task| task.update(&event)) {
                    Some(api::RequestUpdate::Completed { result }) => {
                        self.task = None;
                        if let Err(error) = result.and_then(|value| self.complete(value)) {
                            self.status = error.to_string();
                            self.source = None;
                            self.include = None;
                            self.reading = None;
                            self.next()?;
                        }
                    }
                    Some(api::RequestUpdate::Cancelled { reason, .. }) => {
                        self.task = None;
                        self.source = None;
                        self.include = None;
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
/// Configuration policy is reconstructed from public snapshots and filesystem observations.
struct Configuration;
impl Guest for Configuration {
    /// Ordinary native status remains inspectable when a revision/permission/file conflict fails.
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| {
            // Only logical state may cross replacement; native roots and requests are reacquired.
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
export!(Configuration);
