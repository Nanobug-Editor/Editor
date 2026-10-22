//! Unfamiliar providers compose a real deferred service call and an owned native binding request.
use plugin_protocol::{
    api,
    bindings::{Guest, export},
    language, language_control, service,
};
use serde_json::{Value, json};
use std::{cell::RefCell, collections::BTreeMap};

enum Task {
    Peer(service::guest::Task),
    Native(api::guest::EditorTask),
}
struct Pending {
    reply: api::ResourceHandle,
    reference: Option<api::ResourceHandle>,
    task: Task,
}
thread_local! { static PENDING: RefCell<BTreeMap<u64, Pending>> = const { RefCell::new(BTreeMap::new()) }; }
struct Delegation;
impl Guest for Delegation {
    /// Contract identity chooses the fixture role; production host code never knows these names.
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| {
            if let api::Input::Event {
                event: api::Notification::LanguageService(context),
                ..
            } = &input
            {
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
            if let api::Input::Event { event, .. } = input {
                PENDING.with(|pending| {
                    let mut pending = pending.borrow_mut();
                    if let api::Notification::Service(service::Notification::Invoke(call)) = event {
                        let result = begin(&call);
                        match result {
                            Ok(task) => {
                                pending.insert(task.reply.resource, task);
                            }
                            Err(error) => {
                                let _ = service::guest::reply(&call.reply.unwrap(), Err(error));
                            }
                        }
                    } else {
                        let mut done = Vec::new();
                        for (id, task) in pending.iter_mut() {
                            let update = match &mut task.task {
                                Task::Peer(peer) => match &event {
                                    api::Notification::Service(event) => peer.update(event),
                                    _ => None,
                                },
                                Task::Native(native) => {
                                    native.update(&event).map(|update| match update {
                                        api::RequestUpdate::Completed { result } => {
                                            api::RequestUpdate::Completed {
                                                result: result.and_then(|value| match value {
                                                    api::EditorValue::LanguageBinding {
                                                        binding,
                                                        ..
                                                    } => Ok(json!({"binding":binding})),
                                                    _ => Err(api::Failure::new(
                                                        api::ErrorCode::InvalidState,
                                                        "Expected a native binding",
                                                    )),
                                                }),
                                            }
                                        }
                                        api::RequestUpdate::Cancelled { reason, effect } => {
                                            api::RequestUpdate::Cancelled { reason, effect }
                                        }
                                        api::RequestUpdate::Accepted => {
                                            api::RequestUpdate::Accepted
                                        }
                                        api::RequestUpdate::Progress { message } => {
                                            api::RequestUpdate::Progress { message }
                                        }
                                    })
                                }
                            };
                            match update {
                                Some(api::RequestUpdate::Completed { result }) => {
                                    done.push((*id, result))
                                }
                                Some(api::RequestUpdate::Cancelled { reason, .. }) => done.push((
                                    *id,
                                    Err(api::Failure::new(reason, "Fixture wait ended")),
                                )),
                                _ => {}
                            }
                        }
                        for (id, result) in done {
                            let task = pending.remove(&id).unwrap();
                            let _ = service::guest::reply(&task.reply, result);
                            if let Some(reference) = task.reference {
                                let _ = api::guest::close_resource(reference);
                            }
                        }
                    }
                });
            }
            Ok(Default::default())
        })
    }
}
/// Source prepare borrows the public engine service; engine attach asks the actual native host.
fn begin(call: &service::Invocation) -> Result<Pending, api::Failure> {
    let reply = call.reply.clone().ok_or_else(|| {
        api::Failure::new(api::ErrorCode::InvalidState, "Deferred reply required")
    })?;
    if call.contract == "novel.prepare" {
        let reference = service::guest::open("novel.engine")?;
        let task =
            service::guest::Task::start(&reference, "attach", call.arguments.clone(), 120_000)?;
        return Ok(Pending {
            reply,
            reference: Some(reference),
            task: Task::Peer(task),
        });
    }
    let document: language::DocumentContext =
        serde_json::from_value(call.arguments["document"].clone()).map_err(|_| {
            api::Failure::new(api::ErrorCode::InvalidRequest, "Invalid source document")
        })?;
    let task = api::guest::EditorTask::start(
        api::EditorOperation::LanguageRequest {
            request: language_control::Request {
                provider: "analysis".into(),
                document: language::DocumentContext {
                    generation: 0,
                    ..document.clone()
                },
                method: "novel/attach".into(),
                arguments: Value::Null,
                context_snapshot: false,
                service_sets: Vec::new(),
                imports: vec![language_control::Import {
                    export: "adapter".into(),
                    reference: None,
                }],
                publish_binding: Some(document),
            },
        },
        120_000,
    )?;
    Ok(Pending {
        reply,
        reference: None,
        task: Task::Native(task),
    })
}
export!(Delegation);
