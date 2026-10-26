//! Independent public SDK regression for a native source beside a guest-owned vector canvas.
//! Only host Preview snapshots supply text; the fixture never reads or writes the backing file.
use plugin_protocol::{
    Paint, Rect, api,
    bindings::{Guest, export},
    ui::{self, Kind, Node},
};
use std::cell::RefCell;

/// Derived state has no text authority, editable buffer, undo history or persisted source snapshot.
#[derive(Default)]
struct State {
    source: Option<api::DocumentVersion>,
    text: String,
    caption: String,
    zoom: f32,
    revision: u64,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

struct CanvasFixture;
impl Guest for CanvasFixture {
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| {
            STATE.with(|state| {
                let mut state = state.borrow_mut();
                match input {
                    api::Input::Prepare { .. } => {
                        *state = State {
                            zoom: 1.,
                            ..Default::default()
                        };
                    }
                    api::Input::Snapshot => {
                        return Ok(api::Output {
                            snapshot: Some(Default::default()),
                            ..Default::default()
                        });
                    }
                    api::Input::Event { event, .. } => match event {
                        api::Notification::Preview { document, text } => {
                            state.source = document;
                            state.text = text;
                        }
                        api::Notification::Ui(event) => match event.action {
                            ui::Action::Click if event.node == "zoom" => state.zoom *= 1.25,
                            ui::Action::Change(value) if event.node == "caption" => {
                                state.caption = value
                            }
                            // Resize/focus callbacks do not mutate the derived drawing or its revision.
                            _ => return Ok(Default::default()),
                        },
                        _ => return Ok(Default::default()),
                    },
                    api::Input::Activate => {}
                }
                state.revision += 1;
                Ok(api::Output {
                    views: vec![api::View {
                        panel: "welcome".into(),
                        document: state.view(),
                    }],
                    ..Default::default()
                })
            })
        })
    }
}
export!(CanvasFixture);

impl State {
    /// A document Editor is valid only with the exact current source; no placeholder claims authority.
    fn view(&self) -> ui::Document {
        let Some(source) = &self.source else {
            return ui::Document::new(Node::text("pending", "Waiting for source"))
                .revision(self.revision);
        };
        let paint = if self.text.trim_start().starts_with("<svg") {
            vec![Paint::Svg {
                rect: Rect {
                    x: 0.,
                    y: 0.,
                    w: 160. * self.zoom,
                    h: 100. * self.zoom,
                },
                clip: Rect {
                    x: 0.,
                    y: 0.,
                    w: 800.,
                    h: 600.,
                },
                source: self.text.clone(),
            }]
        } else {
            vec![Paint::Text {
                x: 0.,
                y: 20.,
                text: self.text.clone(),
                color: 0x223344,
                size: 14.,
                bold: false,
                font: None,
            }]
        };
        let source_node = Node::new(
            "source",
            Kind::Editor(plugin_protocol::ui::Editor::document(source.clone())),
        )
        .grow();
        let preview = Node::column(
            "preview",
            vec![
                Node::row(
                    "toolbar",
                    vec![
                        Node::button("zoom", "放大"),
                        Node::input(
                            "caption",
                            ui::Input {
                                value: self.caption.clone(),
                                ..Default::default()
                            },
                        )
                        .grow(),
                    ],
                )
                .height(40.),
                Node::new(
                    "viewport",
                    Kind::Canvas(ui::Canvas {
                        paint,
                        ..Default::default()
                    }),
                )
                .grow(),
            ],
        )
        .grow();
        let mut document = ui::Document::new(Node::row("split", vec![source_node, preview]).grow())
            .revision(self.revision);
        document.editor_layout = true;
        document.source = Some(source.clone());
        document
    }
}
