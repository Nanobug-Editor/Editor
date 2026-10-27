//! Actual table clipboard/row/column/PNG exchange stays separate from immutable B editing acceptance.
use super::*;

/// The actual table serializes its own logical cells; the host only receives inert ordinary formats.
#[test]
#[ignore = "C independent table package via NANOBUG_CANVAS_TABLE_PACKAGE; see packages/README.md"]
fn table_copy_offers_tsv_and_html_from_the_same_guest_cells() {
    use plugin_runtime::plugin_protocol::transfer::{ClipboardOperation, ClipboardValue};
    let mut table = Table::open_interchange();
    table
        .send(ui::UiEvent {
            revision: table.view().revision,
            node: "copy".into(),
            action: ui::Action::Click,
        })
        .unwrap();
    let (_, request) = table
        .manager
        .take_editor_requests()
        .pop()
        .expect("table copy must use the public native clipboard request");
    assert!(request.begin());
    let api::EditorOperation::Clipboard {
        operation: ClipboardOperation::Write { items },
    } = request.operation()
    else {
        panic!("formatted clipboard write expected")
    };
    let text = items
        .iter()
        .find(|item| item.mime == "text/plain;charset=utf-8")
        .unwrap();
    assert_eq!(
        std::str::from_utf8(&text.bytes).unwrap(),
        "姓名 / Name\te\u{301} 👩‍💻\n数量 / Count\t42\n备注 / Notes\t中文"
    );
    let html = items.iter().find(|item| item.mime == "text/html").unwrap();
    assert_eq!(
        std::str::from_utf8(&html.bytes).unwrap(),
        "<table><tr><td>姓名 / Name</td><td>e\u{301} 👩‍💻</td></tr><tr><td>数量 / Count</td><td>42</td></tr><tr><td>备注 / Notes</td><td>中文</td></tr></table>"
    );
    request.finish(Ok(api::EditorValue::ClipboardFormats(
        ClipboardValue::Written,
    )));
    table.manager.poll();
    let mut status = None;
    table.view().root.visit(&mut |node| {
        if node.id == "clipboard-status"
            && let ui::Kind::Text { text } = &node.kind
        {
            status = Some(text.clone());
        }
    });
    assert_eq!(status.as_deref(), Some("copied"));
}

/// The table selects HTML before TSV, imports inert cell text and closes its clipboard lease.
#[test]
#[ignore = "C independent table package via NANOBUG_CANVAS_TABLE_PACKAGE; see packages/README.md"]
fn table_paste_selects_html_and_releases_the_original_clipboard_offer() {
    use plugin_runtime::plugin_protocol::transfer::{
        ClipboardOperation, ClipboardValue, Format, Item,
    };
    let mut table = Table::open_interchange();
    table
        .send(ui::UiEvent {
            revision: table.view().revision,
            node: "paste".into(),
            action: ui::Action::Click,
        })
        .unwrap();
    let (_, enumerate) = table.manager.take_editor_requests().pop().unwrap();
    assert!(enumerate.begin());
    assert!(matches!(
        enumerate.operation(),
        api::EditorOperation::Clipboard {
            operation: ClipboardOperation::Enumerate
        }
    ));
    let lease = enumerate.clipboard_offer().unwrap().clone();
    let descriptor = lease
        .publish(
            29,
            vec![
                Format {
                    mime: "text/plain;charset=utf-8".into(),
                    byte_len: None,
                },
                Format {
                    mime: "text/html".into(),
                    byte_len: None,
                },
            ],
        )
        .unwrap();
    enumerate.finish(Ok(api::EditorValue::ClipboardFormats(
        ClipboardValue::Offer(descriptor.clone()),
    )));
    table.manager.poll();
    let (_, read) = table.manager.take_editor_requests().pop().unwrap();
    assert!(read.begin());
    let api::EditorOperation::Clipboard {
        operation: ClipboardOperation::Read { offer, preferred },
    } = read.operation()
    else {
        panic!("selected clipboard read expected")
    };
    assert_eq!(offer, &descriptor.handle);
    assert_eq!(
        read.clipboard_offer().unwrap().select(preferred).unwrap(),
        (29, "text/html".into())
    );
    read.finish(Ok(api::EditorValue::ClipboardFormats(
        ClipboardValue::Data(Item {
            mime: "text/html".into(),
            bytes: "<table><tr><td>A &amp; B</td><td>中文</td></tr></table>"
                .as_bytes()
                .to_vec(),
        }),
    )));
    table.manager.poll();
    assert_eq!(table.text(0).0, "A & B");
    assert_eq!(table.text(1).0, "中文");
    assert!(
        !lease.is_live(),
        "paste completion must release its original native clipboard lease"
    );
    table
        .send(ui::UiEvent {
            revision: table.view().revision,
            node: "undo".into(),
            action: ui::Action::Click,
        })
        .unwrap();
    assert_eq!(
        table.text(0).0,
        "姓名 / Name",
        "one paste must be one guest-owned undo transaction"
    );
    assert_eq!(table.text(1).0, "e\u{301} 👩‍💻");
}

