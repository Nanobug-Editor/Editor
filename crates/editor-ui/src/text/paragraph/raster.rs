//! Native glyph pixels support bounded affine/path-clipped layers without another font renderer.
use super::*;
use futures::{StreamExt as _, channel::mpsc as queue};
use gpui_kit::{PlatformTextSystem, TextSystem, WindowTextSystem};
use std::{
    sync::{Arc, Mutex, OnceLock, mpsc},
    thread::ThreadId,
    time::Duration,
};

/// The actual App platform supplies font IDs once, before any window or scene worker starts.
pub(super) struct NativeFonts {
    backend: Arc<dyn PlatformTextSystem>,
    system: Arc<TextSystem>,
    ui_thread: ThreadId,
    color_jobs: Mutex<queue::Sender<ColorGlyph>>,
}
/// The public native raster API's color branch uses GPUI's shared GPU context on its UI thread.
struct ColorGlyph {
    params: gpui_kit::RenderGlyphParams,
    bounds: Bounds<gpui_kit::DevicePixels>,
    reply: mpsc::SyncSender<Result<(gpui_kit::Size<gpui_kit::DevicePixels>, Vec<u8>), String>>,
}
static NATIVE_FONTS: OnceLock<NativeFonts> = OnceLock::new();

/// Bind worker rasterization to the App's actual native fonts. Rebinding to a different backend fails.
/// This grants no plugin font-loading or filesystem authority and never constructs another platform.
pub fn install_native_raster_fonts(
    backend: Arc<dyn PlatformTextSystem>,
    cx: &App,
) -> Result<(), String> {
    if let Some(current) = NATIVE_FONTS.get() {
        return if Arc::ptr_eq(&current.backend, &backend) {
            Ok(())
        } else {
            Err("Native paragraph fonts are already bound to another App platform".into())
        };
    }
    let system = Arc::new(TextSystem::new(backend.clone()));
    let (sender, mut receiver) = queue::channel::<ColorGlyph>(32);
    let renderer = backend.clone();
    NATIVE_FONTS
        .set(NativeFonts {
            backend,
            system,
            ui_thread: std::thread::current().id(),
            color_jobs: Mutex::new(sender),
        })
        .map_err(|_| "Native paragraph font initialization raced".to_owned())?;
    cx.foreground_executor()
        .spawn(async move {
            while let Some(job) = receiver.next().await {
                let result = renderer
                    .rasterize_glyph(&job.params, job.bounds)
                    .map_err(|error| format!("Native color glyph raster failed: {error}"));
                let _ = job.reply.send(result);
            }
        })
        .detach();
    Ok(())
}
impl NativeFonts {
    /// Native bounds are CPU-side font metadata and are checked before any bitmap allocation.
    pub(super) fn glyph_bounds(
        &self,
        params: &gpui_kit::RenderGlyphParams,
    ) -> Result<Bounds<gpui_kit::DevicePixels>, String> {
        self.backend
            .glyph_raster_bounds(params)
            .map_err(|error| format!("Native glyph bounds failed: {error}"))
    }
    /// Color glyph rasterization uses the UI executor because the locked Windows backend shares D3D.
    /// Monochrome CPU glyphs remain on the worker; queue saturation/timeout is an explicit scene error.
    pub(super) fn glyph_pixels(
        &self,
        params: &gpui_kit::RenderGlyphParams,
        bounds: Bounds<gpui_kit::DevicePixels>,
    ) -> Result<(gpui_kit::Size<gpui_kit::DevicePixels>, Vec<u8>), String> {
        if !params.is_emoji || std::thread::current().id() == self.ui_thread {
            return self
                .backend
                .rasterize_glyph(params, bounds)
                .map_err(|error| format!("Native glyph raster failed: {error}"));
        }
        let (reply, result) = mpsc::sync_channel(1);
        self.color_jobs
            .lock()
            .map_err(|_| "Native glyph queue is unavailable")?
            .try_send(ColorGlyph {
                params: params.clone(),
                bounds,
                reply,
            })
            .map_err(|_| "Native color glyph queue is full or retired")?;
        result
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| "Native color glyph UI request timed out or retired")?
    }
}
/// Straight RGBA native pixels; the owner charges encoded bytes and final texture residency separately.
pub struct NativeParagraphPixels {
    /// Bounded output dimensions, each at most 2048 physical samples.
    pub width: u32,
    pub height: u32,
    /// Straight sRGB RGBA, exactly width × height × 4.
    pub rgba: Vec<u8>,
}
/// Shape the exact original `text` using resolved `layout`, then rasterize within output/work budgets.
/// `density` changes sampling only. All size/work checks run before native glyph allocation.
pub fn rasterize_native_paragraph(
    text: SharedString,
    layout: &CanvasTextLayout,
    density: f32,
    byte_budget: u64,
    work: &mut u64,
) -> Result<NativeParagraphPixels, String> {
    rasterize(text, layout, density, byte_budget, work, None)
}
/// Rasterize one sealed native presentation, including selection, marked underline and caret.
/// All ranges address `presentation.text`; the same native glyph layout supplies every decoration.
pub fn rasterize_native_draft(
    presentation: &crate::text::input::CanvasTextPresentation,
    layout: &CanvasTextLayout,
    density: f32,
    byte_budget: u64,
    work: &mut u64,
) -> Result<NativeParagraphPixels, String> {
    rasterize(
        presentation.text.clone().into(),
        layout,
        density,
        byte_budget,
        work,
        Some(presentation),
    )
}
/// Native static and editable pixels share allocation checks, shaping and source-over composition.
fn rasterize(
    text: SharedString,
    layout: &CanvasTextLayout,
    density: f32,
    byte_budget: u64,
    work: &mut u64,
    presentation: Option<&crate::text::input::CanvasTextPresentation>,
) -> Result<NativeParagraphPixels, String> {
    let fonts = NATIVE_FONTS
        .get()
        .ok_or("Native paragraph fonts have not been initialized")?;
    if !density.is_finite() || !(0.25..=4.).contains(&density) {
        return Err("Native paragraph raster density is outside its budget".into());
    }
    let system = WindowTextSystem::new(fonts.system.clone());
    let paragraph = CanvasParagraph::shape(
        text,
        layout.font.clone(),
        layout.font_size,
        layout.color,
        layout.bounds,
        &layout.style,
        &system,
        density,
    )?;
    let mut backgrounds = Vec::new();
    let mut foregrounds = Vec::new();
    if let Some(presentation) = presentation {
        for rect in
            paragraph.selection_rects(presentation.selection.start..presentation.selection.end)?
        {
            backgrounds.push((rect, rgba(0x3388ff44).into()));
        }
        if let Some(marked) = &presentation.marked {
            for mut rect in paragraph.selection_rects(marked.start..marked.end)? {
                rect.origin.y += rect.size.height - px(2.);
                rect.size.height = px(1.);
                foregrounds.push((rect, layout.color.into()));
            }
        }
        if presentation.selection.start == presentation.selection.end {
            if let Some(rect) = paragraph.caret_bounds(presentation.selection.end, true) {
                foregrounds.push((rect, layout.color.into()));
            }
        }
    }
    paragraph.visual.raster(
        fonts,
        density,
        byte_budget,
        work,
        &backgrounds,
        &foregrounds,
    )
}
