//! Ordinary guest-owned timeline: 100000 logical events, only the retained visible neighborhood on the wire.
use plugin_protocol::{
    Rect, api,
    bindings::{Guest, export},
    ui::{self, Kind, Node},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{cell::RefCell, collections::BTreeMap};

/// Sparse domain changes and publication counters; the host owns neither timeline data nor undo.
#[derive(Deserialize, Serialize)]
#[serde(default)]
struct State {
    /// Outer document and drawing revisions advance independently of native presentation.
    revision: u64,
    scene_revision: u64,
    /// A replacement always takes a new monotonically allocated incarnation.
    identity: u64,
    /// Fixed deterministic dataset seed; values are generated only for visible logical indices.
    seed: u32,
    /// Sparse edits never allocate the full logical dataset.
    edits: BTreeMap<u32, u32>,
    /// Guest viewport policy, in absolute content coordinates.
    offset: (f32, f32),
    width: f32,
    height: f32,
    /// Visual zoom is separate from absolute content scrolling and OS DPI.
    scale: f32,
    /// Published overscan stays stable during native movement inside the retained rectangle.
    buffer: Rect,
    /// Playback requests are one-shot; no timer drives visual animation.
    animating: bool,
    frame_request: u64,
    frames: u64,
    playhead: f32,
    /// An explicit experiment switch compares equivalent full and delta publications.
    full_publication: bool,
    /// Business timer ownership is native-instance local and is never restored from a snapshot.
    #[serde(skip)]
    business_timer: Option<api::ResourceHandle>,
    business_ticks: u64,
}
impl Default for State {
    fn default() -> Self {
        Self {
            revision: 1,
            scene_revision: 1,
            identity: 401,
            seed: 1729,
            edits: BTreeMap::new(),
            offset: (0., 0.),
            width: 640.,
            height: 360.,
            scale: 1.,
            buffer: Rect {
                x: 0.,
                y: 0.,
                w: 800.,
                h: 520.,
            },
            animating: false,
            frame_request: 0,
            frames: 0,
            playhead: 0.,
            full_publication: false,
            business_timer: None,
            business_ticks: 0,
        }
    }
}
thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
/// The same component entry and public Output contract as every independent plugin.
struct Timeline;
impl Guest for Timeline {
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| {
            STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                match input {
                    api::Input::Prepare { snapshot, .. } => {
                        *state = snapshot
                            .filter(|snapshot| snapshot.schema == 1)
                            .and_then(|snapshot| serde_json::from_str(&snapshot.data).ok())
                            .unwrap_or_default();
                        state.animating = false;
                        Ok(Default::default())
                    }
                    api::Input::Activate => Ok(state.full(None)),
                    api::Input::Snapshot => Ok(api::Output {
                        snapshot: Some(plugin_protocol::Snapshot {
                            schema: 1,
                            data: serde_json::to_string(&*state).unwrap(),
                        }),
                        ..Default::default()
                    }),
                    api::Input::Event {
                        event: api::Notification::CanvasFrame { opportunity, .. },
                        ..
                    } => state.frame(opportunity),
                    api::Input::Event {
                        event: api::Notification::Timer { .. },
                        ..
                    } => {
                        state.business_ticks += 1;
                        Ok(Default::default())
                    }
                    api::Input::Event {
                        event: api::Notification::Ui(event),
                        ..
                    } => state.ui(event),
                    api::Input::Event {
                        event: api::Notification::Command { id, arguments, .. },
                        ..
                    } => state.command(&id, arguments),
                    // Native receipts and incidental observations cannot create an idle publication loop.
                    _ => Ok(Default::default()),
                }
            })
        })
    }
}
export!(Timeline);
impl State {
    /// Enumerate only the viewport plus bounded overscan, not all 100000 logical events.
    fn visible(&self) -> Vec<u32> {
        let x0 = self.buffer.x;
        let y0 = self.buffer.y;
        let x1 = x0 + self.buffer.w;
        let y1 = y0 + self.buffer.h;
        let mut result = Vec::new();
        for row in (y0 / 24.).floor() as u32..((y1 / 24.).ceil() as u32).min(1000) {
            for col in (x0 / 120.).floor() as u32..((x1 / 120.).ceil() as u32).min(100) {
                result.push(row * 100 + col);
            }
        }
        result
    }
    /// Stable scene identity and geometry derive from the logical item, independently of serialization order.
    fn event(&self, index: u32) -> ui::SceneNode {
        let value = self
            .edits
            .get(&index)
            .copied()
            .unwrap_or(self.seed.wrapping_add(index.wrapping_mul(2654435761)));
        let rgba = if value % 2 == 0 {
            0x31aa66ffu32
        } else {
            0x386fe8ffu32
        };
        serde_json::from_value(json!({"kind":"shape","id":format!("event-{index}"),
            "geometry":{"kind":"rect","rect":{"x":(index%100) as f32*120.,"y":(index/100) as f32*24.,"w":100,"h":16}},
            "fill":{"kind":"solid","rgba":rgba}})).unwrap()
    }
    /// Full publication is reserved for first display, explicit resynchronization and incarnation changes.
    fn full(&self, override_identity: Option<u64>) -> api::Output {
        let visible = self.visible();
        let mut children = Vec::new();
        for index in &visible {
            children.push(self.event(*index));
            if index % 100 % 4 == 0 {
                children.push(serde_json::from_value(json!({"kind":"paragraph","id":format!("label-{index}"),
                    "layout_key":format!("label-{index}"),"text":format!("事件 {index} / Event"),
                    "rect":{"x":(index%100) as f32*120.+3.,"y":(index/100) as f32*24.,"w":94,"h":16},
                    "style":{"font":{"size_px":11},"wrap":false}})).unwrap());
            }
        }
        let mut semantics=vec![serde_json::from_value(json!({"id":"timeline-grid","role":"grid",
            "name":{"en":"Timeline events","zh_cn":"时间轴事件"},"value":"100000 logical events",
            "rect":{"x":0,"y":0,"w":12000,"h":24000},"row_count":1000,"column_count":100,"read_only":true})).unwrap()];
        let rows = visible
            .iter()
            .map(|index| index / 100)
            .collect::<std::collections::BTreeSet<_>>();
        for row in rows {
            semantics.push(serde_json::from_value(json!({"id":format!("lane-{row}"),"parent":"timeline-grid","role":"row",
                "name":{"en":format!("Lane {}",row+1),"zh_cn":format!("第{}轨道",row+1)},"row":row+1,"read_only":true,
                "rect":{"x":0,"y":row as f32*24.,"w":12000,"h":16}})).unwrap());
        }
        let scene = ui::CanvasScene {
            identity: override_identity.unwrap_or(self.identity),
            revision: self.scene_revision,
            roots: vec![
                serde_json::from_value(json!({"kind":"group","id":"events","children":children}))
                    .unwrap(),
                self.playhead_node(),
            ],
            targets: Vec::new(),
            semantics,
        };
        let mut document = ui::Document::new(
            Node::new(
                "drawing",
                Kind::Canvas(ui::Canvas {
                    scene: Some(scene),
                    scroll_2d: Some(ui::SceneScroll2D {
                        content: ui::SceneExtent {
                            width: 12000.,
                            height: 24000.,
                        },
                        offset: ui::SceneOffset {
                            x: self.offset.0,
                            y: self.offset.1,
                        },
                        buffer_rect: self.buffer,
                        epoch: 1,
                    }),
                    ..Default::default()
                }),
            )
            .grow(),
        )
        .revision(self.revision);
        document.root.viewport = Some(ui::VisualViewport {
            content: Some(ui::ContentSize {
                width: 12000.,
                height: 24000.,
            }),
            transform: Some(ui::ContentTransform {
                scale: self.scale,
                x: 0.,
                y: 0.,
                anchor_x: 0.,
                anchor_y: 0.,
            }),
        });
        api::Output {
            views: vec![api::View {
                panel: "timeline".into(),
                document,
            }],
            ..Default::default()
        }
    }
    /// Domain commands also expose controlled invalid publications for public Manager recovery tests.
    fn command(
        &mut self,
        id: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<api::Output, api::Failure> {
        match id {
            "publication-mode" => {
                self.full_publication =
                    arguments.as_ref().and_then(|args| args["mode"].as_str()) == Some("full");
                Ok(Default::default())
            }
            "zoom-in" | "zoom-out" => {
                self.scale = (self.scale * if id == "zoom-in" { 1.25 } else { 0.8 }).clamp(0.5, 4.);
                self.refresh_buffer();
                self.revision += 1;
                self.scene_revision += 1;
                Ok(self.full(None))
            }
            "animate" => {
                self.animating = true;
                Ok(self.next_frame(Default::default()))
            }
            "stop" => {
                self.animating = false;
                Ok(Default::default())
            }
            "request-again" | "request-conflict" => Ok(api::Output {
                canvas_frames: vec![ui::CanvasFrameRequest {
                    panel: "timeline".into(),
                    canvas: "drawing".into(),
                    identity: self.identity,
                    request: self.frame_request + u64::from(id == "request-conflict"),
                }],
                ..Default::default()
            }),
            "business-timer" => {
                if self.business_timer.is_none() {
                    self.business_timer = Some(plugin_protocol::timers::start(20, true)?);
                }
                Ok(Default::default())
            }
            "business-stop" => {
                if let Some(handle) = self.business_timer.take() {
                    api::guest::close_resource(handle)?;
                }
                Ok(Default::default())
            }
            "set-offset" => {
                let args = arguments.unwrap_or_default();
                self.offset = (
                    args["x"].as_f64().unwrap_or(0.) as f32,
                    args["y"].as_f64().unwrap_or(0.) as f32,
                );
                self.refresh_buffer();
                self.revision += 1;
                self.scene_revision += 1;
                Ok(self.full(None))
            }
            "mutate" | "stale-patch" | "invalid-patch" => {
                let index = arguments
                    .as_ref()
                    .and_then(|value| value.get("index"))
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0) as u32;
                if index >= 100000 {
                    return Err(api::Failure::new(
                        api::ErrorCode::InvalidRequest,
                        "Unknown timeline event",
                    ));
                }
                let old = self
                    .edits
                    .get(&index)
                    .copied()
                    .unwrap_or(self.seed.wrapping_add(index.wrapping_mul(2654435761)));
                self.edits.insert(index, old.wrapping_add(1));
                if !self.visible().contains(&index) {
                    return Ok(Default::default());
                }
                let base_ui_revision = self.revision;
                let base_revision = self.scene_revision;
                self.revision += 1;
                self.scene_revision += 1;
                let mut node = self.event(index);
                if id == "invalid-patch" {
                    node = serde_json::from_value(json!({"kind":"shape","id":"absent",
                    "geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":1,"h":1}}}))
                    .unwrap();
                }
                Ok(api::Output {
                    scene_patches: vec![ui::CanvasScenePatch {
                        panel: "timeline".into(),
                        canvas: "drawing".into(),
                        base_ui_revision,
                        next_ui_revision: self.revision,
                        identity: self.identity,
                        base_revision: if id == "stale-patch" {
                            base_revision.saturating_sub(1)
                        } else {
                            base_revision
                        },
                        next_revision: self.scene_revision,
                        operations: vec![ui::CanvasSceneOperation::Replace { node }],
                        damage: Some(Rect {
                            x: 0.,
                            y: 0.,
                            w: 1.,
                            h: 1.,
                        }),
                    }],
                    ..Default::default()
                })
            }
            "resync" => {
                self.revision += 1;
                self.scene_revision += 1;
                Ok(self.full(None))
            }
            "new-scene" => {
                self.identity += 1;
                self.revision += 1;
                self.scene_revision = 1;
                Ok(self.full(None))
            }
            "aba" => {
                self.revision += 1;
                self.scene_revision += 1;
                Ok(self.full(Some(401)))
            }
            _ => Err(api::Failure::new(
                api::ErrorCode::InvalidRequest,
                "Unknown timeline command",
            )),
        }
    }
}

