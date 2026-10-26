//! Win32 multi-format clipboard adaptation. HTML is copied as inert bytes with CF_HTML offsets;
//! custom binary formats use a length envelope so GlobalAlloc padding never changes their payload.
//! Enumeration calls only EnumClipboardFormats/GetClipboardFormatName, never GetClipboardData.
use windows::{
    Win32::{
        Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND},
        System::{
            DataExchange::*,
            Memory::{
                GMEM_MOVEABLE, GMEM_ZEROINIT, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
            },
        },
    },
    core::PCWSTR,
};

/// Native failures remain local to this request; no clipboard data or paths appear in diagnostics.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Native clipboard is unavailable: {0}")]
    Unavailable(String),
    #[error("Clipboard changed after enumeration")]
    Changed,
    #[error("Clipboard format or encoded payload is invalid")]
    Invalid,
    #[error("Clipboard format or byte quota exceeded")]
    Limit,
}

/// Metadata-only sequence snapshot; unknown lengths remain unknown until the selected read.
pub struct Snapshot {
    pub sequence: u64,
    pub formats: Vec<String>,
}

/// A clipboard lock is held only for one metadata/read/write operation and is always released.
struct Open;
impl Open {
    fn new(owner: isize) -> Result<Self, Error> {
        unsafe { OpenClipboard(Some(HWND(owner as *mut _))) }.map_err(native)?;
        Ok(Self)
    }
}
impl Drop for Open {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}

/// Prepared global storage is freed until SetClipboardData explicitly transfers OS ownership.
struct Memory(HGLOBAL);
impl Memory {
    fn new(bytes: &[u8]) -> Result<Self, Error> {
        let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, bytes.len().max(1)) }
            .map_err(native)?;
        let memory = Self(handle);
        let pointer = unsafe { GlobalLock(handle) };
        if pointer.is_null() {
            return Err(Error::Unavailable("GlobalLock failed".into()));
        }
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer.cast(), bytes.len());
            let _ = GlobalUnlock(handle);
        }
        Ok(memory)
    }
    fn publish(mut self, format: u32) -> Result<(), Error> {
        unsafe { SetClipboardData(format, Some(HANDLE(self.0.0))) }.map_err(native)?;
        self.0 = HGLOBAL::default();
        Ok(())
    }
}
impl Drop for Memory {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            let _ = unsafe { GlobalFree(Some(self.0)) };
        }
    }
}

/// Enumerate only recognized inert formats. At most 16 names and 256 OS entries are inspected.
pub fn enumerate(owner: isize) -> Result<Snapshot, Error> {
    let _open = Open::new(owner)?;
    let sequence = unsafe { GetClipboardSequenceNumber() } as u64;
    let mut formats = Vec::new();
    let mut format = 0;
    for _ in 0..256 {
        format = unsafe { EnumClipboardFormats(format) };
        if format == 0 {
            return Ok(Snapshot { sequence, formats });
        }
        if let Some(name) = mime_name(format) {
            if !formats.contains(&name) {
                formats.push(name);
            }
            if formats.len() > 16 {
                return Err(Error::Limit);
            }
        }
    }
    Err(Error::Limit)
}

/// Load exactly one requested format under the original sequence and a finite decoded byte limit.
pub fn read(owner: isize, sequence: u64, mime: &str, limit: usize) -> Result<Vec<u8>, Error> {
    let _open = Open::new(owner)?;
    if unsafe { GetClipboardSequenceNumber() } as u64 != sequence {
        return Err(Error::Changed);
    }
    let format = format_id(mime)?;
    let handle = unsafe { GetClipboardData(format) }.map_err(native)?;
    let memory = HGLOBAL(handle.0);
    let bytes = read_memory(memory, mime, limit)?;
    if unsafe { GetClipboardSequenceNumber() } as u64 != sequence {
        return Err(Error::Changed);
    }
    Ok(bytes)
}

