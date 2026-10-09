//! A standalone cloud consumer uses only the host's exported SDK and owned cloud resources.
const TITLE: &str = "Cloud translation";
const TITLE_ZH: &str = "云翻译";
const DEFAULT_URL: &str = "http://127.0.0.1:18906/translate";
const DEFAULT_QUERY: &str = "hello 世界😀";
use plugin_protocol::{Snapshot, api, http, interaction, storage, timers, ui};
use std::cell::RefCell;
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
#[derive(Default)]
struct State {
    environment: plugin_protocol::Environment,
    text: String,
    revision: u64,
    input: Option<api::ResourceHandle>,
    stream: Option<api::ResourceHandle>,
    poll: Option<api::ResourceHandle>,
    refresh: Option<api::ResourceHandle>,
    progress: Option<api::ResourceHandle>,
    cache: Option<api::ResourceHandle>,
    bytes: Vec<u8>,
    url: String,
    query: String,
    status: Option<u16>,
    /// Bounded diagnostic receipts help authors verify disposable timers without exposing host internals.
    timer_observations: std::collections::BTreeMap<u64, u64>,
}
impl State {
    /// Tokens stay in the system credential facility; UI and Snapshot contain only non-sensitive status.
    fn dispatch(&mut self, message: api::Input) -> Result<api::Output, api::Failure> {
        match message {
            api::Input::Prepare { environment, .. } => {
                self.environment = environment;
                self.text = self.label("Ready for a cloud query", "云查询已就绪");
            }
            api::Input::Activate => {
                // Optional package policy exercises a startup request inside the normal activation transaction.
                if let Ok(bytes) = api::guest::read_asset("activation-policy.json") {
                    let policy: serde_json::Value =
                        serde_json::from_slice(&bytes).map_err(|_| {
                            api::Failure::new(
                                api::ErrorCode::InvalidRequest,
                                "Invalid activation policy",
                            )
                        })?;
                    let request: http::Request = serde_json::from_value(policy["request"].clone())
                        .map_err(|_| {
                            api::Failure::new(
                                api::ErrorCode::InvalidRequest,
                                "Invalid startup request",
                            )
                        })?;
                    self.stream = Some(http::start(request)?);
                    self.poll = Some(timers::start(1000, true)?);
                    let data = api::guest::open_data()?;
                    api::guest::write_file(
                        &data,
                        "startup-state.txt",
                        b"candidate initialization".to_vec(),
                    )?;
                    api::guest::close_resource(data)?;
                    if policy["fail"].as_bool() == Some(true) {
                        return Err(api::Failure::new(
                            api::ErrorCode::OperationFailed,
                            "Startup policy rejected activation",
                        ));
                    }
                }
            }
            api::Input::Snapshot => {
                return Ok(api::Output {
                    snapshot: Some(Snapshot {
                        schema: 1,
                        data: "{}".into(),
                    }),
                    ..Default::default()
                });
            }
            api::Input::Event { event, .. } => match event {
                api::Notification::Theme(environment) => self.environment = environment,
                api::Notification::Service(plugin_protocol::service::Notification::Invoke(
                    call,
                )) => {
                    // The public broker keeps caller authority; this diagnostic method cannot borrow a Token.
                    let result = call
                        .arguments
                        .as_str()
                        .and_then(|payload| serde_json::from_str::<api::Operation>(payload).ok())
                        .ok_or_else(|| {
                            api::Failure::new(
                                api::ErrorCode::InvalidRequest,
                                "Invalid service operation",
                            )
                        })
                        .and_then(api::guest::request);
                    let redacted = match result {
                        Ok(api::Value::Secret(_)) => {
                            Ok(api::Value::Bytes(b"unexpected-secret-access".to_vec()))
                        }
                        other => other,
                    };
                    return Ok(api::Output {
                        service_reply: Some(Ok(serde_json::Value::String(
                            serde_json::to_string(&redacted).unwrap(),
                        ))),
                        ..Default::default()
                    });
                }
                api::Notification::Service(plugin_protocol::service::Notification::Request {
                    update,
                    ..
                }) => {
                    self.text = serde_json::to_string(&update).unwrap();
                }
                api::Notification::Command { id, arguments, .. } => match id.as_str() {
                    "set-token" => {
                        self.release();
                        self.input = Some(interaction::start(
                            interaction::Operation::Input {
                                title: self.label("API token", "API 密钥"),
                                value: String::new(),
                                placeholder: None,
                                password: true,
                                max_bytes: 2560,
                            },
                            60000,
                        )?);
                    }
                    "delete-token" => {
                        self.release();
                        plugin_protocol::secrets::delete("api-token")?;
                        self.text = self.label("Token deleted", "密钥已删除");
                    }
                    "query" => {
                        let arguments = arguments.unwrap_or_default();
                        self.url = arguments
                            .get("url")
                            .and_then(|value| value.as_str())
                            .unwrap_or(DEFAULT_URL)
                            .into();
                        self.query = arguments
                            .get("text")
                            .and_then(|value| value.as_str())
                            .unwrap_or(DEFAULT_QUERY)
                            .into();
                        self.begin()?;
                    }
                    "refresh" => {
                        if let Some(handle) = &self.refresh {
                            timers::reset(handle, 300)?;
                        } else {
                            self.refresh = Some(timers::start(300, false)?);
                        }
                        self.text = self.label("Refresh debounced", "已合并刷新请求");
                    }
                    "cancel" => {
                        self.release();
                        self.text = self.label("Cancelled", "已取消");
                    }
                    "read-cache" => {
                        let cache = storage::open(storage::Area::Cache)?;
                        let result = api::guest::read_file(&cache, "last-query.txt");
                        let _ = api::guest::close_resource(cache);
                        self.text = result
                            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                            .unwrap_or_else(|error| format!("{:?}", error.code));
                    }
                    "probe" => {
                        let operation: api::Operation = serde_json::from_value(
                            arguments.unwrap_or_default(),
                        )
                        .map_err(|_| {
                            api::Failure::new(
                                api::ErrorCode::InvalidRequest,
                                "Invalid diagnostic request",
                            )
                        })?;
                        let result = api::guest::request(operation);
                        // Diagnostic fixtures redact secret reads even when using a synthetic test Token.
                        self.text = match result {
                            Ok(api::Value::Secret(value)) => if value.is_some() {
                                "secret-present"
                            } else {
                                "secret-missing"
                            }
                            .into(),
                            value => serde_json::to_string(&value).unwrap(),
                        };
                    }
                    "timer-observations" => {
                        self.text = serde_json::to_string(&self.timer_observations).unwrap()
                    }
                    _ => {}
                },
                api::Notification::Request { handle, update } => {
                    if self.input.as_ref() == Some(&handle) && update.is_terminal() {
                        let _ = api::guest::close_resource(handle.clone());
                        self.input = None;
                        match update {
                            api::RequestUpdate::Completed {
                                result:
                                    Ok(api::EditorValue::Interaction(interaction::Value::Input(value))),
                            } => {
                                plugin_protocol::secrets::write("api-token", value.into_bytes())?;
                                self.text = self.label("Token stored securely", "密钥已安全保存");
                            }
                            _ => self.text = self.label("Token input cancelled", "密钥输入已取消"),
                        }
                    } else if self.progress.as_ref() == Some(&handle) && update.is_terminal() {
                        self.release();
                        self.text = self.label("Cancelled", "已取消");
                    }
                }
                api::Notification::Timer { handle, .. } if self.poll.as_ref() == Some(&handle) => {
                    self.consume()?
                }
                api::Notification::Timer { handle, .. }
                    if self.refresh.as_ref() == Some(&handle) =>
                {
                    self.refresh = None;
                    self.begin()?;
                }
                api::Notification::Timer { handle, sequence } => {
                    self.timer_observations.insert(handle.resource, sequence);
                    if self.timer_observations.len() > 32 {
                        if let Some(first) = self.timer_observations.keys().next().copied() {
                            self.timer_observations.remove(&first);
                        }
                    }
                }
                _ => {}
            },
        }
        self.view()
    }
    /// Translation business policy uses a POST text body; transport and credentials remain host mechanisms.
    fn request(&self) -> http::Request {
        let mut request = http::Request::get(self.url.clone());
        request.method = "POST".into();
        request.body = self.query.as_bytes().to_vec();
        request.headers.push(http::Header {
            name: "Content-Type".into(),
            value: "text/plain; charset=utf-8".into(),
        });
        request
    }
    fn label(&self, english: &str, chinese: &str) -> String {
        if self.environment.locale.starts_with("en") {
            english
        } else {
            chinese
        }
        .into()
    }
    /// Every new intent releases its older network stream and progress; no stale result replaces the current query.
    fn begin(&mut self) -> Result<(), api::Failure> {
        self.release();
        self.bytes.clear();
        self.status = None;
        if self.url.is_empty() {
            self.url = DEFAULT_URL.into();
            self.query = DEFAULT_QUERY.into();
        }
        let token = plugin_protocol::secrets::read("api-token")?;
        let mut request = self.request();
        if let Some(token) = token {
            let token = String::from_utf8(token).map_err(|_| {
                api::Failure::new(api::ErrorCode::InvalidRequest, "Token is not UTF-8")
            })?;
            request.headers.push(http::Header {
                name: "Authorization".into(),
                value: format!("Bearer {token}"),
            });
        }
        request.max_response_bytes = 65536;
        self.stream = Some(http::start(request)?);
        self.poll = Some(timers::start(25, true)?);
        self.cache = Some(storage::open(storage::Area::Cache)?);
        self.progress = Some(interaction::start(
            interaction::Operation::Progress {
                title: self.label(TITLE, TITLE_ZH),
                message: self.label("Receiving response", "正在接收结果"),
                cancellable: true,
            },
            120000,
        )?);
        self.text = self.label("Receiving response", "正在接收结果");
        Ok(())
    }
    /// Pull at most four chunks per tick, preserving host queue backpressure and UTF-8 boundaries.
    fn consume(&mut self) -> Result<(), api::Failure> {
        let Some(stream) = self.stream.clone() else {
            return Ok(());
        };
        for _ in 0..4 {
            let Some(event) = http::read(&stream)? else {
                break;
            };
            match event {
                http::Event::Headers { status, .. } => {
                    self.status = Some(status);
                    self.text = format!("HTTP {status}");
                }
                http::Event::Chunk { bytes } => {
                    self.bytes.extend(bytes);
                    let valid = std::str::from_utf8(&self.bytes)
                        .map(|_| self.bytes.len())
                        .unwrap_or_else(|error| error.valid_up_to());
                    self.text = format!(
                        "HTTP {} · {} bytes\n{}",
                        self.status.unwrap_or(0),
                        self.bytes.len(),
                        String::from_utf8_lossy(&self.bytes[..valid])
                    );
                    if let Some(progress) = &self.progress {
                        let _ = interaction::update(
                            progress.clone(),
                            self.label("Receiving response", "正在接收结果"),
                            None,
                        );
                    }
                }
                http::Event::Finished { bytes } => {
                    if let Some(cache) = &self.cache {
                        api::guest::write_file(cache, "last-query.txt", self.bytes.clone())?;
                    }
                    self.text = format!(
                        "HTTP {} · {bytes} bytes · {}\n{}",
                        self.status.unwrap_or(0),
                        self.label("Complete", "已完成"),
                        String::from_utf8_lossy(&self.bytes)
                    );
                    self.finish_query();
                    break;
                }
                http::Event::Failed { error } => {
                    self.text = format!("{:?}: {}", error.code, error.message);
                    self.finish_query();
                    break;
                }
            }
        }
        Ok(())
    }
    /// Cancelling or replacing an intent revokes its deferred refresh and pending input as well as HTTP.
    fn release(&mut self) {
        for handle in [self.refresh.take(), self.input.take()]
            .into_iter()
            .flatten()
        {
            let _ = api::guest::close_resource(handle);
        }
        self.finish_query();
    }
    /// Finishing the current transfer preserves a newer explicitly scheduled refresh or Token input.
    fn finish_query(&mut self) {
        for handle in [self.stream.take(), self.poll.take(), self.cache.take()]
            .into_iter()
            .flatten()
        {
            let _ = api::guest::close_resource(handle);
        }
        if let Some(handle) = self.progress.take() {
            let _ = interaction::finish(handle.clone());
            let _ = api::guest::close_resource(handle);
        }
    }
    /// Keep complete replies/cache diagnostics in a scrollable set of bounded Unicode-safe Text fragments.
    fn result_node(&self) -> ui::Node {
        const FRAGMENT_BYTES: usize = 32 * 1024;
        if self.text.len() <= FRAGMENT_BYTES {
            return ui::Node::new(
                "result",
                ui::Kind::Text {
                    text: self.text.clone(),
                },
            );
        }
        let mut remaining = self.text.as_str();
        let mut children = vec![];
        while !remaining.is_empty() {
            // Invalid response bytes may expand threefold under lossy UTF-8. Split the rendered
            // string on character boundaries; retain all original bytes separately in the private cache.
            let end = remaining.floor_char_boundary(FRAGMENT_BYTES.min(remaining.len()));
            let (part, rest) = remaining.split_at(end);
            children.push(ui::Node::new(
                format!("result-part-{}", children.len()),
                ui::Kind::Text { text: part.into() },
            ));
            remaining = rest;
        }
        // Fill the panel's bounded viewport; every retained fragment stays reachable by native scrolling.
        ui::Node::scroll("result", ui::Node::column("result-body", children)).grow()
    }
    fn view(&mut self) -> Result<api::Output, api::Failure> {
        self.revision += 1;
        let mut document = ui::Document::new(self.result_node()).revision(self.revision);
        document.tools = vec![];
        Ok(api::Output {
            views: vec![api::View {
                panel: "cloud".into(),
                document,
            }],
            ..Default::default()
        })
    }
}
struct Guest;
impl plugin_protocol::bindings::Guest for Guest {
    fn dispatch(payload: String) -> Result<String, String> {
        let invocation: api::Invocation =
            serde_json::from_str(&payload).map_err(|_| "Invalid host invocation".to_string())?;
        let result = STATE.with(|state| {
            let mut state = state.borrow_mut();
            let initializing = matches!(
                &invocation.message,
                api::Input::Prepare { .. } | api::Input::Activate
            );
            match state.dispatch(invocation.message) {
                Ok(output) => Ok(output),
                Err(error) if initializing => Err(error),
                Err(error) => {
                    state.release();
                    state.text = format!("{:?}: {}", error.code, error.message);
                    state.view()
                }
            }
        });
        serde_json::to_string(&api::Completion {
            id: invocation.id,
            result,
        })
        .map_err(|error| error.to_string())
    }
}
plugin_protocol::bindings::export!(Guest with_types_in plugin_protocol::bindings);
