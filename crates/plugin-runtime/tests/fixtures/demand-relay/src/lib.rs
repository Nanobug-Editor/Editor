//! A real SDK relay keeps an outer invocation pending while its declared child service activates.
use plugin_protocol::{api, service, ui};
use serde_json::json;
use std::cell::RefCell;

thread_local! { static STATE: RefCell<Relay> = RefCell::new(Relay::default()); }

#[derive(Default)]
struct Relay {
    /// Reply and child task are runtime handles, deliberately excluded from logical snapshots.
    reply: Option<api::ResourceHandle>,
    task: Option<service::guest::Task>,
    receipt: String,
    revision: u64,
}

impl Relay {
    /// Deferred and immediately completed ancestors use the same public child-call mechanism.
    fn invoke(&mut self, call: service::Invocation) -> Result<api::Output, api::Failure> {
        if call.method == "echo" {
            self.receipt = call.arguments.as_str().unwrap_or_default().into();
            let mut output = self.view();
            output.service_reply = Some(Ok(call.arguments));
            return Ok(output);
        }
        let arguments: serde_json::Value =
            serde_json::from_str(call.arguments.as_str().ok_or_else(|| {
                api::Failure::new(api::ErrorCode::InvalidRequest, "Expected relay policy")
            })?)
            .map_err(|error| {
                api::Failure::new(api::ErrorCode::InvalidRequest, error.to_string())
            })?;
        let contract = arguments["contract"].as_str().ok_or_else(|| {
            api::Failure::new(api::ErrorCode::InvalidRequest, "Expected child contract")
        })?;
        let reference = service::guest::open(contract)?;
        let task =
            service::guest::Task::start(&reference, "echo", arguments["value"].clone(), 120_000);
        // Opening and closing a reference does not borrow another instance's resource authority.
        api::guest::close_resource(reference)?;
        self.task = Some(task?);
        self.receipt = "Child accepted".into();
        let mut output = self.view();
        if arguments["defer"].as_bool().unwrap_or(true) {
            self.reply = call.reply;
        } else {
            self.reply = None;
            output.service_reply = Some(Ok(json!("Relay queued")));
        }
        Ok(output)
    }

    /// Return the child's real terminal result only to the retained outer invocation.
    fn dispatch(&mut self, input: api::Input) -> Result<api::Output, api::Failure> {
        match input {
            api::Input::Snapshot => {
                return Ok(api::Output {
                    snapshot: Some(plugin_protocol::Snapshot {
                        schema: 1,
                        data: "{}".into(),
                    }),
                    ..Default::default()
                });
            }
            api::Input::Activate => self.receipt = "Activated".into(),
            api::Input::Event {
                event: api::Notification::Service(service::Notification::Invoke(call)),
                ..
            } => return self.invoke(call),
            api::Input::Event {
                event: api::Notification::Service(notification),
                ..
            } => {
                if let service::Notification::InvocationCancelled { request, .. } = &notification {
                    if self.reply.as_ref() == Some(request) {
                        self.reply = None;
                        if let Some(task) = self.task.take() {
                            // Retirement may already have released this source-owned request.
                            let _ = task.cancel(api::CancelMode::TryTerminate);
                        }
                    }
                } else if let Some(update) = self
                    .task
                    .as_mut()
                    .and_then(|task| task.update(&notification))
                {
                    self.receipt = serde_json::to_string(&update).unwrap();
                    if update.is_terminal() {
                        self.task = None;
                        if let Some(reply) = self.reply.take() {
                            let result = match update {
                                api::RequestUpdate::Completed { result } => result,
                                api::RequestUpdate::Cancelled { reason, .. } => {
                                    Err(api::Failure::new(reason, "Child service wait ended"))
                                }
                                _ => unreachable!(),
                            };
                            service::guest::reply(&reply, result)?;
                        }
                    }
                }
            }
            _ => return Ok(api::Output::default()),
        }
        Ok(self.view())
    }

    /// Observable receipts come from the guest's native text view, rather than a host test projection.
    fn view(&mut self) -> api::Output {
        self.revision += 1;
        api::Output {
            views: vec![api::View {
                panel: "receipt".into(),
                document: ui::Document::new(ui::Node::new(
                    "receipt",
                    ui::Kind::Text {
                        text: self.receipt.clone(),
                    },
                ))
                .revision(self.revision),
            }],
            ..Default::default()
        }
    }
}

struct Guest;
impl plugin_protocol::bindings::Guest for Guest {
    /// The independent component enters through the standard SDK dispatch envelope.
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| {
            STATE.with(|state| state.borrow_mut().dispatch(input))
        })
    }
}
plugin_protocol::bindings::export!(Guest with_types_in plugin_protocol::bindings);