/// Decode owned/borrowed HGLOBAL contents identically for clipboard and OLE without taking ownership.
pub(crate) fn read_memory(memory: HGLOBAL, mime: &str, limit: usize) -> Result<Vec<u8>, Error> {
    let length = unsafe { GlobalSize(memory) };
    // UTF-16 can use twice the UTF-8 byte budget; HTML adds a small bounded ASCII header.
    let native_limit = limit
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(4096))
        .ok_or(Error::Limit)?;
    if length == 0 || length > native_limit || limit > 1024 * 1024 {
        return Err(Error::Limit);
    }
    let pointer = unsafe { GlobalLock(memory) };
    if pointer.is_null() {
        return Err(Error::Unavailable("GlobalLock failed".into()));
    }
    let result = (|| {
        let bytes = unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), length) };
        match mime {
            "text/plain;charset=utf-8" => {
                if length % 2 != 0 {
                    return Err(Error::Invalid);
                }
                let units =
                    unsafe { std::slice::from_raw_parts(pointer.cast::<u16>(), length / 2) };
                let end = units
                    .iter()
                    .position(|unit| *unit == 0)
                    .ok_or(Error::Invalid)?;
                String::from_utf16(&units[..end])
                    .map(String::into_bytes)
                    .map_err(|_| Error::Invalid)
            }
            "text/html" => html_fragment(bytes).map(|bytes| bytes.to_vec()),
            "image/png" => png_bytes(bytes).map(|bytes| bytes.to_vec()),
            _ => {
                if !bytes.starts_with(b"NBX1") || bytes.len() < 8 {
                    return Err(Error::Invalid);
                }
                let length = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
                bytes
                    .get(8..8usize.checked_add(length).ok_or(Error::Limit)?)
                    .map(<[u8]>::to_vec)
                    .ok_or(Error::Invalid)
            }
        }
    })();
    let _ = unsafe { GlobalUnlock(memory) };
    let bytes = result?;
    if bytes.len() > limit {
        return Err(Error::Limit);
    }
    Ok(bytes)
}

/// Prepare every representation before replacing the clipboard. The OS lock prevents interleaving;
/// a native SetClipboardData error is reported explicitly rather than claiming a complete write.
pub fn write(owner: isize, items: &[(String, Vec<u8>)], limit: usize) -> Result<(), Error> {
    if owner == 0
        || items.is_empty()
        || items.len() > 16
        || items.iter().map(|(_, bytes)| bytes.len()).sum::<usize>() > limit
        || limit > 1024 * 1024
    {
        return Err(Error::Limit);
    }
    let mut prepared = Vec::new();
    for (mime, bytes) in items {
        let format = format_id(mime)?;
        let encoded = encode(mime, bytes)?;
        prepared.push((format, Memory::new(&encoded)?));
    }
    let _open = Open::new(owner)?;
    unsafe { EmptyClipboard() }.map_err(native)?;
    for (format, memory) in prepared {
        memory.publish(format)?;
    }
    Ok(())
}

/// Both OS transports use the same safe native representation and length envelope.
pub(crate) fn encode(mime: &str, bytes: &[u8]) -> Result<Vec<u8>, Error> {
    Ok(match mime {
        "text/plain;charset=utf-8" => {
            let text = std::str::from_utf8(bytes).map_err(|_| Error::Invalid)?;
            if text.contains('\0') {
                return Err(Error::Invalid);
            }
            text.encode_utf16()
                .chain(Some(0))
                .flat_map(u16::to_le_bytes)
                .collect()
        }
        "text/html" => encode_html(bytes)?,
        "image/png" => {
            png_bytes(bytes)?;
            bytes.to_vec()
        }
        _ => {
            let mut output = b"NBX1".to_vec();
            output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            output.extend_from_slice(bytes);
            output
        }
    })
}

/// Transfer one prepared allocation to OLE's STGMEDIUM, whose receiver must ReleaseStgMedium.
pub(crate) fn allocate(bytes: &[u8]) -> Result<HGLOBAL, Error> {
    let mut memory = Memory::new(bytes)?;
    let raw = memory.0;
    memory.0 = HGLOBAL::default();
    Ok(raw)
}

