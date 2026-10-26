//! Readonly OLE IDataObject presents finite format metadata and renders only explicitly requested data.
use super::*;
use windows::{
    Win32::{
        Foundation::{E_FAIL, E_NOTIMPL, S_OK},
        UI::Shell::SHCreateStdEnumFmtEtc,
    },
    core::{BOOL, HRESULT, Ref, implement},
};

#[implement(IDataObject)]
struct Source {
    provider: Arc<dyn Provider>,
    formats: Vec<(String, u16)>,
}

/// Construct a native delayed source. The caller owns gesture consent and source-instance revocation.
pub fn data_object(provider: Arc<dyn Provider>) -> Result<IDataObject, Error> {
    let names = provider.formats();
    if names.is_empty() || names.len() > 16 {
        return Err(Error::Limit);
    }
    let mut formats = Vec::new();
    for mime in names {
        if formats.iter().any(|(name, _)| name == &mime) {
            return Err(Error::Invalid);
        }
        let id = if mime == "application/vnd.nanobug.files" {
            15
        } else {
            clipboard::format_id(&mime)? as u16
        };
        formats.push((mime, id));
    }
    Ok(Source { provider, formats }.into())
}
impl Source_Impl {
    /// Validate the complete requested medium before invoking any producer code.
    fn selected(&self, request: *const FORMATETC) -> windows::core::Result<&str> {
        let request =
            unsafe { request.as_ref() }.ok_or_else(|| windows::core::Error::from(E_FAIL))?;
        if request.dwAspect != DVASPECT_CONTENT.0
            || request.lindex != -1
            || request.tymed & TYMED_HGLOBAL.0 as u32 == 0
        {
            return Err(E_FAIL.into());
        }
        self.formats
            .iter()
            .find(|(_, id)| *id == request.cfFormat)
            .map(|(name, _)| name.as_str())
            .ok_or_else(|| E_FAIL.into())
    }
}
#[allow(non_snake_case)]
impl IDataObject_Impl for Source_Impl {
    fn GetData(&self, request: *const FORMATETC) -> windows::core::Result<STGMEDIUM> {
        if !self.provider.is_live() {
            return Err(E_FAIL.into());
        }
        let mime = self.selected(request)?;
        let encoded = match self
            .provider
            .read(mime)
            .map_err(|_| windows::core::Error::from(E_FAIL))?
        {
            Data::Bytes(bytes)
                if bytes.len() <= 256 * 1024 && mime != "application/vnd.nanobug.files" =>
            {
                clipboard::encode(mime, &bytes).map_err(|_| windows::core::Error::from(E_FAIL))?
            }
            Data::Files(paths)
                if mime == "application/vnd.nanobug.files"
                    && !paths.is_empty()
                    && paths.len() <= 32 =>
            {
                // DROPFILES has a 20-byte header; fWide=1 and double-NUL UTF-16 paths follow it.
                let mut bytes = vec![0u8; 20];
                bytes[..4].copy_from_slice(&20u32.to_le_bytes());
                bytes[16..20].copy_from_slice(&1u32.to_le_bytes());
                for path in paths {
                    let name = path
                        .to_str()
                        .filter(|name| !name.contains('\0') && name.len() <= 32_768)
                        .ok_or_else(|| windows::core::Error::from(E_FAIL))?;
                    for unit in name.encode_utf16().chain(Some(0)) {
                        bytes.extend_from_slice(&unit.to_le_bytes());
                    }
                }
                bytes.extend_from_slice(&[0, 0]);
                if bytes.len() > 2 * 1024 * 1024 {
                    return Err(E_FAIL.into());
                }
                bytes
            }
            _ => return Err(E_FAIL.into()),
        };
        let memory =
            clipboard::allocate(&encoded).map_err(|_| windows::core::Error::from(E_FAIL))?;
        Ok(STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: STGMEDIUM_0 { hGlobal: memory },
            pUnkForRelease: std::mem::ManuallyDrop::new(None),
        })
    }
    fn QueryGetData(&self, request: *const FORMATETC) -> HRESULT {
        if self.selected(request).is_ok() {
            S_OK
        } else {
            E_FAIL
        }
    }
    fn EnumFormatEtc(&self, direction: u32) -> windows::core::Result<IEnumFORMATETC> {
        if direction != DATADIR_GET.0 as u32 {
            return Err(E_NOTIMPL.into());
        }
        unsafe {
            SHCreateStdEnumFmtEtc(
                &self
                    .formats
                    .iter()
                    .map(|(_, id)| format(*id))
                    .collect::<Vec<_>>(),
            )
        }
    }
    fn GetDataHere(&self, _: *const FORMATETC, _: *mut STGMEDIUM) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn GetCanonicalFormatEtc(&self, _: *const FORMATETC, _: *mut FORMATETC) -> HRESULT {
        E_NOTIMPL
    }
    fn SetData(
        &self,
        _: *const FORMATETC,
        _: *const STGMEDIUM,
        _: BOOL,
    ) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn DAdvise(
        &self,
        _: *const FORMATETC,
        _: u32,
        _: Ref<IAdviseSink>,
    ) -> windows::core::Result<u32> {
        Err(E_NOTIMPL.into())
    }
    fn DUnadvise(&self, _: u32) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn EnumDAdvise(&self) -> windows::core::Result<IEnumSTATDATA> {
        Err(E_NOTIMPL.into())
    }
}