/// Header/data order and quoted TSV delimiters survive an actual guest paste followed by copy.
#[test]
#[ignore = "C independent table package via NANOBUG_CANVAS_TABLE_PACKAGE; see packages/README.md"]
fn table_html_headers_and_tsv_delimiters_round_trip_in_original_cell_order() {
    use plugin_runtime::plugin_protocol::transfer::{
        ClipboardOperation, ClipboardValue, Format, Item,
    };
    let mut table = Table::open_interchange();
    table
        .send(ui::UiEvent {
            revision: table.view().revision,
            node: "paste".into(),
            action: ui::Action::Click,
        })
        .unwrap();
    let (_, enumerate) = table.manager.take_editor_requests().pop().unwrap();
    assert!(enumerate.begin());
    let descriptor = enumerate
        .clipboard_offer()
        .unwrap()
        .publish(
            31,
            vec![Format {
                mime: "text/html".into(),
                byte_len: None,
            }],
        )
        .unwrap();
    enumerate.finish(Ok(api::EditorValue::ClipboardFormats(
        ClipboardValue::Offer(descriptor),
    )));
    table.manager.poll();
    let (_, read) = table.manager.take_editor_requests().pop().unwrap();
    assert!(read.begin());
    read.finish(Ok(api::EditorValue::ClipboardFormats(ClipboardValue::Data(Item {
        mime: "text/html".into(), bytes: b"<table><thead><tr><th>Header</th><td>A\t&quot;B&quot;\nC</td></tr></thead></table>".to_vec(),
    }))));
    table.manager.poll();
    assert_eq!(table.text(0).0, "Header");
    assert_eq!(table.text(1).0, "A\t\"B\"\nC");
    table
        .send(ui::UiEvent {
            revision: table.view().revision,
            node: "copy".into(),
            action: ui::Action::Click,
        })
        .unwrap();
    let (_, write) = table.manager.take_editor_requests().pop().unwrap();
    assert!(write.begin());
    let api::EditorOperation::Clipboard {
        operation: ClipboardOperation::Write { items },
    } = write.operation()
    else {
        panic!("formatted copy expected")
    };
    let text = items
        .iter()
        .find(|item| item.mime == "text/plain;charset=utf-8")
        .unwrap();
    assert_eq!(
        std::str::from_utf8(&text.bytes).unwrap(),
        "Header\t\"A\t\"\"B\"\"\nC\"\n数量 / Count\t42\n备注 / Notes\t中文"
    );
}

/// An external data boundary returns only the selected inert representation, never host domain state.
struct TableDrop(plugin_runtime::plugin_protocol::transfer::Item);
impl plugin_runtime::TransferReader for TableDrop {
    fn read(&self, mime: &str) -> Result<plugin_runtime::HostTransferData, api::Failure> {
        assert_eq!(mime, self.0.mime);
        Ok(plugin_runtime::HostTransferData::Bytes(self.0.clone()))
    }
}