/// Preserve UTF-8 byte offsets required by the registered Windows HTML Format.
fn encode_html(fragment: &[u8]) -> Result<Vec<u8>, Error> {
    std::str::from_utf8(fragment).map_err(|_| Error::Invalid)?;
    let prefix = b"<html><body><!--StartFragment-->";
    let suffix = b"<!--EndFragment--></body></html>";
    let header = |start_html: usize, end_html: usize, start: usize, end: usize| {
        format!(
            "Version:1.0\r\nStartHTML:{start_html:010}\r\nEndHTML:{end_html:010}\r\nStartFragment:{start:010}\r\nEndFragment:{end:010}\r\n"
        )
    };
    let start_html = header(0, 0, 0, 0).len();
    let start = start_html + prefix.len();
    let end = start + fragment.len();
    let end_html = end + suffix.len();
    let mut bytes = header(start_html, end_html, start, end).into_bytes();
    bytes.extend_from_slice(prefix);
    bytes.extend_from_slice(fragment);
    bytes.extend_from_slice(suffix);
    bytes.push(0);
    Ok(bytes)
}

/// Extract declared byte offsets without parsing, rendering or following anything in HTML.
fn html_fragment(bytes: &[u8]) -> Result<&[u8], Error> {
    let header = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
    let offset = |name: &str| {
        header
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .and_then(|value| value.trim().parse::<usize>().ok())
    };
    let start = offset("StartFragment:").ok_or(Error::Invalid)?;
    let end = offset("EndFragment:").ok_or(Error::Invalid)?;
    let fragment = bytes.get(start..end).ok_or(Error::Invalid)?;
    std::str::from_utf8(fragment).map_err(|_| Error::Invalid)?;
    Ok(fragment)
}

/// Bound PNG chunks and trim only native memory padding; native image admission checks decoded pixels.
fn png_bytes(bytes: &[u8]) -> Result<&[u8], Error> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(Error::Invalid);
    }
    let mut offset = 8usize;
    for _ in 0..16_384 {
        let header = bytes.get(offset..offset + 8).ok_or(Error::Invalid)?;
        let length = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
        let end = offset
            .checked_add(length)
            .and_then(|end| end.checked_add(12))
            .filter(|end| *end <= bytes.len())
            .ok_or(Error::Invalid)?;
        if &header[4..8] == b"IEND" {
            return (length == 0).then_some(&bytes[..end]).ok_or(Error::Invalid);
        }
        offset = end;
    }
    Err(Error::Limit)
}

/// Win32 predefined Unicode text and registered safe formats use one common MIME mapping.
pub(crate) fn format_id(mime: &str) -> Result<u32, Error> {
    if mime == "text/plain;charset=utf-8" {
        return Ok(13);
    }
    let name = match mime {
        "text/html" => "HTML Format",
        "image/png" => "PNG",
        name if custom_name(name) => name,
        _ => return Err(Error::Invalid),
    };
    let wide = name.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let id = unsafe { RegisterClipboardFormatW(PCWSTR(wide.as_ptr())) };
    if id == 0 {
        Err(Error::Unavailable("Format registration failed".into()))
    } else {
        Ok(id)
    }
}

/// Query names only; this path deliberately never obtains clipboard payload handles.
pub(crate) fn mime_name(format: u32) -> Option<String> {
    if format == 13 {
        return Some("text/plain;charset=utf-8".into());
    }
    let mut name = [0u16; 256];
    let length = unsafe { GetClipboardFormatNameW(format, &mut name) };
    let name = String::from_utf16(name.get(..usize::try_from(length).ok()?)?).ok()?;
    match name.as_str() {
        "HTML Format" => Some("text/html".into()),
        "PNG" => Some("image/png".into()),
        name if custom_name(name) => Some(name.into()),
        _ => None,
    }
}

/// Custom native formats never select predefined Win32 object or path representations.
fn custom_name(name: &str) -> bool {
    name.len() <= 192
        && name
            .strip_prefix("application/vnd.nanobug.")
            .is_some_and(|suffix| {
                suffix.contains('.')
                    && !suffix.contains("..")
                    && !suffix.starts_with('.')
                    && !suffix.ends_with('.')
                    && suffix.bytes().all(|byte| {
                        byte.is_ascii_lowercase()
                            || byte.is_ascii_digit()
                            || matches!(byte, b'.' | b'-' | b'_')
                    })
            })
}
fn native(error: windows::core::Error) -> Error {
    Error::Unavailable(error.code().to_string())
}
