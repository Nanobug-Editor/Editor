//! Row/column copying and PNG transport remain guest policy over the same generic transfer API.
use super::*;
use plugin_protocol::{
    Rect,
    transfer::{self, DragSource, DropTarget, Item},
};

impl Table {
    /// Visible handles describe finite row/column matrices, independent of cell focus or native mouse code.
    pub(super) fn drag_handles(
        &self,
        roots: &mut Vec<serde_json::Value>,
        targets: &mut Vec<serde_json::Value>,
    ) {
        for (id, rect, rows) in (0..3)
            .map(|row| {
                (
                    format!("row-{row}"),
                    Rect {
                        x: 320.,
                        y: row as f32 * 40.,
                        w: 24.,
                        h: 40.,
                    },
                    vec![self.cells[row * 2..row * 2 + 2].to_vec()],
                )
            })
            .chain((0..2).map(|column| {
                (
                    format!("column-{column}"),
                    Rect {
                        x: column as f32 * 160.,
                        y: 120.,
                        w: 160.,
                        h: 24.,
                    },
                    self.cells
                        .chunks(2)
                        .map(|row| vec![row[column].clone()])
                        .collect(),
                )
            }))
        {
            roots.push(json!({"kind":"shape","id":format!("handle-{id}"),"geometry":{"kind":"rect","rect":rect},"fill":{"kind":"solid","rgba":0x99aabbffu32}}));
            targets.push(json!({"id":id,"epoch":1,"geometry":{"kind":"rect","rect":rect},"cursor":ui::CanvasCursor::Grab,"drag_source":DragSource { items:exchange::formats(&rows) },"drop_target":DropTarget { preferred:vec!["text/html".into(),"text/plain;charset=utf-8".into()] }}));
        }
    }
    /// This ordinary native node accepts PNG bytes and can export the exact imported representation.
    pub(super) fn image_drop(&self) -> ui::Node {
        let mut node = ui::Node::text(
            "image-drop",
            match &self.image {
                Some(bytes) => format!("PNG · {} bytes / 可拖出图片", bytes.len()),
                None => "拖入 PNG / Drop PNG".into(),
            },
        );
        node.drop_target = Some(DropTarget {
            preferred: vec!["image/png".into()],
        });
        node.drag_source = self.image.as_ref().map(|bytes| DragSource {
            items: vec![Item {
                mime: "image/png".into(),
                bytes: bytes.clone(),
            }],
        });
        node
    }
    /// A drop records the hit destination once; subsequent selection/focus cannot redirect its result.
    pub(super) fn drop(&mut self, event: transfer::Event) -> Result<(), api::Failure> {
        if self.transfer_request.is_some() {
            return Err(api::Failure::new(
                api::ErrorCode::Conflict,
                "Table transfer already pending",
            ));
        }
        let destination = event.address.canvas.as_ref().and_then(|anchor| {
            anchor
                .target
                .id
                .strip_prefix("row-")
                .and_then(|id| id.parse::<usize>().ok())
                .filter(|row| *row < 3)
                .map(|row| row * 2)
                .or_else(|| {
                    anchor
                        .target
                        .id
                        .strip_prefix("column-")
                        .and_then(|id| id.parse::<usize>().ok())
                        .filter(|column| *column < 2)
                })
        });
        if destination.is_none() && event.address.node != "image-drop" {
            return Err(api::Failure::new(
                api::ErrorCode::InvalidRequest,
                "Unknown table destination",
            ));
        }
        let preferred = if destination.is_some() {
            vec!["text/html".into(), "text/plain;charset=utf-8".into()]
        } else {
            vec!["image/png".into()]
        };
        let result = api::guest::request(api::Operation::Editor {
            operation: api::EditorOperation::Transfer {
                operation: transfer::Operation::Read {
                    offer: event.offer.handle.clone(),
                    preferred,
                },
            },
            timeout_ms: 30_000,
        })?;
        let api::Value::Accepted(handle) = result else {
            return Err(api::Failure::new(
                api::ErrorCode::InvalidState,
                "Transfer was not accepted",
            ));
        };
        self.transfer_request = Some(handle);
        self.transfer_offer = Some(event.offer.handle);
        self.transfer_destination = destination;
        self.transfer_status = "reading".into();
        Ok(())
    }
    /// Native validation precedes completion; guest matrix limits and undo still apply to table data.
    pub(super) fn drop_complete(&mut self, result: Result<api::EditorValue, api::Failure>) {
        self.transfer_request = None;
        if let Some(handle) = self.transfer_offer.take() {
            let _ = api::guest::request(api::Operation::CloseResource { handle });
        }
        let status = match result {
            Ok(api::EditorValue::Transfer(transfer::Value::Data(item))) => {
                if let Some(destination) = self.transfer_destination.take() {
                    self.import_at(&item, destination).map(|_| "copied")
                } else if item.mime == "image/png" && item.validate().is_ok() {
                    self.image = Some(item.bytes);
                    Ok("image received")
                } else {
                    Err("unsupported image")
                }
            }
            _ => Err("transfer failed"),
        };
        self.transfer_status = status.unwrap_or_else(|error| error).into();
    }
}