impl State {
    /// One tiny moving item exercises true retained publication without serializing logical events.
    fn playhead_node(&self) -> ui::SceneNode {
        serde_json::from_value(json!({"kind":"shape","id":"playhead","geometry":{"kind":"rect",
            "rect":{"x":self.playhead,"y":0,"w":2,"h":24000}},"fill":{"kind":"solid","rgba":0xee4433ffu32}})).unwrap()
    }
    /// Allocate only one next opportunity; the host checks pending ownership and monotonically new tokens.
    fn next_frame(&mut self, mut output: api::Output) -> api::Output {
        self.frame_request += 1;
        output.canvas_frames.push(ui::CanvasFrameRequest {
            panel: "timeline".into(),
            canvas: "drawing".into(),
            identity: self.identity,
            request: self.frame_request,
        });
        output
    }
    /// A native opportunity advances playback; hidden or stopped playback cannot create another demand.
    fn frame(
        &mut self,
        opportunity: ui::CanvasFrameOpportunity,
    ) -> Result<api::Output, api::Failure> {
        if !self.animating
            || opportunity.identity != self.identity
            || opportunity.request != self.frame_request
            || !opportunity.visible
        {
            return Ok(Default::default());
        }
        self.frames += 1;
        // Keep the playback indicator in the actually visible content region during both-axis motion.
        let visible_width = opportunity.viewport.w.max(1.);
        self.playhead = opportunity.viewport.x
            + (self.playhead - opportunity.viewport.x
                + (opportunity.interval_ms.max(1.) * 0.06).min(16.))
            .rem_euclid(visible_width);
        let base_ui_revision = self.revision;
        let base_revision = self.scene_revision;
        self.revision += 1;
        self.scene_revision += 1;
        let output = if self.full_publication {
            self.full(None)
        } else {
            api::Output {
                scene_patches: vec![ui::CanvasScenePatch {
                    panel: "timeline".into(),
                    canvas: "drawing".into(),
                    base_ui_revision,
                    next_ui_revision: self.revision,
                    identity: self.identity,
                    base_revision,
                    next_revision: self.scene_revision,
                    operations: vec![ui::CanvasSceneOperation::Replace {
                        node: self.playhead_node(),
                    }],
                    damage: None,
                }],
                ..Default::default()
            }
        };
        Ok(self.next_frame(output))
    }
    /// Refill only when the requested viewport leaves the old painted rectangle.
    fn refresh_buffer(&mut self) {
        let width = self.width / self.scale;
        let height = self.height / self.scale;
        self.offset.0 = self.offset.0.clamp(0., (12000. - width).max(0.));
        self.offset.1 = self.offset.1.clamp(0., (24000. - height).max(0.));
        let x = (self.offset.0 - 160.).max(0.);
        let y = (self.offset.1 - 160.).max(0.);
        self.buffer = Rect {
            x,
            y,
            w: (self.offset.0 + width + 160.).min(12000.) - x,
            h: (self.offset.1 + height + 160.).min(24000.) - y,
        };
    }
    /// Native scroll reports requested absolute content coordinates; no movement invents missing events.
    fn ui(&mut self, event: ui::UiEvent) -> Result<api::Output, api::Failure> {
        let event = match event.action {
            ui::Action::Canvas(event) => event,
            ui::Action::ViewportInput(input) => input.event,
            _ => return Ok(Default::default()),
        };
        match event {
            ui::CanvasEvent::Resize { width, height, .. }
                if width > 0.
                    && height > 0.
                    && ((self.width - width).abs() > 0.1 || (self.height - height).abs() > 0.1) =>
            {
                self.width = width;
                self.height = height;
            }
            ui::CanvasEvent::Scroll2D {
                epoch: 1, offset, ..
            } => {
                self.offset = (offset.x, offset.y);
                if offset.x >= self.buffer.x
                    && offset.y >= self.buffer.y
                    && offset.x + self.width / self.scale <= self.buffer.x + self.buffer.w
                    && offset.y + self.height / self.scale <= self.buffer.y + self.buffer.h
                {
                    return Ok(Default::default());
                }
            }
            _ => return Ok(Default::default()),
        }
        self.refresh_buffer();
        self.revision += 1;
        self.scene_revision += 1;
        Ok(self.full(None))
    }
}
