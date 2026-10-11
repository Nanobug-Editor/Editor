//! Todo navigation is derived from immutable snapshots of real open documents.
//! Document events rescan current text; no filesystem index, secondary Rope or edit history is retained.
use plugin_protocol::{
    api,
    bindings::{Guest, export},
    tree, ui,
};
use std::{cell::RefCell, collections::BTreeMap};
thread_local! { static STATE: RefCell<Todos> = RefCell::new(Todos::default()); }
#[derive(Default)]
enum Stage {
    #[default]
    Idle,
    List,
    Read(api::DocumentVersion),
    Locate,
}
#[derive(Default)]
struct Todos {
    tree: Option<api::ResourceHandle>,
    task: Option<api::guest::EditorTask>,
    /// A terminal stream failure must be followed by a new subscription and full enumeration.
    subscription: Option<api::ResourceHandle>,
    stage: Stage,
    remaining: Vec<api::DocumentVersion>,
    roots: Vec<tree::Node>,
    children: BTreeMap<String, Vec<tree::Node>>,
    locations: BTreeMap<String, (api::DocumentVersion, api::TextPosition)>,
    pending: Vec<tree::Request>,
    ready: bool,
    status: String,
    english: bool,
}
impl Todos {
    /// Replace user intent before scheduling native work; a cancelled older task cannot republish its source.
    fn start(&mut self, operation: api::EditorOperation, stage: Stage) -> Result<(), api::Failure> {
        if let Some(task) = self.task.take() {
            let _ = task.cancel(api::CancelMode::StopWaiting);
        }
        self.task = Some(api::guest::EditorTask::start(operation, 30_000)?);
        self.stage = stage;
        Ok(())
    }
    fn refresh(&mut self) -> Result<(), api::Failure> {
        if let Some(tree) = &self.tree {
            tree::refresh(tree.clone())?;
        }
        self.ready = false;
        self.roots.clear();
        self.children.clear();
        self.locations.clear();
        self.pending.clear();
        self.start(api::EditorOperation::ListDocuments, Stage::List)
    }
    fn next(&mut self) -> Result<(), api::Failure> {
        if let Some(document) = self.remaining.pop() {
            self.start(
                api::EditorOperation::ReadDocument {
                    document: document.clone(),
                    range: None,
                },
                Stage::Read(document),
            )
        } else {
            self.ready = true;
            self.status = format!("{} Todos / 待办项", self.locations.len());
            for request in std::mem::take(&mut self.pending) {
                self.page(request)?;
            }
            Ok(())
        }
    }
    /// Location versions are captured with their unsaved snapshot; navigation rejects later edits natively.
    fn advance(&mut self, value: api::EditorValue) -> Result<(), api::Failure> {
        match (std::mem::take(&mut self.stage), value) {
            (Stage::List, api::EditorValue::Documents { documents, .. }) => {
                self.remaining = documents
                    .into_iter()
                    .filter(|info| matches!(info.resource, api::ResourceIdentity::Local { .. }))
                    .map(|info| info.document)
                    .rev()
                    .collect();
                self.next()?;
            }
            (Stage::Read(document), api::EditorValue::DocumentSnapshot(snapshot)) => {
                let root = format!("doc-{}", document.id);
                let mut children = Vec::new();
                for (line, text) in snapshot.text.lines().enumerate() {
                    if !text.contains("TODO") && !text.contains("FIXME") {
                        continue;
                    }
                    if self.locations.len() >= 1024 {
                        break;
                    }
                    let id = format!("todo-{}-{line}", document.id);
                    let label = text.trim().chars().take(240).collect::<String>();
                    let mut node = tree::Node::new(&id, format!("{}: {label}", line + 1), false);
                    let command = tree::Command {
                        command: "locate-todo".into(),
                        label: ui::LocalizedText::new("Locate Todo", "定位待办"),
                        arguments: serde_json::json!(id),
                    };
                    node.activation = Some(command.clone());
                    node.menu.push(command);
                    self.locations.insert(
                        id,
                        (
                            document.clone(),
                            api::TextPosition {
                                line: line as u32,
                                character: 0,
                            },
                        ),
                    );
                    children.push(node);
                }
                self.roots
                    .push(tree::Node::new(&root, document.path.clone(), true));
                self.children.insert(root, children);
                self.next()?;
            }
            (Stage::Locate, api::EditorValue::DocumentLocated { .. }) => {
                self.status = "Todo located / 已定位待办".into()
            }
            _ => {
                return Err(api::Failure::new(
                    api::ErrorCode::InvalidState,
                    "Unexpected Todo task result",
                ));
            }
        }
        Ok(())
    }
    /// The provider uses thirty-two nodes per page; the native renderer retains only accepted pages.
    fn page(&mut self, request: tree::Request) -> Result<(), api::Failure> {
        if !self.ready {
            if self.pending.len() >= 8 {
                return Err(api::Failure::new(
                    api::ErrorCode::LimitExceeded,
                    "Todo pending-page budget exhausted",
                ));
            }
            self.pending.push(request);
            return Ok(());
        }
        let nodes = match &request.parent {
            None => &self.roots,
            Some(id) => self.children.get(id).ok_or_else(|| {
                api::Failure::new(api::ErrorCode::NotFound, "Todo source was closed")
            })?,
        };
        let start = request
            .cursor
            .as_deref()
            .unwrap_or("0")
            .parse::<usize>()
            .map_err(|_| {
                api::Failure::new(api::ErrorCode::InvalidRequest, "Unknown Todo cursor")
            })?;
        if start > nodes.len() {
            return Err(api::Failure::new(
                api::ErrorCode::InvalidRequest,
                "Todo cursor exceeds branch",
            ));
        }
        let end = (start + 32).min(nodes.len());
        tree::reply(
            request,
            Ok(tree::Page {
                nodes: nodes[start..end].to_vec(),
                next: (end < nodes.len()).then(|| end.to_string()),
            }),
        )
    }
    fn input(
        &mut self,
        input: api::Input,
    ) -> Result<Option<Result<serde_json::Value, api::Failure>>, api::Failure> {
        match input {
            api::Input::Prepare { environment, .. } => {
                self.english = environment.locale.starts_with("en")
            }
            api::Input::Activate => {
                self.tree = Some(tree::create()?);
                self.subscription = Some(api::guest::subscribe_document_events()?);
                self.refresh()?;
            }
            api::Input::Event {
                event:
                    api::Notification::Ui(ui::UiEvent {
                        action: ui::Action::Tree(tree::Event::Request(request)),
                        ..
                    }),
                ..
            } => self.page(request)?,
            api::Input::Event {
                event: api::Notification::CommandInvocation(call),
                ..
            } if call.id == "locate-todo" => {
                let id = call.arguments.as_str().unwrap_or("");
                let result = self
                    .locations
                    .get(id)
                    .cloned()
                    .ok_or_else(|| {
                        api::Failure::new(api::ErrorCode::NotFound, "Todo node no longer exists")
                    })
                    .and_then(|(document, position)| {
                        self.start(
                            api::EditorOperation::LocateDocument { document, position },
                            Stage::Locate,
                        )
                    });
                return Ok(Some(result.map(|_| serde_json::Value::Null)));
            }
            api::Input::Event {
                event: api::Notification::Command { id, arguments, .. },
                ..
            } => match id.as_str() {
                "refresh-todos" => self.refresh()?,
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
                        self.refresh()?;
                    }
                } else if matches!(
                    event,
                    api::Notification::DocumentEvent {
                        event: api::DocumentEvent {
                            kind: api::DocumentEventKind::Opened(_)
                                | api::DocumentEventKind::ContentChanged(_)
                                | api::DocumentEventKind::Closed(_),
                            ..
                        },
                        ..
                    }
                ) {
                    self.refresh()?;
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
        Ok(None)
    }
    fn view(&self) -> ui::Document {
        let mut nodes = vec![ui::Node::text("todo-status", self.status.clone())];
        if let Some(handle) = &self.tree {
            nodes.push(
                ui::Node {
                    id: "todo-tree".into(),
                    kind: ui::Kind::Tree {
                        handle: handle.clone(),
                    },
                    ..ui::Node::text("unused", "")
                }
                .grow(),
            );
        }
        // A growing root supplies a finite viewport for Base's virtualized child tree.
        ui::Document::new(ui::Node::column("todos", nodes).grow()).revision(1)
    }
}
struct Todo;
impl Guest for Todo {
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| {
            STATE.with(|state| {
                let mut state = state.borrow_mut();
                let reply = match state.input(input) {
                    Ok(reply) => reply,
                    Err(error) => {
                        for request in std::mem::take(&mut state.pending) {
                            let _ = tree::reply(request, Err(error.clone()));
                        }
                        state.status = error.to_string();
                        None
                    }
                };
                Ok(api::Output {
                    snapshot: Some(Default::default()),
                    service_reply: reply,
                    views: vec![api::View {
                        panel: "presentation".into(),
                        document: state.view(),
                    }],
                    ..Default::default()
                })
            })
        })
    }
}
export!(Todo);
