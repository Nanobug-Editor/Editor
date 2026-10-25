//! Coalesce one bounded complex-layer draft job per Canvas and retire its native texture explicitly.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

/// Private native draft pixels have a process-wide pool separate from committed worker scene pixels.
static RESIDENT: AtomicU64 = AtomicU64::new(0);
const JOB_BYTES: u64 = 16 * 1024 * 1024;
const POOL_BYTES: u64 = 64 * 1024 * 1024;

/// Reservation precedes raster allocation and stays with the displayed private texture.
struct PixelsLease(u64);
impl PixelsLease {
    /// Admit at most the native raster's 2048² RGBA maximum before a worker starts.
    fn reserve() -> Result<Self, String> {
        RESIDENT
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |bytes| {
                bytes
                    .checked_add(JOB_BYTES)
                    .filter(|next| *next <= POOL_BYTES)
            })
            .map(|_| Self(JOB_BYTES))
            .map_err(|_| "Native text presentation pixel pool is full".into())
    }
    /// Release unused worst-case capacity after actual output dimensions are known.
    fn shrink(&mut self, actual: u64) {
        debug_assert!(actual <= self.0);
        RESIDENT.fetch_sub(self.0 - actual, Ordering::AcqRel);
        self.0 = actual;
    }
}
impl Drop for PixelsLease {
    fn drop(&mut self) {
        RESIDENT.fetch_sub(self.0, Ordering::AcqRel);
    }
}

/// Exact source/native target and presentation identity; an accepted replacement cannot borrow pixels.
#[derive(Clone)]
struct Request {
    graphics: Arc<scene::PreparedScene>,
    target: (String, u64),
    draft: scene::paragraph::NativeDraft,
}
impl PartialEq for Request {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.graphics, &other.graphics)
            && self.target == other.target
            && self.draft == other.draft
    }
}
/// One completed immutable private image, retaining the memory reservation until native eviction.
struct Displayed {
    request: Request,
    image: super::super::super::images::VectorImage,
    _pixels: PixelsLease,
}
/// One running job and one coalesced desired presentation; commits/preedit are never coalesced as input.
#[derive(Default)]
pub(super) struct State {
    wanted: Option<Request>,
    displayed: Option<Displayed>,
    failed: Option<Request>,
    job: Option<gpui_kit::Task<()>>,
}
impl State {
    /// Revocation withdraws presentation authority immediately; native eviction occurs on render/release.
    pub(super) fn retire(&mut self) {
        self.wanted = None;
        self.failed = None;
    }
    /// Explicitly evict GPUI atlas state before dropping the final private pixel lease.
    fn evict(&mut self, window: Option<&mut Window>, cx: &mut App) {
        if let Some(displayed) = self.displayed.take() {
            cx.drop_image(displayed.image.image.clone(), window);
        }
    }
}
impl CanvasView {
    /// Register native texture retirement against this exact owning window and entity lifetime.
    pub(in crate::ui::plugin::canvas) fn install_draft_retirement(
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let owner_window = window.window_handle();
        cx.on_release(move |view, cx| {
            let image = view.targets.raster.displayed.take();
            if let Some(displayed) = image {
                if owner_window
                    .update(cx, |_, window, cx| {
                        cx.drop_image(displayed.image.image.clone(), Some(window))
                    })
                    .is_err()
                {
                    cx.drop_image(displayed.image.image.clone(), None);
                }
            }
        })
        .detach();
    }
    /// Snapshot the live focused endpoint only; stale sources and ordinary native text use no raster job.
    fn desired_draft(&self, window: &Window, cx: &App) -> Option<Request> {
        let graphics = self.scene_graphics.as_ref()?;
        if !self.enabled
            || self.drawing.scene.as_ref() != Some(&graphics.source)
            || !graphics.drawing.as_ref().ok()?.uses_raster()
        {
            return None;
        }
        let (target, input) = self
            .targets
            .sessions
            .iter()
            .find(|(_, input)| input.state.read(cx).active(window))?;
        let state = input.state.read(cx);
        let binding = state.binding();
        Some(Request {
            graphics: graphics.clone(),
            target: target.clone(),
            draft: scene::paragraph::NativeDraft {
                paragraph: binding.paragraph.clone(),
                layout_key: binding.layout_key.clone(),
                presentation: state.presentation(),
            },
        })
    }
    /// Keep at most one worker running. A newer native presentation replaces only the desired snapshot.
    pub(in crate::ui::plugin::canvas) fn sync_native_draft(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let wanted = self.desired_draft(window, cx);
        if self.targets.raster.wanted != wanted {
            self.targets.raster.failed = None;
        }
        self.targets.raster.wanted = wanted.clone();
        if wanted.is_none() {
            self.targets.raster.evict(Some(window), cx);
            return;
        }
        let request = wanted.unwrap();
        let state = &self.targets.raster;
        if state.job.is_some()
            || state.failed.as_ref() == Some(&request)
            || state
                .displayed
                .as_ref()
                .is_some_and(|image| image.request == request)
        {
            return;
        }
        let work = request.clone();
        self.targets.raster.job = Some(cx.spawn_in(window, async move |owner, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut lease = PixelsLease::reserve()?;
                    let image = work.graphics.prepare_draft(&work.draft, JOB_BYTES)?;
                    let size = image.image.size(0);
                    lease.shrink(size.width.0 as u64 * size.height.0 as u64 * 4);
                    Ok::<_, String>((image, lease))
                })
                .await;
            let _ = owner.update_in(cx, |view, window, cx| {
                view.targets.raster.job = None;
                if view.targets.raster.wanted.as_ref() == Some(&request)
                    && view.desired_draft(window, cx).as_ref() == Some(&request)
                {
                    match result {
                        Ok((image, lease)) => {
                            view.targets.raster.evict(Some(window), cx);
                            view.targets.raster.displayed = Some(Displayed {
                                request,
                                image,
                                _pixels: lease,
                            });
                        }
                        Err(error) => {
                            view.targets.raster.failed = Some(request.clone());
                            view.emit_target(
                                request.target,
                                CanvasInputKind::TextError {
                                    code: protocol::api::ErrorCode::LimitExceeded,
                                    message: error.chars().take(128).collect(),
                                },
                                cx,
                            );
                        }
                    }
                }
                cx.notify();
            });
        }));
    }
    /// Paint only pixels for the exact current desired source; source receipts stay public and unchanged.
    pub(in crate::ui::plugin::canvas) fn paint_native_draft(
        &self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
    ) -> Result<bool, String> {
        let state = &self.targets.raster;
        let Some(displayed) = state
            .displayed
            .as_ref()
            .filter(|image| state.wanted.as_ref() == Some(&image.request))
        else {
            return Ok(false);
        };
        let image = &displayed.image;
        window
            .paint_image(
                bounds,
                rect_bounds(self.project(image.rect, bounds), bounds.origin),
                Default::default(),
                image.image.clone(),
                0,
                false,
            )
            .map_err(|error| format!("Native text presentation submission failed: {error}"))?;
        Ok(true)
    }
}
