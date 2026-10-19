//! Separate unknown SDK consumers borrow one export through an independently owned native engine.
//! The fake stdio worker never executes adapter code; only authorization and lifecycle are observed.
use plugin_protocol::{
    Snapshot, api,
    bindings::{Guest, export},
    language, language_control, service,
};
use serde_json::{Value, json};
use std::cell::RefCell;

/// Each role keeps only its own public asynchronous handle and final inert receipt.
enum Work {
    Peer {
        reference: api::ResourceHandle,
        task: service::guest::Task,
    },
    Native {
        reference: api::ResourceHandle,
        reply: api::ResourceHandle,
        task: api::guest::EditorTask,
    },
    /// Related-source queries carry no executable export or private provider handle.
    Context {
        reply: api::ResourceHandle,
        task: api::guest::EditorTask,
    },
}

#[derive(Default)]
struct State {
    pending: Vec<Work>,
    receipt: Value,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
struct Fixture;

impl State {
    /// A real consumer instance starts the declared peer call; no forged principal or host handle.
    fn start_consumer(&mut self, method: &str, arguments: Value) -> Result<(), api::Failure> {
        self.receipt = json!({"state":"pending"});
        let reference = service::guest::open("novel.engine")?;
        let task = match service::guest::Task::start(&reference, method, arguments, 120_000) {
            Ok(task) => task,
            Err(error) => {
                let _ = api::guest::close_resource(reference);
                return Err(error);
            }
        };
        self.pending.push(Work::Peer { reference, task });
        Ok(())
    }

    /// The receiving engine opens its own exporter reference. Passing that reference through the
    /// public import operation cannot lend another plugin's private handle or filesystem grant.
    fn start_engine(&mut self, call: &service::Invocation) -> Result<(), api::Failure> {
        let reply = call.reply.clone().ok_or_else(|| {
            api::Failure::new(api::ErrorCode::InvalidState, "A deferred reply is required")
        })?;
        let document: language::DocumentContext =
            serde_json::from_value(call.arguments["document"].clone()).map_err(|_| {
                api::Failure::new(api::ErrorCode::InvalidRequest, "Invalid original document")
            })?;
        if call.method == "snapshot" {
            // The broker method deliberately delegates only editor.read. The request flag must
            // never turn this owner's stronger workspace.read grant into the original caller's.
            let task = api::guest::EditorTask::start(
                api::EditorOperation::LanguageRequest {
                    request: language_control::Request {
                        provider: "analysis".into(),
                        document,
                        method: "novel/snapshot".into(),
                        arguments: Value::Null,
                        context_snapshot: call.arguments["context_snapshot"]
                            .as_bool()
                            .unwrap_or(false),
                        service_sets: Vec::new(),
                        imports: Vec::new(),
                        publish_binding: None,
                    },
                },
                120_000,
            )?;
            self.pending.push(Work::Context { reply, task });
            return Ok(());
        }
        let reference = service::guest::open("novel.module")?;
        let task = api::guest::EditorTask::start(
            api::EditorOperation::LanguageRequest {
                request: language_control::Request {
                    provider: "analysis".into(),
                    document,
                    method: "novel/attach".into(),
                    arguments: Value::Null,
                    context_snapshot: false,
                    service_sets: Vec::new(),
                    imports: vec![language_control::Import {
                        export: "adapter".into(),
                        reference: Some(reference.clone()),
                    }],
                    publish_binding: None,
                },
            },
            120_000,
        );
        match task {
            Ok(task) => self.pending.push(Work::Native {
                reference,
                reply,
                task,
            }),
            Err(error) => {
                let _ = api::guest::close_resource(reference);
                return Err(error);
            }
        }
        Ok(())
    }

