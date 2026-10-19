//! Controlled database catalog exercises lazy pages, retries and sealed delayed replies.
//! It deliberately owns no database connection; the domain catalog is provider policy.
use plugin_protocol::{
    api,
    bindings::{Guest, export},
    tree, ui,
};
use std::cell::RefCell;
thread_local! { static STATE: RefCell<Catalog> = RefCell::new(Catalog::default()); }
#[derive(Default)]
struct Catalog {
    handle: Option<api::ResourceHandle>,
    failed_once: bool,
    pending: Vec<tree::Request>,
    status: String,
    english: bool,
}
impl Catalog {
    /// The controlled service returns sixteen records per page despite the larger host quota.
    fn page(&mut self, request: tree::Request) -> Result<(), api::Failure> {
        let page = match request.parent.as_deref() {
            None => tree::Page {
                nodes: vec![tree::Node::new("main", "main", true)],
                next: None,
            },
            Some("main") => tree::Page {
                nodes: vec![
                    tree::Node::new("users", "users (512)", true),
                    tree::Node::new(
                        "fault-once",
                        if self.english {
                            "Retry demonstration"
                        } else {
                            "错误重试示例"
                        },
                        true,
                    ),
                    tree::Node::new(
                        "slow",
                        if self.english {
                            "Delayed demonstration"
                        } else {
                            "延迟加载示例"
                        },
                        true,
                    ),
                ],
                next: None,
            },
            Some("users") => {
                let start = request
                    .cursor
                    .as_deref()
                    .unwrap_or("0")
                    .parse::<usize>()
                    .map_err(|_| {
                        api::Failure::new(api::ErrorCode::InvalidRequest, "Unknown catalog cursor")
                    })?;
                if start >= 512 {
                    return Err(api::Failure::new(
                        api::ErrorCode::InvalidRequest,
                        "Catalog cursor exhausted",
                    ));
                }
                let end = (start + 16).min(512);
                let nodes = (start..end)
                    .map(|id| {
                        let mut node =
                            tree::Node::new(format!("user-{id}"), format!("User {id:03}"), false);
                        let command = tree::Command {
                            command: "inspect-object".into(),
                            disabled: false,
                            separator_before: false,
                            label: ui::LocalizedText::new("Inspect object", "查看对象"),
                            arguments: serde_json::json!(format!("user-{id}")),
                        };
                        node.activation = Some(command.clone());
                        node.menu.push(command);
                        node
                    })
                    .collect();
                tree::Page {
                    nodes,
                    next: (end < 512).then(|| end.to_string()),
                }
            }
            Some("fault-once") if !self.failed_once => {
                self.failed_once = true;
                return tree::reply(
                    request,
                    Err(api::Failure::new(
                        api::ErrorCode::OperationFailed,
                        if self.english {
                            "Service unavailable; retry this node"
                        } else {
                            "服务暂不可用，请重试此节点"
                        },
                    )),
                );
            }
            Some("fault-once") => {
                let mut node = tree::Node::new(
                    "recovered",
                    if self.english {
                        "Recovered"
                    } else {
                        "已恢复"
                    },
                    false,
                );
                // One object exposes distinct views through the same declared typed command.
                // Activation and each menu item retain their own arguments and translated labels.
                node.activation = Some(tree::Command {
                    command: "inspect-object".into(),
                    disabled: false,
                    separator_before: false,
                    label: ui::LocalizedText::new("Inspect summary", "查看摘要"),
                    arguments: serde_json::json!("recovered:summary"),
                });
                node.menu = [
                    ("Inspect details", "查看详情", "recovered:details"),
                    ("Inspect schema", "查看结构", "recovered:schema"),
                ]
                .into_iter()
                .map(|(en, zh_cn, object)| tree::Command {
                    command: "inspect-object".into(),
                    disabled: false,
                    separator_before: false,
                    label: ui::LocalizedText::new(en, zh_cn),
                    arguments: serde_json::json!(object),
                })
                .collect();
                tree::Page {
                    nodes: vec![node],
                    next: None,
                }
            }
            Some("slow") => {
                if self.pending.len() >= 8 {
                    return Err(api::Failure::new(
                        api::ErrorCode::LimitExceeded,
                        "Delayed service budget exhausted",
                    ));
                }
                self.pending.push(request);
                self.status = if self.english {
                    "Waiting for Complete delayed page"
                } else {
                    "等待“完成延迟页”命令"
                }
                .into();
                return Ok(());
            }
            _ => {
                return Err(api::Failure::new(
                    api::ErrorCode::NotFound,
                    "Catalog node not found",
                ));
            }
        };
        tree::reply(request, Ok(page))
    }
    /// Stale callbacks remain visible failures; they cannot publish into refreshed or folded branches.
    fn complete(&mut self) {
        for request in self.pending.drain(..) {
            let result = tree::reply(
                request,
                Ok(tree::Page {
                    nodes: vec![tree::Node::new("delayed-child", "Delayed child", false)],
                    next: None,
                }),
            );
            self.status = match result {
                Ok(()) => if self.english {
                    "Delayed page completed"
                } else {
                    "延迟页已完成"
                }
                .into(),
                Err(error) => format!("{:?}: {}", error.code, error.message),
            };
        }
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
                self.handle = Some(tree::create()?);
                self.status = "Database catalog ready / 数据库目录就绪".into();
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
            } => {
                if call.id == "inspect-object" {
                    self.status =
                        format!("Object / 对象: {}", call.arguments.as_str().unwrap_or(""));
                    return Ok(Some(Ok(serde_json::Value::Null)));
                }
            }
            api::Input::Event {
                event: api::Notification::Command { id, arguments, .. },
                ..
            } => match id.as_str() {
                "refresh-database" => {
                    if let Some(handle) = &self.handle {
                        tree::refresh(handle.clone())?;
                    }
                }
                "complete-pending" => self.complete(),
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
            _ => {}
        }
        Ok(None)
    }
    /// Stable scene revision retains native focus while page state evolves independently.
    fn view(&self) -> ui::Document {
        let mut nodes = vec![ui::Node::text("catalog-status", self.status.clone())];
        if let Some(handle) = &self.handle {
            nodes.push(
                ui::Node {
                    id: "catalog-tree".into(),
                    kind: ui::Kind::Tree {
                        handle: handle.clone(),
                    },
                    ..ui::Node::text("unused", "")
                }
                .grow(),
            );
        }
        // The root gives its virtual tree a bounded share of the native panel's available height.
        ui::Document::new(ui::Node::column("catalog", nodes).grow()).revision(1)
    }
}
struct Database;
impl Guest for Database {
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| {
            STATE.with(|state| {
                let mut state = state.borrow_mut();
                let reply = match state.input(input) {
                    Ok(reply) => reply,
                    Err(error) => {
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
export!(Database);
