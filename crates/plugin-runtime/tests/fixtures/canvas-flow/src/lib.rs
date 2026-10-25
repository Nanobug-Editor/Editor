//! Independent flow diagram uses only public scene, viewport and native node contracts.
use plugin_protocol::{
    Rect, api,
    bindings::{Guest, export},
    ui::{self, Kind, Node},
};
use std::cell::RefCell;

/// Domain selection and viewport policy belong to the guest, never to the host renderer.
struct State {
    /// Domain text and undo live entirely in this ordinary guest.
    text: String,
    undo: Vec<String>,
    text_revision: u64,
    acknowledged: u64,
    target_epoch: u64,
    target_present: bool,
    /// Explicitly hold acknowledgements to exercise the bounded native predecessor chain.
    hold: bool,
    /// Deliberate malformed ack tests host atomic admission, then ordinary recover clears it.
    bad_ack: bool,
    /// Outer UI revision advances independently of deliberately malformed acceptance publications.
    revision: u64,
    /// Domain scene identity changes only on an explicit replacement test.
    identity: u64,
    /// An intentional stale scene revision exercises atomic rejection through the public manager.
    scene_override: Option<u64>,
    /// Domain selection remains guest-owned.
    selected: bool,
    /// Guest-owned zoom and pan policy, expressed in viewport logical pixels.
    transform: ui::ContentTransform,
}
impl Default for State {
    fn default() -> Self {
        Self {
            text: "e\u{301} 👩‍💻".into(),
            undo: Vec::new(),
            text_revision: 1,
            acknowledged: 0,
            target_epoch: 1,
            target_present: true,
            hold: false,
            bad_ack: false,
            revision: 1,
            identity: 1,
            scene_override: None,
            selected: false,
            transform: ui::ContentTransform {
                anchor_x: 0.,
                anchor_y: 0.,
                ..Default::default()
            },
        }
    }
}
thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
/// The fixture publishes only after an actual domain or viewport change.
struct Flow;
impl Guest for Flow {
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| {
            STATE.with(|state| {
                let mut state = state.borrow_mut();
                match input {
                    api::Input::Prepare { .. } => *state = State::default(),
                    api::Input::Activate => {}
                    api::Input::Snapshot => {
                        return Ok(api::Output {
                            snapshot: Some(Default::default()),
                            ..Default::default()
                        });
                    }
                    api::Input::Event {
                        event: api::Notification::Ui(event),
                        ..
                    } => match event.action {
                        ui::Action::Click if event.node == "zoom" => state.transform.scale *= 1.25,
                        ui::Action::Click if event.node == "pan" => state.transform.x += 20.,
                        ui::Action::Click if event.node == "rewind" => {
                            state.scene_override = Some(1)
                        }
                        ui::Action::Click if event.node == "recover" => {
                            state.scene_override = None;
                            state.bad_ack = false;
                        }
                        ui::Action::Click if event.node == "hold" => {
                            state.hold = true;
                        }
                        ui::Action::Click if event.node == "ack" => {
                            state.hold = false;
                        }
                        ui::Action::Click if event.node == "bad-ack" => {
                            state.bad_ack = true;
                        }
                        ui::Action::Click if event.node == "remove-target" => {
                            state.target_present = false;
                        }
                        ui::Action::Click if event.node == "restore-target" => {
                            state.target_present = true;
                        }
                        ui::Action::Click if event.node == "fresh-target" => {
                            state.target_present = true;
                            state.target_epoch += 1;
                            state.acknowledged = 0;
                        }
                        ui::Action::Click if event.node == "undo" => {
                            if let Some(text) = state.undo.pop() {
                                state.text = text;
                                state.text_revision += 1;
                                state.target_epoch += 1;
                                state.acknowledged = 0;
                            }
                        }
                        ui::Action::Canvas(ui::CanvasEvent::Input { input }) => match input.kind {
                            ui::CanvasInputKind::Text(edit) => {
                                edit.range.to_utf16(&state.text).map_err(|error| {
                                    api::Failure::new(api::ErrorCode::InvalidRequest, error)
                                })?;
                                let before = state.text.clone();
                                state.undo.push(before);
                                state
                                    .text
                                    .replace_range(edit.range.start..edit.range.end, &edit.text);
                                state.text_revision += 1;
                                state.acknowledged = input.sequence;
                                if state.hold {
                                    return Ok(Default::default());
                                }
                            }
                            ui::CanvasInputKind::A11y {
                                action: ui::CanvasSemanticAction::SetValue,
                                value: Some(value),
                                ..
                            } => {
                                let before = state.text.clone();
                                state.undo.push(before);
                                state.text = value;
                                state.text_revision += 1;
                                state.target_epoch += 1;
                                state.acknowledged = 0;
                            }
                            ui::CanvasInputKind::Pointer(pointer)
                                if pointer.phase == ui::CanvasPointerPhase::Down =>
                            {
                                state.selected = true
                            }
                            _ => return Ok(Default::default()),
                        },
                        ui::Action::Click if event.node == "replace" => state.identity = 2,
                        ui::Action::Click if event.node == "restore" => state.identity = 1,
                        ui::Action::ViewportInput(ui::ViewportInput {
                            event:
                                ui::CanvasEvent::Pointer {
                                    phase: ui::PointerPhase::Down,
                                    x,
                                    y,
                                    ..
                                },
                            ..
                        }) => {
                            let x = (x - state.transform.x) / state.transform.scale;
                            let y = (y - state.transform.y) / state.transform.scale;
                            state.selected =
                                (20. ..=120.).contains(&x) && (20. ..=80.).contains(&y);
                        }
                        _ => return Ok(Default::default()),
                    },
                    _ => return Ok(Default::default()),
                }
                state.revision += 1;
                Ok(api::Output {
                    views: vec![api::View {
                        panel: "diagram".into(),
                        document: state.view(),
                    }],
                    ..Default::default()
                })
            })
        })
    }
}
export!(Flow);
impl State {
    /// Shapes retain stable IDs while selection changes only the selected node's outline.
    fn view(&self) -> ui::Document {
        let shape = |id: &str, x, color| ui::SceneNode::Shape {
            id: id.into(),
            geometry: ui::SceneGeometry::Rect {
                rect: Rect {
                    x,
                    y: 20.,
                    w: 100.,
                    h: 60.,
                },
                radii: [12.; 4],
            },
            fill: Some(ui::SceneBrush::Solid { rgba: color }),
            stroke: (self.selected && id == "start").then(|| ui::SceneStroke {
                brush: ui::SceneBrush::Solid { rgba: 0x000000ff },
                width: 3.,
                cap: Default::default(),
                join: Default::default(),
                miter_limit: 4.,
                dash: vec![],
            }),
        };
        let scene = ui::CanvasScene {
            semantics: if self.target_present {
                vec![serde_json::from_value(serde_json::json!({
                "id":"start-name","role":"text_input","name":{"en":"Start node text","zh_cn":"开始节点文字"},
                "target":"start","value":self.text,"actions":["focus","activate","set_value"]})).unwrap()]
            } else {
                Vec::new()
            },
            targets: if self.target_present {
                vec![serde_json::from_value(serde_json::json!({
                "id":"start","epoch":self.target_epoch,"focusable":true,"tab_index":0,"cursor":"text",
                "geometry":{"kind":"rect","rect":{"x":20,"y":20,"w":100,"h":60}},
                "text_input":{"paragraph":"start-text","layout_key":format!("start-{}",self.text_revision),
                    "text_revision":self.text_revision,"acknowledged_sequence":if self.bad_ack {self.acknowledged+100}else{self.acknowledged},
                    "selection":{"start":self.text.len(),"end":self.text.len()},"editing_keys":true}
            })).unwrap()]
            } else {
                Vec::new()
            },
            identity: self.identity,
            revision: self.scene_override.unwrap_or(self.revision),
            roots: vec![
                shape("start", 20., 0x3399ffff),
                shape("finish", 200., 0x66cc88ff),
                ui::SceneNode::Paragraph {
                    id: "start-text".into(),
                    layout_key: format!("start-{}", self.text_revision),
                    text: self.text.clone(),
                    rect: Rect {
                        x: 24.,
                        y: 25.,
                        w: 92.,
                        h: 50.,
                    },
                    style: ui::ParagraphStyle::default(),
                },
                ui::SceneNode::Shape {
                    id: "edge".into(),
                    geometry: ui::SceneGeometry::Path {
                        segments: vec![
                            ui::ScenePathSegment::Move { x: 120., y: 50. },
                            ui::ScenePathSegment::Cubic {
                                x1: 150.,
                                y1: 0.,
                                x2: 170.,
                                y2: 100.,
                                x: 200.,
                                y: 50.,
                            },
                        ],
                        rule: Default::default(),
                    },
                    fill: None,
                    stroke: Some(ui::SceneStroke {
                        brush: ui::SceneBrush::Solid { rgba: 0x445566ff },
                        width: 2.,
                        cap: ui::SceneLineCap::Round,
                        join: Default::default(),
                        miter_limit: 4.,
                        dash: vec![5., 3.],
                    }),
                },
            ],
        };
        let mut canvas = Node::new(
            "drawing",
            Kind::Canvas(ui::Canvas {
                scene: Some(scene),
                focusable: true,
                ..Default::default()
            }),
        )
        .grow();
        canvas.viewport = Some(ui::VisualViewport {
            content: Some(ui::ContentSize {
                width: 320.,
                height: 200.,
            }),
            transform: Some(self.transform),
        });
        ui::Document::new(
            Node::column(
                "root",
                vec![
                    Node::row(
                        "tools",
                        vec![
                            Node::button("zoom", "放大 / Zoom"),
                            Node::button("pan", "平移 / Pan"),
                            Node::button("rewind", "Stale revision"),
                            Node::button("recover", "Recover"),
                            Node::button("replace", "Replace"),
                            Node::button("restore", "Retired identity"),
                            Node::button("hold", "暂停确认 / Hold"),
                            Node::button("ack", "确认 / Ack"),
                            Node::button("bad-ack", "错误确认 / Bad ack"),
                            Node::button("remove-target", "移除 / Remove"),
                            Node::button("restore-target", "旧目标 / Old target"),
                            Node::button("fresh-target", "新目标 / Fresh target"),
                            Node::button("undo", "撤销 / Undo"),
                        ],
                    ),
                    Node::text("selection", if self.selected { "start" } else { "none" }),
                    canvas,
                ],
            )
            .grow(),
        )
        .revision(self.revision)
    }
}