    /// Native and broker completions correlate through their own typed task before resources close.
    fn advance(&mut self, event: &api::Notification) {
        let mut completed = Vec::new();
        for (index, work) in self.pending.iter_mut().enumerate() {
            let update = match work {
                Work::Peer { task, .. } => match event {
                    api::Notification::Service(event) => task.update(event),
                    _ => None,
                },
                Work::Native { task, .. } | Work::Context { task, .. } => {
                    task.update(event).map(native_result)
                }
            };
            match update {
                Some(api::RequestUpdate::Completed { result }) => completed.push((index, result)),
                Some(api::RequestUpdate::Cancelled { reason, .. }) => completed.push((
                    index,
                    Err(api::Failure::new(
                        reason,
                        "Original native consumer retired",
                    )),
                )),
                _ => {}
            }
        }
        // Reverse removal preserves the indices captured from the same immutable completion pass.
        for (index, result) in completed.into_iter().rev() {
            match self.pending.remove(index) {
                Work::Peer { reference, .. } => {
                    let _ = api::guest::close_resource(reference);
                    self.receipt = match result {
                        Ok(value) => json!({"state":"complete","value":value}),
                        Err(error) => json!({"state":"failed","error":error}),
                    };
                }
                Work::Native {
                    reference, reply, ..
                } => {
                    let _ = api::guest::close_resource(reference);
                    let _ = service::guest::reply(&reply, result);
                }
                Work::Context { reply, .. } => {
                    let _ = service::guest::reply(&reply, result);
                }
            }
        }
    }

    /// Snapshot exposes observations without creating a UI, mutable source model or native test API.
    fn handle(&mut self, input: api::Input) -> Result<api::Output, api::Failure> {
        match input {
            api::Input::Event {
                event: api::Notification::LanguageService(context),
                ..
            } => {
                let root = context
                    .document
                    .as_ref()
                    .and_then(|document| document.path.rsplit_once('/'))
                    .map(|(root, _)| root)
                    .unwrap_or("first");
                return Ok(api::Output {
                    language_service: Some(language::Proposal {
                        project_root: Some(root.into()),
                        context: Some(language::ProjectContext {
                            key: root.into(),
                            generation: 1,
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                });
            }
            api::Input::Event {
                event: api::Notification::Command { id, arguments, .. },
                ..
            } if id == "run" => {
                if let Err(error) = self.start_consumer("attach", arguments.unwrap_or(Value::Null))
                {
                    self.receipt = json!({"state":"failed","error":error});
                }
            }
            api::Input::Event {
                event: api::Notification::Command { id, arguments, .. },
                ..
            } if id == "snapshot" => {
                if let Err(error) =
                    self.start_consumer("snapshot", arguments.unwrap_or(Value::Null))
                {
                    self.receipt = json!({"state":"failed","error":error});
                }
            }
            api::Input::Event {
                event: api::Notification::Service(service::Notification::Invoke(call)),
                ..
            } => {
                if call.contract == "novel.module" {
                    if let Some(reply) = call.reply {
                        let _ = service::guest::reply(&reply, Ok(Value::Null));
                    }
                } else if let Err(error) = self.start_engine(&call) {
                    if let Some(reply) = call.reply {
                        let _ = service::guest::reply(&reply, Err(error));
                    }
                }
            }
            api::Input::Event { event, .. } => self.advance(&event),
            api::Input::Snapshot => {
                return Ok(api::Output {
                    snapshot: Some(Snapshot {
                        schema: 1,
                        data: json!({"receipt":self.receipt}).to_string(),
                    }),
                    ..Default::default()
                });
            }
            _ => {}
        }
        Ok(Default::default())
    }
}

/// Only the bounded native reply shape is accepted; progress and cancellation preserve their meaning.
fn native_result(update: api::RequestUpdate<api::EditorValue>) -> api::RequestUpdate<Value> {
    match update {
        api::RequestUpdate::Completed { result } => api::RequestUpdate::Completed {
            result: result.and_then(|value| match value {
                api::EditorValue::Language(value) => Ok(value),
                _ => Err(api::Failure::new(
                    api::ErrorCode::InvalidState,
                    "Expected native language reply",
                )),
            }),
        },
        api::RequestUpdate::Cancelled { reason, effect } => {
            api::RequestUpdate::Cancelled { reason, effect }
        }
        api::RequestUpdate::Accepted => api::RequestUpdate::Accepted,
        api::RequestUpdate::Progress { message } => api::RequestUpdate::Progress { message },
    }
}

impl Guest for Fixture {
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| {
            STATE.with(|state| state.borrow_mut().handle(input))
        })
    }
}
export!(Fixture);
