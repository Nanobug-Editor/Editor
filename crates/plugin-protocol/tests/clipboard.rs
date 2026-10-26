//! Formatted clipboard items are inert bounded data carried by the public typed request contract.
use plugin_protocol::api::EditorOperation;
use serde_json::json;

/// Existing plain-text requests keep their wire spelling while formatted writes retain HTML as bytes.
#[test]
fn formatted_clipboard_preserves_inert_formats_and_legacy_text() {
    let html = b"<table><tr><td>name</td></tr></table><script>never execute</script>";
    let wire = json!({"kind":"clipboard","operation":{"type":"write","items":[
        {"mime":"text/plain;charset=utf-8","bytes":b"name\tvalue"},
        {"mime":"text/html","bytes":html.as_slice()},
        {"mime":"application/vnd.nanobug.canvas-table.cells","bytes":[1,2,3]}]}});
    let operation: EditorOperation = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(operation).unwrap(), wire);
    let old: EditorOperation =
        serde_json::from_value(json!({"kind":"write_clipboard","text":"中文"})).unwrap();
    assert_eq!(
        serde_json::to_value(old).unwrap(),
        json!({"kind":"write_clipboard","text":"中文"})
    );
}

/// A caller may publish only its own custom namespace and a finite well-formed complete payload.
#[test]
fn formatted_clipboard_admission_bounds_namespaces_and_bytes() {
    use plugin_protocol::transfer::{ClipboardOperation, Item, MAX_BYTES};
    let write = |mime: &str, bytes: Vec<u8>| ClipboardOperation::Write {
        items: vec![Item {
            mime: mime.into(),
            bytes,
        }],
    };
    write("application/vnd.nanobug.canvas-table.cells", vec![1, 2])
        .validate("canvas-table")
        .unwrap();
    assert!(
        write("application/vnd.nanobug.other.cells", vec![1])
            .validate("canvas-table")
            .is_err()
    );
    assert!(
        write("text/html", vec![255])
            .validate("canvas-table")
            .is_err()
    );
    assert!(
        write("text/plain;charset=utf-8", vec![b'x'; MAX_BYTES + 1])
            .validate("canvas-table")
            .is_err()
    );
    assert!(
        write("application/octet-stream", vec![1])
            .validate("canvas-table")
            .is_err()
    );
}

/// Dotted package IDs cannot borrow a shorter package's prefix, for clipboard or outgoing drag.
#[test]
fn custom_formats_have_an_unambiguous_package_owner() {
    use plugin_protocol::transfer::{ClipboardOperation, DragSource, Item};
    let items = vec![Item {
        mime: "application/vnd.nanobug.foo.bar.state".into(),
        bytes: vec![1],
    }];
    let write = ClipboardOperation::Write {
        items: items.clone(),
    };
    write.validate("foo.bar").unwrap();
    assert!(
        write.validate("foo").is_err(),
        "a format's last component is its name, not another package prefix"
    );
    DragSource {
        items: items.clone(),
    }
    .validate_for("foo.bar")
    .unwrap();
    assert!(DragSource { items }.validate_for("foo").is_err());
}
