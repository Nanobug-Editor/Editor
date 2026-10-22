//! An independent document observer uses SDK ownership and intent filtering instead of its own handles.
use plugin_protocol::{
    api::{
        self,
        guest::{EditorIntent, ResourceGroup},
    },
    ui,
};
use std::cell::RefCell;
thread_local! {
    /// Actual guest instance state; only business counters are rendered.
    static STATE: RefCell<State> = RefCell::new(State::default());
}
/// No second document model or Undo state exists in this consumer.
#[derive(Default)]
struct State {
    /// Independent subscription lifetime.
    group: ResourceGroup,
    /// Only the most recent asynchronous query may publish a result.
    intent: EditorIntent,
    /// Number of terminal results actually accepted for the current intent.
    results: u64,
    /// Monotonic native publication identity.
    revision: u64,
}
impl State {
    /// Exercise helpers exclusively through public lifecycle and notification contracts.
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
                event: api::Notification::Command { id, .. },
                ..
            } => match id.as_str() {
                "query" => {
                    if self.group.is_empty() {
                        self.group.documents()?;
                    }
                    self.intent
                        .replace(
                            api::EditorOperation::ListDocuments,
                            10000,
                            api::CancelMode::StopWaiting,
                        )
                        .map_err(|error| error.cause)?;
                }
                "close" => {
                    let requests = self.intent.close();
                    let subscriptions = self.group.close_all();
                    if !requests.is_complete() || !subscriptions.is_complete() {
                        return Err(api::Failure::new(
                            api::ErrorCode::OperationFailed,
                            "Cleanup incomplete",
                        ));
                    }
                }
                _ => {}
            },
            api::Input::Event { event, .. } => {
                self.group.observe(&event);
                if let Some(update) = self.intent.update(&event) {
                    if update.is_terminal() {
                        self.results += 1;
                    }
                }
            }
            _ => {}
        }
        self.revision += 1;
        Ok(api::Output {
            views: vec![api::View {
                panel: "result".into(),
                document: ui::Document::new(ui::Node::text(
                    "receipt",
                    serde_json::json!({"results":self.results,"subscriptions":self.group.len()})
                        .to_string(),
                ))
                .revision(self.revision),
            }],
            ..Default::default()
        })
    }
}
/// Export the same public world used by every third-party executable plugin.
struct Guest;
impl plugin_protocol::bindings::Guest for Guest {
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| {
            STATE.with(|state| state.borrow_mut().dispatch(input))
        })
    }
}
plugin_protocol::bindings::export!(Guest with_types_in plugin_protocol::bindings);
