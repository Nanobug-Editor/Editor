//! Independent HTTP consumer observes task-owned stream/timer cleanup through ordinary panel metadata.
use plugin_protocol::{
    api,
    http::{self, HttpTask},
    ui,
};
use std::cell::RefCell;
thread_local! {
    /// One guest instance owns its task; logical snapshots never serialize live handles.
    static STATE: RefCell<State> = RefCell::new(State::default());
}
/// Business observations are intentionally separate from the SDK's resource lifecycle state.
#[derive(Default)]
struct State {
    /// Negotiated features supplied by the actual host Prepare call.
    api: Option<api::Negotiated>,
    /// The sole owner of this transfer and its polling timer.
    task: Option<HttpTask>,
    /// Bytes observed without copying or decoding response data.
    bytes: usize,
    /// Last HTTP status, including ordinary non-2xx responses.
    status: Option<u16>,
    /// Whether the ordered stream has delivered its final result.
    finished: bool,
    /// Cleanup must be reported independently of the transfer result.
    pending: usize,
    /// Only a stable typed startup code is rendered; the test checks partial cleanup independently.
    start_error: Option<api::ErrorCode>,
    /// Monotonic publication revision.
    revision: u64,
}
impl State {
    /// Handle public lifecycle/command/timer input; snapshots intentionally contain no runtime objects.
    fn dispatch(&mut self, input: api::Input) -> Result<api::Output, api::Failure> {
        match input {
            api::Input::Prepare { api, .. } => self.api = Some(api),
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
                    if let Some(task) = &mut self.task {
                        let report = task.close();
                        if !report.is_complete() {
                            return Err(api::Failure::new(
                                api::ErrorCode::OperationFailed,
                                "Previous cleanup incomplete",
                            ));
                        }
                    }
                    self.bytes = 0;
                    self.status = None;
                    self.finished = false;
                    self.task = Some(
                        HttpTask::start(
                            self.api.as_ref().unwrap(),
                            http::Request::get("http://127.0.0.1:18927/query"),
                            10,
                        )
                        .map_err(|error| {
                            self.pending = error.cleanup.pending.len();
                            self.task = error.recovery;
                            error.cause
                        })?,
                    );
                }
                "close" => {
                    if let Some(task) = &mut self.task {
                        self.pending = task.close().pending.len();
                    }
                }
                "full_timers" => {
                    let mut timers = api::guest::ResourceGroup::default();
                    for _ in 0..32 { timers.timer(60_000, true)?; }
                    let error = HttpTask::start(self.api.as_ref().unwrap(), http::Request::get("http://127.0.0.1:18927/query"), 10).err().expect("native timer quota is full");
                    self.start_error = Some(error.cause.code);
                    self.pending = error.cleanup.pending.len();
                    self.task = error.recovery;
                    let cleanup = timers.close_all();
                    self.pending += cleanup.pending.len();
                }
                "trap" => panic!("controlled lifecycle failure"),
                _ => {}
            },
            api::Input::Event { event, .. } => {
                if let Some(task) = &mut self.task {
                    let batch = task.on_event(&event);
                    self.pending = batch.cleanup.pending.len();
                    for event in batch.events {
                        match event {
                            http::Event::Headers { status, .. } => self.status = Some(status),
                            http::Event::Chunk { bytes } => self.bytes += bytes.len(),
                            http::Event::Finished { .. } | http::Event::Failed { .. } => {
                                self.finished = true
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        self.revision += 1;
        let text = serde_json::json!({"bytes":self.bytes,"status":self.status,"finished":self.finished,"pending":self.pending,"start_error":self.start_error}).to_string();
        Ok(api::Output {
            views: vec![api::View {
                panel: "result".into(),
                document: ui::Document::new(ui::Node::text("receipt", text))
                    .revision(self.revision),
            }],
            ..Default::default()
        })
    }
}
/// Independent packages export only the public SDK world.
struct Guest;
impl plugin_protocol::bindings::Guest for Guest {
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| {
            STATE.with(|state| state.borrow_mut().dispatch(input))
        })
    }
}
plugin_protocol::bindings::export!(Guest with_types_in plugin_protocol::bindings);
