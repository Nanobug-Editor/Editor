//! An unknown independently compiled guest exercises only the public service transport.
use plugin_protocol::{
    api,
    bindings::{Guest, export},
    service,
};
use std::{cell::RefCell, collections::BTreeMap};
thread_local! {static PENDING: RefCell<BTreeMap<u64,(api::ResourceHandle,service::guest::Task)>> = const {RefCell::new(BTreeMap::new())};}
struct SetFixture;
impl Guest for SetFixture {
    /// The command returns public results as bounded data; no host internals or privileged probe API.
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| {
            let reply = match input {
                api::Input::Event {
                    event: api::Notification::CommandInvocation(call),
                    ..
                } => {
                    let operation: api::Operation = serde_json::from_str(
                        call.arguments.as_str().unwrap_or(""),
                    )
                    .map_err(|error| {
                        api::Failure::new(api::ErrorCode::InvalidRequest, error.to_string())
                    })?;
                    // Keep the initiating command while a cold member is prepared, and observe its
                    // actual reply. An Accepted handle alone would not prove the service executed.
                    if let api::Operation::Service {
                        operation:
                            service::Operation::Call {
                                reference,
                                method,
                                arguments,
                                timeout_ms,
                            },
                    } = &operation
                    {
                        let reply = call.reply.clone();
                        match service::guest::Task::start(
                            reference,
                            method,
                            arguments.clone(),
                            *timeout_ms,
                        ) {
                            Ok(task) => {
                                PENDING.with(|pending| {
                                    pending.borrow_mut().insert(reply.resource, (reply, task))
                                });
                                return Ok(Default::default());
                            }
                            Err(error) => {
                                return Ok(api::Output {
                                    service_reply: Some(Ok(serde_json::json!(
                                        serde_json::to_string(&Result::<api::Value, _>::Err(error))
                                            .unwrap()
                                    ))),
                                    ..Default::default()
                                });
                            }
                        }
                    }
                    Some(Ok(serde_json::json!(
                        serde_json::to_string(&api::guest::request(operation)).unwrap()
                    )))
                }
                api::Input::Event {
                    event: api::Notification::Service(service::Notification::Invoke(call)),
                    ..
                } => Some(Ok(call.arguments)),
                api::Input::Event {
                    event: api::Notification::Service(event),
                    ..
                } => {
                    PENDING.with(|pending| {
                        let mut pending = pending.borrow_mut();
                        let mut done = Vec::new();
                        for (id, (_, task)) in pending.iter_mut() {
                            let result = match task.update(&event) {
                                Some(api::RequestUpdate::Completed { result }) => {
                                    Some(result.map(|_| api::Value::Unit))
                                }
                                Some(api::RequestUpdate::Cancelled { reason, .. }) => {
                                    Some(Err(api::Failure::new(reason, "Member call cancelled")))
                                }
                                _ => None,
                            };
                            if let Some(result) = result {
                                done.push((*id, result));
                            }
                        }
                        for (id, result) in done {
                            let (reply, _) = pending.remove(&id).unwrap();
                            let _ = service::guest::reply(
                                &reply,
                                Ok(serde_json::json!(serde_json::to_string(&result).unwrap())),
                            );
                        }
                    });
                    None
                }
                _ => None,
            };
            Ok(api::Output {
                service_reply: reply,
                ..Default::default()
            })
        })
    }
}
export!(SetFixture);
