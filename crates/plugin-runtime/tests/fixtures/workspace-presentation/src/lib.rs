//! An independent review queue proves that workspace presentation contains no Git policy.
use plugin_protocol::{
    api,
    bindings::{Guest, export},
    presentation, tree, ui,
};
use std::cell::RefCell;

/// Review state contains metadata only and one instance-owned presentation collection.
#[derive(Default)]
struct ReviewState {
    resource: Option<api::ResourceHandle>,
    remaining: usize,
    renamed: bool,
}
thread_local! { static REVIEW: RefCell<ReviewState> = RefCell::default(); }

impl ReviewState {
    /// Update the same owned collection; completed review removes filename colors and disables Next.
    fn publish(&mut self) -> Result<(), api::Failure> {
        if self.resource.is_none() {
            self.resource = Some(presentation::create()?);
        }
        let resource = self.resource.as_ref().unwrap().clone();
        presentation::publish_files(
            resource.clone(),
            if self.remaining == 0 {
                Vec::new()
            } else {
                vec![presentation::FileDecoration {
                    path: "review.txt".into(),
                    tone: tree::Tone::Informative,
                    tooltip: ui::LocalizedText::new("Awaiting review", "等待审查"),
                }]
            },
        )?;
        let command = tree::Command {
            command: "next".into(),
            label: ui::LocalizedText::new("Review next file", "审查下一文件"),
            arguments: serde_json::Value::Null,
            disabled: self.remaining == 0,
            separator_before: false,
        };
        presentation::publish_entries(
            resource,
            vec![presentation::Entry {
                id: "queue".into(),
                label: ui::LocalizedText::new(
                    format!(
                        "{} ({})",
                        if self.renamed {
                            "Review tasks"
                        } else {
                            "Review queue"
                        },
                        self.remaining
                    ),
                    format!("待审查 ({})", self.remaining),
                ),
                tooltip: ui::LocalizedText::new("Files awaiting review", "等待审查的文件"),
                icon: ui::ToolIcon::new("review.svg", "review.svg"),
                order: 200,
                disabled: false,
                busy: false,
                menu: vec![ui::ActionMenuItem::action("next", command)],
                on_open: None,
            }],
        )
    }
    /// Invalid publication is a recoverable typed command error and leaves accepted metadata intact.
    fn command(&mut self, id: &str) -> Result<(), api::Failure> {
        match id {
            "publish" => self.remaining = 1,
            "next" => self.remaining = 0,
            "rename" => self.renamed = true,
            "invalid" => {
                return presentation::publish_files(
                    self.resource.as_ref().unwrap().clone(),
                    vec![presentation::FileDecoration {
                        path: "../outside.txt".into(),
                        tone: tree::Tone::Negative,
                        tooltip: ui::LocalizedText::new("Invalid", "无效"),
                    }],
                );
            }
            _ => {
                return Err(api::Failure::new(
                    api::ErrorCode::InvalidRequest,
                    "Unknown review action",
                ));
            }
        }
        self.publish()
    }
}

/// The package publishes immutable review metadata; the host owns files, menus and lifetime gates.
struct ReviewQueue;
impl Guest for ReviewQueue {
    /// Publish review state after an explicit typed command, propagating ordinary host failures.
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| {
            match input {
                api::Input::Snapshot => {
                    return Ok(api::Output {
                        snapshot: Some(Default::default()),
                        ..Default::default()
                    });
                }
                api::Input::Activate => REVIEW.with_borrow_mut(|state| {
                    state.remaining = 1;
                    state.publish()
                })?,
                api::Input::Event {
                    event: api::Notification::CommandInvocation(invocation),
                    ..
                } => {
                    let result = REVIEW.with_borrow_mut(|state| state.command(&invocation.id));
                    plugin_protocol::commands::reply(
                        invocation.reply,
                        result.map(|_| serde_json::Value::Null),
                    )?;
                }
                _ => {}
            }
            Ok(api::Output::default())
        })
    }
}
export!(ReviewQueue);
