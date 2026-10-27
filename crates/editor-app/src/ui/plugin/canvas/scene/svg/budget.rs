//! SVG expansion and retained PNG bytes share a quota charged before growth.
use super::*;
use std::{cell::Cell, fmt, io, rc::Rc};

/// Body and definitions share one allocation allowance; rejected writes cannot grow either buffer.
pub(super) struct SvgText {
    text: String,
    remaining: Rc<Cell<usize>>,
    failed: Rc<Cell<bool>>,
}
impl SvgText {
    /// Reserve bounded header space and return two writers sharing the rest of the 4 MiB quota.
    pub(super) fn pair() -> (Self, Self) {
        let remaining = Rc::new(Cell::new(4 * 1024 * 1024 - 1024));
        let failed = Rc::new(Cell::new(false));
        (
            Self {
                text: String::new(),
                remaining: remaining.clone(),
                failed: failed.clone(),
            },
            Self {
                text: String::new(),
                remaining,
                failed,
            },
        )
    }
    /// Return capacity available to encoding before any temporary output is allocated.
    pub(super) fn remaining(&self) -> usize {
        self.remaining.get()
    }
    /// Charge retained image bytes before installing a resolver entry.
    pub(super) fn charge(&mut self, bytes: usize) -> Result<(), String> {
        let Some(remaining) = self.remaining.get().checked_sub(bytes) else {
            self.failed.set(true);
            return self.check();
        };
        self.remaining.set(remaining);
        self.check()
    }
    /// Retain the convenient string append form while remembering any rejected expansion.
    pub(super) fn push_str(&mut self, value: &str) {
        let _ = fmt::Write::write_str(self, value);
    }
    /// Propagate an overflow instead of rendering truncated XML as successful artwork.
    pub(super) fn check(&self) -> Result<(), String> {
        if self.failed.get() {
            Err("Canvas raster source exceeds its expansion budget".into())
        } else {
            Ok(())
        }
    }
}
impl fmt::Write for SvgText {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let Some(remaining) = self.remaining.get().checked_sub(value.len()) else {
            self.failed.set(true);
            return Err(fmt::Error);
        };
        if self.failed.get() {
            return Err(fmt::Error);
        }
        self.remaining.set(remaining);
        self.text.push_str(value);
        Ok(())
    }
}
impl fmt::Display for SvgText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.text)
    }
}

/// Encode admitted straight BGRA pixels as inert PNG within `remaining` SVG bytes.
/// Source decoding already caps the temporary RGBA conversion at 64 MiB; output is independently capped.
pub(super) fn png(
    bitmap: &super::super::super::super::bitmap::Bitmap,
    remaining: usize,
) -> Result<Vec<u8>, String> {
    let bytes = bitmap
        .image
        .as_bytes(0)
        .ok_or("Canvas image frame is unavailable")?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("Canvas image exceeds its decode budget".into());
    }
    let mut pixels = bytes.to_vec();
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    png_rgba(&pixels, bitmap.width, bitmap.height, remaining)
}
/// Native paragraph glyph pixels use the same write-before-grow PNG cap as admitted source images.
pub(super) fn png_rgba(
    pixels: &[u8],
    width: u32,
    height: u32,
    remaining: usize,
) -> Result<Vec<u8>, String> {
    use image::ImageEncoder as _;
    if pixels.len() > 64 * 1024 * 1024
        || u64::from(width) * u64::from(height) * 4 != pixels.len() as u64
    {
        return Err("Canvas native pixels exceed their encode budget".into());
    }
    let mut output = PngOutput {
        bytes: Vec::new(),
        limit: remaining.saturating_sub(1024),
    };
    image::codecs::png::PngEncoder::new(&mut output)
        .write_image(pixels, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|_| "Canvas PNG exceeds its expansion budget or failed to encode")?;
    Ok(output.bytes)
}
/// PNG compression writes through this cap rather than allocating a complete oversized result first.
struct PngOutput {
    bytes: Vec<u8>,
    limit: usize,
}
impl io::Write for PngOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("Canvas PNG expansion budget exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
