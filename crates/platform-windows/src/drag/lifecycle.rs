//! OLE target registration stays on its HWND thread; source drag loops run on a dedicated STA.
use super::*;
use std::{
    cell::RefCell,
    marker::PhantomData,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::ScreenToClient,
        System::{
            Ole::*,
            SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS},
            Threading::{AttachThreadInput, GetCurrentThreadId},
        },
        UI::WindowsAndMessaging::GetWindowThreadProcessId,
    },
    core::{BOOL, HRESULT, Ref, implement},
};

/// Client physical pixels; the GPUI host applies its own current logical DPI conversion exactly once.
#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}
/// Native callbacks describe metadata and pointer lifecycle, without reading a payload.
pub enum Event {
    Enter { offer: Arc<Offer>, position: Point },
    Move { offer: Arc<Offer>, position: Point },
    Leave,
    Drop { offer: Arc<Offer>, position: Point },
}

// Window callbacks retain thread-local GPUI routes and must always marshal back to the owner STA.
#[implement(IDropTarget, Agile = false)]
struct Target {
    hwnd: isize,
    current: RefCell<Option<Arc<Offer>>>,
    callback: Box<dyn Fn(Event) -> bool>,
}
impl Target_Impl {
    /// Convert only this registration's own client coordinates, never an ambient active window.
    fn point(&self, position: &POINTL) -> windows::core::Result<Point> {
        let mut point = POINT {
            x: position.x,
            y: position.y,
        };
        unsafe { ScreenToClient(HWND(self.hwnd as *mut _), &mut point) }.ok()?;
        Ok(Point {
            x: point.x,
            y: point.y,
        })
    }
    /// Effects reflect the original OS allowed mask and the current native receiver's acceptance.
    fn effect(&self, pointer: *mut DROPEFFECT, accepted: bool) -> windows::core::Result<()> {
        let effect =
            unsafe { pointer.as_mut() }.ok_or_else(|| windows::core::Error::from(E_POINTER))?;
        *effect = if accepted && effect.0 & DROPEFFECT_COPY.0 != 0 {
            DROPEFFECT_COPY
        } else {
            DROPEFFECT_NONE
        };
        Ok(())
    }
}
#[allow(non_snake_case)]
impl IDropTarget_Impl for Target_Impl {
    fn DragEnter(
        &self,
        object: Ref<IDataObject>,
        _: MODIFIERKEYS_FLAGS,
        position: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        if let Some(old) = self.current.borrow_mut().take() {
            old.revoke();
        }
        let offer = object.ok().and_then(|object| {
            Offer::capture(object).map_err(|_| windows::core::Error::from(E_FAIL))
        });
        match offer {
            Ok(offer) => {
                let accepted = (self.callback)(Event::Enter {
                    offer: offer.clone(),
                    position: self.point(position)?,
                });
                *self.current.borrow_mut() = Some(offer);
                self.effect(effect, accepted)
            }
            Err(_) => self.effect(effect, false),
        }
    }
    fn DragOver(
        &self,
        _: MODIFIERKEYS_FLAGS,
        position: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        let offer = self.current.borrow().clone();
        let accepted = offer.is_some_and(|offer| {
            self.point(position)
                .is_ok_and(|position| (self.callback)(Event::Move { offer, position }))
        });
        self.effect(effect, accepted)
    }
    fn DragLeave(&self) -> windows::core::Result<()> {
        (self.callback)(Event::Leave);
        if let Some(offer) = self.current.borrow_mut().take() {
            offer.revoke();
        }
        Ok(())
    }
    fn Drop(
        &self,
        _: Ref<IDataObject>,
        _: MODIFIERKEYS_FLAGS,
        position: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        let offer = self.current.borrow_mut().take();
        let accepted = if let Some(offer) = offer {
            let accepted = (self.callback)(Event::Drop {
                offer: offer.clone(),
                position: self.point(position)?,
            });
            if !accepted {
                offer.revoke();
            }
            accepted
        } else {
            false
        };
        self.effect(effect, accepted)
    }
}

