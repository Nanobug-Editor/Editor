//! Finite OLE offers enumerate only formats; selected worker reads use the COM Global Interface Table.
//! No apartment-bound IDataObject pointer is sent to a worker, and native paths never enter guest data.
use crate::clipboard;
pub use clipboard::Error;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use windows::{
    Win32::{
        Foundation::RPC_E_CHANGED_MODE,
        System::Com::*,
        UI::Shell::{DragQueryFileW, HDROP},
    },
    core::{GUID, Interface},
};
mod source;
pub use source::data_object;
mod lifecycle;
pub use lifecycle::{Event, Outcome, Point, Registration, start};

/// Only the native adapter may carry an OS file list; guests receive controlled imports separately.
pub enum Data {
    /// One encoded inert representation, with a finite byte budget.
    Bytes(Vec<u8>),
    /// User-selected native files; never accepted from serialized plugin data.
    Files(Vec<PathBuf>),
}

/// Delayed source boundary used by OS IDataObject::GetData; enumeration must not call read.
pub trait Provider: Send + Sync {
    /// Source lifetime is checked by the OS loop and again before GetData materialization.
    fn is_live(&self) -> bool {
        true
    }
    /// Finite supported formats, without rendering or loading their data.
    fn formats(&self) -> Vec<String>;
    /// Materialize exactly the requested representation after native consent/selection.
    fn read(&self, mime: &str) -> Result<Data, Error>;
}