/// The actual guest exports row/column matrices and applies one dropped row as one undo transaction.
#[test]
#[ignore = "C independent table package via NANOBUG_CANVAS_TABLE_PACKAGE; see packages/README.md"]
fn table_dragged_rows_and_columns_keep_matrix_shape_and_guest_undo() {
    use plugin_runtime::plugin_protocol::{Rect, transfer};
    use std::sync::atomic::AtomicBool;
    let mut table = Table::open_interchange();
    let shown = table.present();
    let ui::Kind::Canvas(canvas) = &shown.active_node("drawing").unwrap().kind else {
        unreachable!()
    };
    let scene = canvas.scene.as_ref().unwrap();
    let address = |target: &str| transfer::Address {
        node: "drawing".into(),
        ui_revision: shown.revision,
        source: None,
        file: None,
        canvas: Some(ui::CanvasAnchor {
            identity: scene.identity,
            scene_revision: scene.revision,
            target: ui::CanvasTargetRef {
                id: target.into(),
                epoch: 1,
            },
            space: None,
            rect: Rect {
                x: 0.,
                y: 0.,
                w: 20.,
                h: 20.,
            },
        }),
    };
    let mut row_html = None;
    for (id, expected) in [
        ("row-0", "姓名 / Name\te\u{301} 👩‍💻"),
        ("column-1", "e\u{301} 👩‍💻\n42\n中文"),
    ] {
        let source = scene
            .targets
            .iter()
            .find(|target| target.id == id)
            .and_then(|target| target.drag_source.clone())
            .expect("row and column handles must declare their domain formats");
        row_html = row_html.or_else(|| {
            source
                .items
                .iter()
                .find(|item| item.mime == "text/html")
                .cloned()
        });
        let lease = table
            .manager
            .begin_drag_snapshot(
                "canvas-table",
                "table",
                address(id),
                source,
                Arc::new(AtomicBool::new(true)),
            )
            .unwrap();
        let plugin_runtime::HostTransferData::Bytes(item) =
            lease.read(&["text/plain;charset=utf-8".into()]).unwrap()
        else {
            panic!("matrix TSV expected")
        };
        assert_eq!(std::str::from_utf8(&item.bytes).unwrap(), expected);
        table
            .manager
            .finish_drag(
                "canvas-table",
                "table",
                &lease.metadata().handle,
                transfer::DragOutcome::Cancelled,
            )
            .unwrap();
    }
    let item = row_html.unwrap();
    let source = plugin_runtime::HostTransferOffer::new(
        transfer::Origin::Plugin {
            package: "canvas-table".into(),
        },
        vec![transfer::Format {
            mime: item.mime.clone(),
            byte_len: Some(item.bytes.len() as u64),
        }],
        Arc::new(TableDrop(item)),
    )
    .unwrap();
    let offer = table
        .manager
        .begin_transfer(
            "canvas-table",
            "table",
            address("row-1"),
            source,
            transfer::Position { x: 325., y: 45. },
        )
        .unwrap();
    table
        .manager
        .update_transfer(
            "canvas-table",
            "table",
            &offer.handle,
            transfer::Phase::Drop,
            transfer::Position { x: 325., y: 45. },
        )
        .unwrap();
    let (_, request) = table
        .manager
        .take_editor_requests()
        .pop()
        .expect("row drop must read only its selected public offer");
    assert!(request.begin());
    let api::EditorOperation::Transfer {
        operation: transfer::Operation::Read { preferred, .. },
    } = request.operation()
    else {
        panic!("selected table transfer expected")
    };
    let data = request
        .transfer_offer()
        .unwrap()
        .read_selected(preferred)
        .unwrap();
    request.finish(Ok(api::EditorValue::Transfer(data)));
    table.manager.poll();
    assert_eq!(table.text(2).0, "姓名 / Name");
    assert_eq!(table.text(3).0, "e\u{301} 👩‍💻");
    table
        .send(ui::UiEvent {
            revision: table.view().revision,
            node: "undo".into(),
            action: ui::Action::Click,
        })
        .unwrap();
    assert_eq!(table.text(2).0, "数量 / Count");
    assert_eq!(table.text(3).0, "42");
}

/// PNG remains one inert representation: an actual recipient can export exactly what its dropped lease supplied.
#[test]
#[ignore = "C independent table package via NANOBUG_CANVAS_TABLE_PACKAGE; see packages/README.md"]
fn table_png_drop_can_be_exported_without_borrowing_file_or_clipboard_authority() {
    use plugin_runtime::plugin_protocol::transfer;
    use std::sync::atomic::AtomicBool;
    let mut table = Table::open_interchange();
    let png = vec![
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 48, 158, 249, 255,
        63, 0, 5, 153, 2, 203, 20, 229, 63, 253, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    let source = plugin_runtime::HostTransferOffer::new(
        transfer::Origin::OperatingSystem,
        vec![transfer::Format {
            mime: "image/png".into(),
            byte_len: Some(png.len() as u64),
        }],
        Arc::new(TableDrop(transfer::Item {
            mime: "image/png".into(),
            bytes: png.clone(),
        })),
    )
    .unwrap();
    let offer = table
        .manager
        .begin_transfer(
            "canvas-table",
            "table",
            transfer::Address::node("image-drop", table.view().revision),
            source,
            transfer::Position { x: 4., y: 5. },
        )
        .unwrap();
    table
        .manager
        .update_transfer(
            "canvas-table",
            "table",
            &offer.handle,
            transfer::Phase::Drop,
            transfer::Position { x: 4., y: 5. },
        )
        .unwrap();
    let (_, request) = table.manager.take_editor_requests().pop().unwrap();
    assert!(request.begin());
    let lease = request.transfer_offer().unwrap();
    let data = lease.read_selected(&["image/png".into()]).unwrap();
    request.finish(Ok(api::EditorValue::Transfer(data)));
    table.manager.poll();
    assert!(
        !lease.is_live(),
        "guest must release its completed incoming lease"
    );
    let document = table.view();
    let expected = document
        .active_node("image-drop")
        .unwrap()
        .drag_source
        .clone()
        .expect("received PNG must be available for a new original native gesture");
    let source = table
        .manager
        .begin_drag_snapshot(
            "canvas-table",
            "table",
            transfer::Address::node("image-drop", document.revision),
            expected,
            Arc::new(AtomicBool::new(true)),
        )
        .unwrap();
    let plugin_runtime::HostTransferData::Bytes(item) = source.read(&["image/png".into()]).unwrap()
    else {
        panic!("PNG bytes expected")
    };
    assert_eq!(item.bytes, png);
    table
        .manager
        .finish_drag(
            "canvas-table",
            "table",
            &source.metadata().handle,
            transfer::DragOutcome::Cancelled,
        )
        .unwrap();
}
