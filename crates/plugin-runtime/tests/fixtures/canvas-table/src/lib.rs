//! An ordinary table guest owns domain cells/undo and uses public native focus, IME and semantics.
use plugin_protocol::{
    api,
    bindings::{Guest, export},
    ui,
};
use serde_json::json;
use std::cell::RefCell;

/// Visible cells are a bounded projection of a logical grid, never a host-specific table service.
struct Table {
    revision: u64,
    cells: Vec<String>,
    text_revisions: Vec<u64>,
    epochs: Vec<u64>,
    acknowledgements: Vec<u64>,
    selected: usize,
    /// Undo is exclusively guest business state; native drafts do not own this stack.
    undo: Vec<(usize, String)>,
}
impl Default for Table {
    fn default() -> Self {
        Self {
            revision: 1,
            cells: [
                "姓名 / Name",
                "e\u{301} 👩‍💻",
                "数量 / Count",
                "42",
                "备注 / Notes",
                "中文",
            ]
            .map(String::from)
            .to_vec(),
            text_revisions: vec![1; 6],
            epochs: vec![1; 6],
            acknowledgements: vec![0; 6],
            selected: 0,
            undo: Vec::new(),
        }
    }
}
thread_local! {static TABLE:RefCell<Table>=RefCell::new(Table::default());}
/// All behavior enters through the public guest dispatch contract, with no native business API.
struct Plugin;
impl Guest for Plugin {
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| {
            TABLE.with(|state| {
                let mut state = state.borrow_mut();
                match input {
                    api::Input::Prepare { .. } => *state = Table::default(),
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
                        ui::Action::Click if event.node == "undo" => {
                            state.undo();
                        }
                        ui::Action::Canvas(ui::CanvasEvent::Input { input }) => {
                            let Some(index) = input
                                .target
                                .strip_prefix("cell-")
                                .and_then(|id| id.parse::<usize>().ok())
                                .filter(|index| *index < state.cells.len())
                            else {
                                return Ok(Default::default());
                            };
                            match input.kind {
                                ui::CanvasInputKind::Text(edit) => {
                                    edit.range.to_utf16(&state.cells[index]).map_err(
                                        |message| {
                                            api::Failure::new(
                                                api::ErrorCode::InvalidRequest,
                                                message,
                                            )
                                        },
                                    )?;
                                    let before = state.cells[index].clone();
                                    state.undo.push((index, before));
                                    state.cells[index].replace_range(
                                        edit.range.start..edit.range.end,
                                        &edit.text,
                                    );
                                    state.text_revisions[index] += 1;
                                    state.acknowledgements[index] = input.sequence;
                                }
                                ui::CanvasInputKind::Focus { focused: true } => {
                                    state.selected = index
                                }
                                ui::CanvasInputKind::A11y {
                                    action: ui::CanvasSemanticAction::SetValue,
                                    value: Some(value),
                                    ..
                                } => {
                                    let before = state.cells[index].clone();
                                    state.undo.push((index, before));
                                    state.cells[index] = value;
                                    state.text_revisions[index] += 1;
                                    state.epochs[index] += 1;
                                    state.acknowledgements[index] = 0;
                                }
                                ui::CanvasInputKind::Key {
                                    phase: ui::CanvasKeyPhase::Down,
                                    key,
                                    modifiers,
                                    ..
                                } if modifiers.ctrl && key == "z" => {
                                    state.undo();
                                }
                                _ => return Ok(Default::default()),
                            }
                        }
                        _ => return Ok(Default::default()),
                    },
                    _ => return Ok(Default::default()),
                }
                state.revision += 1;
                Ok(api::Output {
                    views: vec![api::View {
                        panel: "table".into(),
                        document: state.view(),
                    }],
                    ..Default::default()
                })
            })
        })
    }
}
export!(Plugin);
impl Table {
    /// Domain undo deliberately replaces the target generation to cancel conflicting native preedit.
    fn undo(&mut self) {
        if let Some((index, text)) = self.undo.pop() {
            self.cells[index] = text;
            self.text_revisions[index] += 1;
            self.epochs[index] += 1;
            self.acknowledgements[index] = 0;
        }
    }
    /// Describe six visible cells and logical totals, with all shaping/input/accessibility public.
    fn view(&self) -> ui::Document {
        let mut roots = Vec::new();
        let mut targets = Vec::new();
        let mut semantics = vec![json!({"id":"table","role":"grid",
            "name":{"en":"Editable table","zh_cn":"可编辑表格"},"rect":{"x":0,"y":0,"w":320,"h":120},"row_count":100000,"column_count":2})];
        for (index, text) in self.cells.iter().enumerate() {
            let space = format!("space-{index}");
            let paragraph = format!("text-{index}");
            let target = format!("cell-{index}");
            let layout = format!("cell-{}-{}", index, self.text_revisions[index]);
            let row = index / 2;
            let column = index % 2;
            roots.push(json!({"kind":"group","id":space,"transform":[1,0,0,1,column*160,row*40],
                "clip":{"kind":"rect","rect":{"x":0,"y":0,"w":160,"h":40}},"children":[
                {"kind":"shape","id":format!("background-{index}"),"geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":159,"h":39}},
                    "fill":{"kind":"solid","rgba":if self.selected==index {0xccddffffu32}else{0xf5f5f5ffu32}}},
                {"kind":"paragraph","id":paragraph,"layout_key":layout,"text":text,"rect":{"x":5,"y":5,"w":150,"h":30},
                    "style":{"font":{"family":"Segoe UI","size_px":14},"wrap":false,"overflow":"ellipsis"}}
            ]}));
            targets.push(json!({"id":target,"epoch":self.epochs[index],"space":space,"focusable":true,"tab_index":index,"cursor":"text",
                "geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":160,"h":40}},
                "text_input":{"paragraph":paragraph,"layout_key":layout,"text_revision":self.text_revisions[index],"acknowledged_sequence":self.acknowledgements[index],
                    "selection":{"start":text.len(),"end":text.len()},"editing_keys":true}}));
            semantics.push(json!({"id":format!("semantic-{index}"),"parent":"table","target":target,"role":"text_input",
                "name":{"en":format!("Row {}, column {}",row+1,column+1),"zh_cn":format!("第 {} 行，第 {} 列",row+1,column+1)},
                "value":text,"row":row+1,"column":column+1,"actions":["focus","set_value","scroll_into_view"]}));
        }
        let canvas:ui::Canvas=serde_json::from_value(json!({"paint":[],"scene":{"identity":1,"revision":self.revision,"roots":roots,"targets":targets,"semantics":semantics}})).unwrap();
        ui::Document::new(
            ui::Node::column(
                "root",
                vec![
                    ui::Node::button("undo", "撤销 / Undo"),
                    ui::Node::new("drawing", ui::Kind::Canvas(canvas)).grow(),
                ],
            )
            .grow(),
        )
        .revision(self.revision)
    }
}