/// Balance only this call's successful COM initialization; existing GPUI STA ownership is preserved.
struct Apartment(bool);
impl Apartment {
    fn enter() -> Result<Self, Error> {
        let status = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if status.is_ok() {
            Ok(Self(true))
        } else if status == RPC_E_CHANGED_MODE {
            Ok(Self(false))
        } else {
            Err(native(status.into()))
        }
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

/// Per-process system GIT; each call obtains an interface usable only in its current apartment.
fn git() -> Result<IGlobalInterfaceTable, Error> {
    const CLSID_GIT: GUID = GUID::from_u128(0x00000323_0000_0000_c000_000000000046);
    unsafe { CoCreateInstance(&CLSID_GIT, None, CLSCTX_INPROC_SERVER) }.map_err(native)
}

/// A revocable marshaling cookie is Send/Sync; no foreign apartment pointer is stored here.
pub struct Offer {
    cookie: Mutex<Option<u32>>,
    formats: Vec<(String, u16)>,
}
impl Offer {
    /// Capture finite metadata and register its native source without requesting any data.
    pub fn capture(object: &IDataObject) -> Result<Arc<Self>, Error> {
        let _apartment = Apartment::enter()?;
        let enumerator = unsafe { object.EnumFormatEtc(DATADIR_GET.0 as u32) }.map_err(native)?;
        let mut formats = Vec::new();
        let mut finished = false;
        for _ in 0..256 {
            let mut item = [FORMATETC::default()];
            let mut fetched = 0;
            unsafe { enumerator.Next(&mut item, Some(&mut fetched)) }
                .ok()
                .map_err(native)?;
            if fetched == 0 {
                finished = true;
                break;
            }
            let format = &item[0];
            let mime = if format.cfFormat == 15 {
                Some("application/vnd.nanobug.files".into())
            } else {
                clipboard::mime_name(u32::from(format.cfFormat))
            };
            if !format.ptd.is_null() {
                unsafe { CoTaskMemFree(Some(format.ptd.cast())) };
            }
            if format.dwAspect != DVASPECT_CONTENT.0
                || format.lindex != -1
                || format.tymed & TYMED_HGLOBAL.0 as u32 == 0
            {
                continue;
            }
            if let Some(mime) = mime {
                if !formats.iter().any(|(name, _)| name == &mime) {
                    formats.push((mime, format.cfFormat));
                }
                if formats.len() > 16 {
                    return Err(Error::Limit);
                }
            }
        }
        if !finished || formats.is_empty() {
            return Err(Error::Invalid);
        }
        let cookie = unsafe { git()?.RegisterInterfaceInGlobal(object, &IDataObject::IID) }
            .map_err(native)?;
        Ok(Arc::new(Self {
            cookie: Mutex::new(Some(cookie)),
            formats,
        }))
    }
    /// Unknown lengths stay unknown until selection; names alone never load a representation.
    pub fn formats(&self) -> Vec<&str> {
        self.formats.iter().map(|(name, _)| name.as_str()).collect()
    }
    /// Native liveness can additionally narrow the runtime's instance/target/expiry lease.
    pub fn is_live(&self) -> bool {
        self.cookie.lock().unwrap().is_some()
    }
    /// Revoke GIT registration exactly once, serialized with interface acquisition as required by COM.
    pub fn revoke(&self) {
        let Ok(_apartment) = Apartment::enter() else {
            return;
        };
        let mut cookie = self.cookie.lock().unwrap();
        if let Some(cookie) = cookie.take() {
            if let Ok(git) = git() {
                let _ = unsafe { git.RevokeInterfaceFromGlobal(cookie) };
            }
        }
    }
    /// Read one previously enumerated HGLOBAL representation on the calling initialized apartment.
    pub fn read(&self, mime: &str, limit: usize) -> Result<Data, Error> {
        if limit == 0 || limit > 256 * 1024 {
            return Err(Error::Limit);
        }
        let id = self
            .formats
            .iter()
            .find(|(name, _)| name == mime)
            .map(|(_, id)| *id)
            .ok_or(Error::Invalid)?;
        let _apartment = Apartment::enter()?;
        let object: IDataObject = {
            let cookie = self.cookie.lock().unwrap();
            let cookie = cookie.ok_or(Error::Changed)?;
            let mut pointer = std::ptr::null_mut();
            unsafe { git()?.GetInterfaceFromGlobal(cookie, &IDataObject::IID, &mut pointer) }
                .map_err(native)?;
            unsafe { IDataObject::from_raw(pointer) }
        };
        let mut medium = unsafe { object.GetData(&format(id)) }.map_err(native)?;
        let result = (|| {
            if medium.tymed != TYMED_HGLOBAL.0 as u32 {
                return Err(Error::Invalid);
            }
            let memory = unsafe { medium.u.hGlobal };
            if memory.is_invalid() {
                return Err(Error::Invalid);
            }
            if mime == "application/vnd.nanobug.files" {
                read_files(memory).map(Data::Files)
            } else {
                clipboard::read_memory(memory, mime, limit).map(Data::Bytes)
            }
        })();
        unsafe { windows::Win32::System::Ole::ReleaseStgMedium(&mut medium) };
        if !self.is_live() {
            return Err(Error::Changed);
        }
        result
    }
}
impl Drop for Offer {
    fn drop(&mut self) {
        self.revoke();
    }
}

/// All supported OS formats use one complete HGLOBAL value, with no device/stream object execution.
fn format(id: u16) -> FORMATETC {
    FORMATETC {
        cfFormat: id,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    }
}
/// Bound native list storage, count and UTF-16 filenames before allocating path buffers.
fn read_files(memory: windows::Win32::Foundation::HGLOBAL) -> Result<Vec<PathBuf>, Error> {
    if unsafe { windows::Win32::System::Memory::GlobalSize(memory) } > 2 * 1024 * 1024 {
        return Err(Error::Limit);
    }
    let drop = HDROP(memory.0);
    let count = unsafe { DragQueryFileW(drop, u32::MAX, None) };
    if count == 0 || count > 32 {
        return Err(Error::Limit);
    }
    (0..count)
        .map(|index| {
            let length = unsafe { DragQueryFileW(drop, index, None) } as usize;
            if length == 0 || length > 32_768 {
                return Err(Error::Limit);
            }
            let mut name = vec![0; length + 1];
            if unsafe { DragQueryFileW(drop, index, Some(&mut name)) } as usize != length {
                return Err(Error::Invalid);
            }
            String::from_utf16(&name[..length])
                .map(PathBuf::from)
                .map_err(|_| Error::Invalid)
        })
        .collect()
}
fn native(error: windows::core::Error) -> Error {
    Error::Unavailable(error.code().to_string())
}
