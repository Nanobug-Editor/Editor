//! A real independently SDK-built guest observes reset of a later timer from an earlier due callback.
use plugin_protocol::{api, timers, ui};
use std::cell::RefCell;

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
#[derive(Default)]
struct State {
    a: Option<api::ResourceHandle>,
    b: Option<api::ResourceHandle>,
    repeat: bool,
    events: Vec<&'static str>,
    revision: u64,
}
impl State {
    /// Only public timer operations produce these receipts; one-shot callbacks arrive after release.
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
            api::Input::Event {
                event: api::Notification::Command { id, arguments, .. },
                ..
            } if id == "arm" => {
                self.release();
                self.events.clear();
                self.repeat = arguments
                    .as_ref()
                    .and_then(|value| value.get("repeat"))
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                self.a = Some(timers::start(10, false)?);
                self.b = Some(timers::start(10, self.repeat)?);
            }
            api::Input::Event {
                event: api::Notification::Command { id, .. },
                ..
            } if id == "release" => self.release(),
            api::Input::Event {
                event: api::Notification::Timer { handle, .. },
                ..
            } if self.a.as_ref() == Some(&handle) => {
                self.a = None;
                timers::reset(self.b.as_ref().expect("B was armed after A"), 300)?;
                self.events.push("a-reset-b");
            }
            api::Input::Event {
                event: api::Notification::Timer { handle, .. },
                ..
            } if self.b.as_ref() == Some(&handle) => {
                self.events.push("b-tick");
                if !self.repeat {
                    self.b = None;
                }
            }
            _ => {}
        }
        self.revision += 1;
        Ok(api::Output {
            views: vec![api::View {
                panel: "cloud".into(),
                document: ui::Document::new(ui::Node::new(
                    "receipt",
                    ui::Kind::Text {
                        text: serde_json::json!({"events":self.events,"pending_b":self.b})
                            .to_string(),
                    },
                ))
                .revision(self.revision),
            }],
            ..Default::default()
        })
    }
    /// Repeat timers remain owned until explicit close; starting another pair cannot leak older handles.
    fn release(&mut self) {
        for handle in [self.a.take(), self.b.take()].into_iter().flatten() {
            let _ = api::guest::close_resource(handle);
        }
    }
}
struct Guest;
impl plugin_protocol::bindings::Guest for Guest {
    /// Correlation and failures use the published wire contract, exactly like independently installed guests.
    fn dispatch(payload: String) -> Result<String, String> {
        let invocation: api::Invocation =
            serde_json::from_str(&payload).map_err(|error| error.to_string())?;
        let result = STATE.with(|state| state.borrow_mut().dispatch(invocation.message));
        serde_json::to_string(&api::Completion {
            id: invocation.id,
            result,
        })
        .map_err(|error| error.to_string())
    }
}
plugin_protocol::bindings::export!(Guest with_types_in plugin_protocol::bindings);