/// One live window registration. Keep it until that same native window closes, including fallback drops.
/// Replaces GPUI's files-only handler; callers must route unhandled file drops to the existing GPUI path.
pub struct Registration {
    hwnd: isize,
    _target: IDropTarget,
    _thread: PhantomData<Rc<()>>,
}
impl Registration {
    /// Register only the calling thread's live process-owned HWND; never modify another app's target.
    pub fn new(owner: isize, callback: impl Fn(Event) -> bool + 'static) -> Result<Self, Error> {
        let hwnd = HWND(owner as *mut _);
        let mut process = 0;
        let thread = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process)) };
        if process != std::process::id() || thread != unsafe { GetCurrentThreadId() } {
            return Err(Error::Invalid);
        }
        let target: IDropTarget = Target {
            hwnd: owner,
            current: RefCell::new(None),
            callback: Box::new(callback),
        }
        .into();
        // GPUI registers CF_HDROP first. Our window owner now routes both generic and legacy file consumers.
        let _ = unsafe { RevokeDragDrop(hwnd) };
        unsafe { RegisterDragDrop(hwnd, &target) }.map_err(native)?;
        Ok(Self {
            hwnd: owner,
            _target: target,
            _thread: PhantomData,
        })
    }
}
impl Drop for Registration {
    fn drop(&mut self) {
        let _ = unsafe { RevokeDragDrop(HWND(self.hwnd as *mut _)) };
    }
}

/// OLE reports copying only; cancellation and targets that accept nothing remain explicit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Dropped,
    Cancelled,
}
#[implement(IDropSource)]
struct SourceControl {
    alive: Arc<AtomicBool>,
    provider: Arc<dyn Provider>,
}
#[allow(non_snake_case)]
impl IDropSource_Impl for SourceControl_Impl {
    fn QueryContinueDrag(&self, escape: BOOL, keys: MODIFIERKEYS_FLAGS) -> HRESULT {
        if escape.as_bool() || !self.alive.load(Ordering::Acquire) || !self.provider.is_live() {
            DRAGDROP_S_CANCEL
        } else if keys.0 & MK_LBUTTON.0 == 0 {
            DRAGDROP_S_DROP
        } else {
            S_OK
        }
    }
    fn GiveFeedback(&self, _: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

/// Start after a verified native left-button drag gesture, on a dedicated thread without GPUI borrows.
/// OLE pumps this source's STA while the application's own HWND/UI thread continues normally.
/// `owner` must be a live process-owned HWND; its input queue is attached only for this gesture.
pub fn start(
    owner: isize,
    provider: Arc<dyn Provider>,
    alive: Arc<AtomicBool>,
) -> Result<Outcome, Error> {
    let mut process = 0;
    let owner_thread =
        unsafe { GetWindowThreadProcessId(HWND(owner as *mut _), Some(&mut process)) };
    let current = unsafe { GetCurrentThreadId() };
    if process != std::process::id() || owner_thread == 0 || owner_thread == current {
        return Err(Error::Invalid);
    }
    unsafe { OleInitialize(None) }.map_err(native)?;
    struct Ole;
    impl Drop for Ole {
        fn drop(&mut self) {
            unsafe { OleUninitialize() };
        }
    }
    let _ole = Ole;
    // OLE's dedicated source thread must observe this owned window's held/released mouse state.
    // Detaching precedes OleUninitialize even on errors; no other application's queue is attached.
    struct InputAttachment(u32, u32);
    impl Drop for InputAttachment {
        fn drop(&mut self) {
            let _ = unsafe { AttachThreadInput(self.0, self.1, false) };
        }
    }
    unsafe { AttachThreadInput(current, owner_thread, true) }
        .ok()
        .map_err(native)?;
    let _input = InputAttachment(current, owner_thread);
    if !alive.load(Ordering::Acquire) {
        return Ok(Outcome::Cancelled);
    }
    let object = data_object(provider.clone())?;
    let source: IDropSource = SourceControl { alive, provider }.into();
    let mut effect = DROPEFFECT_NONE;
    let status = unsafe { DoDragDrop(&object, &source, DROPEFFECT_COPY, &mut effect) };
    if status == DRAGDROP_S_DROP && effect.0 & DROPEFFECT_COPY.0 != 0 {
        Ok(Outcome::Dropped)
    } else if status.is_ok() {
        Ok(Outcome::Cancelled)
    } else {
        Err(native(status.into()))
    }
}
