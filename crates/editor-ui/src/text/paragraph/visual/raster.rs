//! Composite the same native glyph IDs/positions into bounded straight RGBA for affine placement.
use super::super::raster::NativeParagraphPixels;
use super::*;
use gpui_kit::{RenderGlyphParams, Rgba};

impl VisualParagraph {
    /// Charge output and every glyph's native bounds before rasterization; clip without rewrapping.
    pub(in crate::text::paragraph) fn raster(
        &self,
        fonts: &super::super::raster::NativeFonts,
        density: f32,
        byte_budget: u64,
        work: &mut u64,
        backgrounds: &[(Bounds<Pixels>, Rgba)],
        foregrounds: &[(Bounds<Pixels>, Rgba)],
    ) -> Result<NativeParagraphPixels, String> {
        let width = (self.bounds.size.width / px(1.) * density).ceil().max(1.) as u32;
        let height = (self.bounds.size.height / px(1.) * density).ceil().max(1.) as u32;
        let count = u64::from(width) * u64::from(height);
        if width > 2048 || height > 2048 || count * 4 > byte_budget.min(16 * 1024 * 1024) {
            return Err("Native paragraph raster output budget exhausted".into());
        }
        *work = work
            .checked_sub(count)
            .ok_or("Native paragraph raster work budget exhausted")?;
        let mut output = vec![0u8; count as usize * 4];
        decorations(
            &mut output,
            width,
            height,
            self.bounds.origin,
            density,
            backgrounds,
            work,
        )?;
        for glyph in &self.glyphs {
            let params = RenderGlyphParams {
                font_id: glyph.font,
                glyph_id: glyph.id,
                font_size: glyph.size,
                subpixel_variant: gpui_kit::point(0, 0),
                scale_factor: density,
                is_emoji: glyph.emoji,
                subpixel_rendering: false,
                dilation: 0,
            };
            let bounds = fonts.glyph_bounds(&params)?;
            let w = i32::from(bounds.size.width);
            let h = i32::from(bounds.size.height);
            if w <= 0 || h <= 0 {
                continue;
            }
            let pixels = (w as u64)
                .checked_mul(h as u64)
                .ok_or("Native glyph raster overflow")?;
            if w > 2048 || h > 2048 || pixels * 4 > 16 * 1024 * 1024 {
                return Err("Native glyph exceeds its raster allocation budget".into());
            }
            let x = ((glyph.origin.x - self.bounds.origin.x) / px(1.) * density).floor() as i32
                + i32::from(bounds.origin.x);
            let y = ((glyph.origin.y - self.bounds.origin.y) / px(1.) * density).floor() as i32
                + i32::from(bounds.origin.y);
            if x >= width as i32 || y >= height as i32 || x + w <= 0 || y + h <= 0 {
                continue;
            }
            *work = work
                .checked_sub(pixels)
                .ok_or("Native glyph sampling work budget exhausted")?;
            let (size, bitmap) = fonts.glyph_pixels(&params, bounds)?;
            let channels = if glyph.emoji { 4 } else { 1 };
            if i32::from(size.width) != w
                || i32::from(size.height) != h
                || bitmap.len() != pixels as usize * channels
            {
                return Err("Native glyph returned an invalid pixel extent".into());
            }
            let color: Rgba = glyph.color.into();
            for row in 0.max(-y)..h.min(height as i32 - y) {
                for col in 0.max(-x)..w.min(width as i32 - x) {
                    let at = (row * w + col) as usize * channels;
                    let source = if glyph.emoji {
                        // GPUI native polychrome glyph atlases use straight BGRA, including DirectWrite.
                        [bitmap[at + 2], bitmap[at + 1], bitmap[at], bitmap[at + 3]]
                    } else {
                        [
                            (color.r * 255.).round() as u8,
                            (color.g * 255.).round() as u8,
                            (color.b * 255.).round() as u8,
                            (f32::from(bitmap[at]) * color.a).round() as u8,
                        ]
                    };
                    let destination =
                        ((y + row) as usize * width as usize + (x + col) as usize) * 4;
                    over(&mut output[destination..destination + 4], source);
                }
            }
        }
        decorations(
            &mut output,
            width,
            height,
            self.bounds.origin,
            density,
            foregrounds,
            work,
        )?;
        // Composite in premultiplied bytes, then return the PNG encoder's straight-alpha convention.
        for pixel in output.chunks_exact_mut(4) {
            if pixel[3] > 0 {
                for channel in 0..3 {
                    pixel[channel] = ((u32::from(pixel[channel]) * 255 + u32::from(pixel[3]) / 2)
                        / u32::from(pixel[3]))
                    .min(255) as u8;
                }
            }
        }
        Ok(NativeParagraphPixels {
            width,
            height,
            rgba: output,
        })
    }
}
/// Charge clipped selection/preedit/caret samples before compositing into the existing output.
fn decorations(
    output: &mut [u8],
    width: u32,
    height: u32,
    origin: gpui_kit::Point<Pixels>,
    density: f32,
    rects: &[(Bounds<Pixels>, Rgba)],
    work: &mut u64,
) -> Result<(), String> {
    for (rect, color) in rects {
        let left =
            (((rect.origin.x - origin.x) / px(1.) * density).floor() as i32).clamp(0, width as i32);
        let top = (((rect.origin.y - origin.y) / px(1.) * density).floor() as i32)
            .clamp(0, height as i32);
        let right = ((((rect.origin.x + rect.size.width) - origin.x) / px(1.) * density).ceil()
            as i32)
            .clamp(left, width as i32);
        let bottom = ((((rect.origin.y + rect.size.height) - origin.y) / px(1.) * density).ceil()
            as i32)
            .clamp(top, height as i32);
        *work = work
            .checked_sub((right - left) as u64 * (bottom - top) as u64)
            .ok_or("Native text decoration work budget exhausted")?;
        let source = [
            (color.r * 255.).round() as u8,
            (color.g * 255.).round() as u8,
            (color.b * 255.).round() as u8,
            (color.a * 255.).round() as u8,
        ];
        for y in top..bottom {
            for x in left..right {
                let at = (y as usize * width as usize + x as usize) * 4;
                over(&mut output[at..at + 4], source);
            }
        }
    }
    Ok(())
}
/// Source-over keeps accumulated bytes premultiplied until the complete paragraph is encoded.
fn over(destination: &mut [u8], source: [u8; 4]) {
    let alpha = u32::from(source[3]);
    let inverse = 255 - alpha;
    for channel in 0..3 {
        destination[channel] =
            ((u32::from(source[channel]) * alpha + u32::from(destination[channel]) * inverse + 127)
                / 255)
                .min(255) as u8;
    }
    destination[3] = (alpha + (u32::from(destination[3]) * inverse + 127) / 255).min(255) as u8;
}
