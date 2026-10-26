//! Real system clipboard checks retain exact inert bytes, format priority and old native text access.
use super::*;

/// Round-trip OS formats and ensure an overwritten enumeration cannot be read as a new clipboard.
pub(super) fn run(window: &mut Window, cx: &mut App) -> anyhow::Result<()> {
    use platform_windows::clipboard;
    use protocol::transfer::{self, Item};
    let hwnd = super::uia::owned_handle(window)? as isize;
    let html="<table><tr><td>中文</td></tr></table><script>inert()</script><img src='https://invalid.example/x'>".as_bytes().to_vec();
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        4,
        4,
        image::Rgba([1, 2, 3, 255]),
    ))
    .write_to(&mut png, image::ImageFormat::Png)?;
    let items = vec![
        (
            "text/plain;charset=utf-8".into(),
            "中文\tvalue".as_bytes().to_vec(),
        ),
        ("text/html".into(), html),
        ("image/png".into(), png.into_inner()),
        (
            "application/vnd.nanobug.canvas-table.cells".into(),
            vec![0, 1, 0, 255],
        ),
    ];
    clipboard::write(hwnd, &items, transfer::MAX_BYTES)
        .map_err(|error| anyhow::anyhow!("native formatted write: {error}"))?;
    let snapshot = clipboard::enumerate(hwnd)?;
    anyhow::ensure!(
        snapshot.formats.len() == 4,
        "native enumeration must expose the four inert formats"
    );
    for (mime, expected) in &items {
        let bytes = clipboard::read(hwnd, snapshot.sequence, mime, transfer::MAX_BYTES)
            .map_err(|error| anyhow::anyhow!("native selected {mime} read: {error}"))?;
        anyhow::ensure!(
            &bytes == expected,
            "native {mime} bytes must survive exact Windows encoding/padding"
        );
        Item {
            mime: mime.clone(),
            bytes,
        }
        .validate()?;
    }
    anyhow::ensure!(
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .as_deref()
            == Some("中文\tvalue"),
        "ordinary GPUI text clipboard must read the formatted clipboard's TSV fallback"
    );
    clipboard::write(
        hwnd,
        &[("text/plain;charset=utf-8".into(), b"replacement".to_vec())],
        transfer::MAX_BYTES,
    )?;
    anyhow::ensure!(
        matches!(
            clipboard::read(hwnd, snapshot.sequence, "text/html", transfer::MAX_BYTES),
            Err(clipboard::Error::Changed)
        ),
        "old format enumeration must not borrow a replacement system clipboard"
    );
    Ok(())
}
