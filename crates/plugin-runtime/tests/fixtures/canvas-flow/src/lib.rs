//! Independent flow diagram uses only public scene, viewport and native node contracts.
use plugin_protocol::{
    Rect, api,
    bindings::{Guest, export},
    ui::{self, Kind, Node},
};
use std::cell::RefCell;
mod publication;

/// Domain selection and viewport policy belong to the guest, never to the host renderer.
struct State {
    /// Immutable publication bytes are a wire base, never another editable model or undo stack.
    published: Option<ui::Document>,
    /// Ordinary native property editing is guest-owned committed state, separate from scene text.
    property: String,
    property_revision: u64,
    /// Public request/notification observations; no private host state is inspected by the guest.
    exchange: String,
    /// One observable OS result; source cancellation never mutates destination/business permissions.
    drag_result: String,
    /// Existing document popup owns input only while the guest keeps this policy enabled.
    menu: bool,
    /// Optional nested native Canvas exercises ancestor ownership through the ordinary UI protocol.
    nested: bool,
    /// Deliberate oversized PNG exercises native decoder admission without allocating guest pixels.
    bad_source_image: bool,
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
            published: None,
            property: "节点属性 / Node property".into(),
            property_revision: 0,
            exchange: String::new(),
            drag_result: "idle".into(),
            menu: false,
            nested: false,
            bad_source_image: false,
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
        if let Some(completion) = publication::budget_completion(&payload) {
            STATE.with(|state| state.borrow_mut().published = None);
            return completion;
        }
        api::guest::dispatch(&payload, |input| {
            STATE.with(|state| {
                let mut state = state.borrow_mut();
                // Deliberate invalid publications must reach host admission, preserving its recoverable
                // scene rejection contract instead of becoming a guest callback failure.
                let explicit_full=matches!(&input,api::Input::Event{event:api::Notification::Ui(event),..}
                    if ["rewind","bad-ack","restore","restore-target","bad-source","recover"].contains(&event.node.as_str()));
                let retired_publication=matches!(&input,api::Input::Event{event:api::Notification::Ui(event),..}
                    if ["restore","restore-target"].contains(&event.node.as_str()));
                match input {
                    api::Input::Prepare { .. } => *state = State::default(),
                    api::Input::Activate => {}
                    api::Input::Event {
                        event: api::Notification::Command { id, arguments, .. }, ..
                    } if id=="publication-probe" => {
                        // Public wire faults exercise host transaction admission through a real component.
                        // An unconfirmed injected publication can never become our next diff baseline.
                        state.published=None;
                        return publication::probe(arguments.unwrap_or_default());
                    }
                    api::Input::Event {
                        event: api::Notification::Command { id, arguments, .. },
                        ..
                    } if id == "exchange-probe" => {
                        let operation = serde_json::from_value(arguments.unwrap_or_default())
                            .map_err(|error| {
                                api::Failure::new(api::ErrorCode::InvalidRequest, error.to_string())
                            })?;
                        state.exchange =
                            serde_json::to_string(&api::guest::request(operation)).unwrap();
                    }
                    api::Input::Event {
                        event: api::Notification::Request { update, .. },
                        ..
                    } => {
                        state.exchange = serde_json::to_string(&update).unwrap();
                    }
                    api::Input::Event {
                        event: api::Notification::Transfer(event),
                        ..
                    } => {
                        state.exchange = serde_json::to_string(&event).unwrap();
                    }
                    api::Input::Event {
                        event: api::Notification::DragFinished(result),
                        ..
                    } => {
                        state.drag_result = match result.outcome {
                            plugin_protocol::transfer::DragOutcome::Dropped => "dropped",
                            plugin_protocol::transfer::DragOutcome::Cancelled => "cancelled",
                        }
                        .into();
                    }
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
                        ui::Action::Click if event.node == "zoom" => {
                            state.transform.scale = (state.transform.scale * 1.25).min(4.)
                        }
                        ui::Action::Change(value) if event.node == "native-property" => {
                            state.property = value;
                            state.property_revision += 1;
                        }
                        ui::Action::Click if event.node == "show-menu" => state.menu = true,
                        ui::Action::Click if event.node == "nested" => state.nested = !state.nested,
                        ui::Action::Click if event.node == "bad-source" => {
                            state.bad_source_image = true
                        }
                        ui::Action::Dismiss | ui::Action::Select(_)
                            if event.node == "scene-menu" =>
                        {
                            state.menu = false
                        }
                        ui::Action::Click if event.node == "pan" => state.transform.x += 20.,
                        ui::Action::Click if event.node == "rewind" => {
                            state.scene_override = Some(1)
                        }
                        ui::Action::Click if event.node == "recover" => {
                            state.scene_override = None;
                            state.bad_ack = false;
                            state.published = None;
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
                let next=state.view();
                let unconfirmed=state.scene_override.is_some() || state.bad_ack || state.bad_source_image
                    || retired_publication;
                if explicit_full || unconfirmed {
                    state.published=None;
                    // Recovery itself is valid, so the next local edit may use its newly sent base.
                    if !unconfirmed {state.published=Some(next.clone());}
                    return Ok(api::Output{views:vec![api::View{panel:"diagram".into(),document:next}],..Default::default()});
                }
                let publication=ui::retained_publication("diagram",state.published.as_ref(),next)?;
                state.published=Some(publication.document);
                Ok(publication.output)
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
                "drop_target":{"preferred":["text/html","text/plain;charset=utf-8"]},
                "drag_source":{"items":[{"mime":"text/plain;charset=utf-8","bytes":b"Flow\tnode".to_vec()},
                    {"mime":"text/html","bytes":b"<table><tr><td>Flow</td></tr></table>".to_vec()}]},
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
        let nested = if self.nested && self.target_present {
            let mut nested_scene = scene.clone();
            nested_scene.identity = 2001;
            nested_scene.targets[0].epoch = 1;
            if let Some(binding) = &mut nested_scene.targets[0].text_input {
                binding.acknowledged_sequence = 0;
            }
            Some(ui::CanvasOverlay {
                id: "nested-scene".into(),
                anchor: self.anchor(),
                focus_return: None,
                rect: Rect {
                    x: 0.,
                    y: 130.,
                    w: 300.,
                    h: 200.,
                },
                child: Box::new(Node::new(
                    "nested-drawing",
                    Kind::Canvas(ui::Canvas {
                        scene: Some(nested_scene),
                        focusable: true,
                        ..Default::default()
                    }),
                )),
            })
        } else {
            None
        };
        let mut canvas = Node::new(
            "drawing",
            Kind::Canvas(ui::Canvas {
                overlays: if self.target_present {
                    vec![ui::CanvasOverlay {
                        id: "properties".into(),
                        anchor: self.anchor(),
                        focus_return: Some(ui::CanvasTargetRef {
                            id: "start".into(),
                            epoch: self.target_epoch,
                        }),
                        rect: Rect {
                            x: 20.,
                            y: 90.,
                            w: 190.,
                            h: 32.,
                        },
                        child: Box::new(Node::input(
                            "native-property",
                            ui::Input {
                                value: self.property.clone(),
                                value_revision: self.property_revision,
                                placeholder: "节点属性 / Node property".into(),
                                ..Default::default()
                            },
                        )),
                    }]
                } else {
                    Vec::new()
                },
                scene: Some(scene),
                focusable: true,
                ..Default::default()
            }),
        )
        .grow();
        if let Kind::Canvas(drawing) = &mut canvas.kind {
            drawing.overlays.extend(nested);
        }
        canvas.viewport = Some(ui::VisualViewport {
            content: Some(ui::ContentSize {
                width: 320.,
                height: 200.,
            }),
            transform: Some(self.transform),
        });
        if self.target_present {
            canvas.tooltip = Some("开始节点 / Start node".into());
            canvas.tooltip_anchor = Some(ui::CanvasNodeAnchor {
                canvas: "drawing".into(),
                anchor: self.anchor(),
            });
        }
        let mut drop_zone = Node::button("drop-zone", "拖入文字或图片 / Drop text or image");
        drop_zone.drop_target = Some(plugin_protocol::transfer::DropTarget {
            preferred: vec![
                plugin_protocol::transfer::FILES_MIME.into(),
                "image/png".into(),
                "text/html".into(),
                "text/plain;charset=utf-8".into(),
            ],
        });
        let mut document = ui::Document::new(
            // Ordinary source/target declarations use the same host port as Canvas hit targets.
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
                            Node::button("show-menu", "菜单 / Menu"),
                            Node::button("nested", "嵌套场景 / Nested scene"),
                            Node::button("bad-source", "超额图片 / Oversized image"),
                        ],
                    ),
                    Node::text("selection", if self.selected { "start" } else { "none" }),
                    Node::text("exchange", self.exchange.clone()),
                    drop_zone,
                    {
                        let mut source = Node::button("drag-source", "拖出节点 / Drag node out");
                        source.drag_source = Some(plugin_protocol::transfer::DragSource {
                            items: vec![
                                plugin_protocol::transfer::Item {
                                    mime: "text/plain;charset=utf-8".into(),
                                    bytes: b"Flow\tnode".to_vec(),
                                },
                                plugin_protocol::transfer::Item {
                                    mime: "text/html".into(),
                                    bytes: b"<table><tr><td>Flow</td></tr></table>".to_vec(),
                                },
                                plugin_protocol::transfer::Item {
                                    mime: "application/vnd.nanobug.canvas-flow.node".into(),
                                    bytes: b"flow-node-a".to_vec(),
                                },
                                plugin_protocol::transfer::Item {
                                    mime: "image/png".into(),
                                    bytes: if self.bad_source_image {
                                        vec![
                                            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72,
                                            68, 82, 0, 1, 0, 0, 0, 1, 0, 0, 8, 6, 0, 0, 0, 108,
                                            132, 48, 227, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156,
                                            99, 48, 158, 249, 255, 63, 0, 5, 153, 2, 203, 20, 229,
                                            63, 253, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
                                        ]
                                    } else {
                                        vec![
                                            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72,
                                            68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 31, 21,
                                            196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99,
                                            48, 158, 249, 255, 63, 0, 5, 153, 2, 203, 20, 229, 63,
                                            253, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
                                        ]
                                    },
                                },
                            ],
                        });
                        source
                    },
                    Node::text("drag-result", self.drag_result.clone()),
                    canvas,
                ],
            )
            .grow(),
        )
        .revision(self.revision);
        if self.menu {
            document.menu = Some(ui::PopupMenu {
                id: "scene-menu".into(),
                x: 0.,
                y: 0.,
                anchor: Some(ui::CanvasNodeAnchor {
                    canvas: "drawing".into(),
                    anchor: self.anchor(),
                }),
                items: vec![ui::MenuItem {
                    id: "inspect".into(),
                    label: "节点属性 / Node property".into(),
                    disabled: false,
                    separator_before: false,
                }],
            });
        }
        document
    }

    /// The guest declares a stable target lifetime; native projection is entirely host-owned.
    fn anchor(&self) -> ui::CanvasAnchor {
        ui::CanvasAnchor {
            identity: self.identity,
            scene_revision: 1,
            target: ui::CanvasTargetRef {
                id: "start".into(),
                epoch: self.target_epoch,
            },
            space: None,
            rect: Rect {
                x: 20.,
                y: 20.,
                w: 100.,
                h: 60.,
            },
        }
    }
}
