//! Generic Windows credentials protect plugin secrets without ordinary files or credential enumeration.

/// A fixed host-built target names exactly one credential. It never enumerates another application.
pub struct CredentialStore {
    target: String,
}
impl CredentialStore {
    /// Require an explicit Nanobug namespace and a bounded name; callers hash profile/plugin/key identities.
    pub fn new(target: &str) -> Result<Self, String> {
        if !target.starts_with("Nanobug/")
            || target.len() > 512
            || target.chars().any(char::is_control)
        {
            return Err("Invalid credential namespace".into());
        }
        Ok(Self {
            target: target.into(),
        })
    }
    /// Read a system-protected blob. Missing is None; facility errors never fall back to plaintext.
    pub fn read(&self) -> Result<Option<Vec<u8>>, String> {
        native::read(&self.target)
    }
    /// Replace one system credential, with the platform's Generic Credential size bound.
    pub fn write(&self, bytes: &[u8]) -> Result<(), String> {
        if bytes.is_empty() || bytes.len() > 2560 {
            return Err("Secret must contain 1 to 2560 bytes".into());
        }
        native::write(&self.target, bytes)
    }
    /// Delete only the selected key. Deleting an absent credential is idempotent.
    pub fn delete(&self) -> Result<(), String> {
        native::delete(&self.target)
    }
}

#[cfg(windows)]
mod native {
    use windows::{
        Win32::{Foundation::ERROR_NOT_FOUND, Security::Credentials::*},
        core::{PCWSTR, PWSTR},
    };
    fn target(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }
    fn unavailable() -> String {
        "System credential facility is unavailable; no plaintext fallback".into()
    }
    pub(super) fn read(name: &str) -> Result<Option<Vec<u8>>, String> {
        let name = target(name);
        let mut pointer = std::ptr::null_mut();
        // The OS allocation is always freed, including a malformed credential returned by the facility.
        unsafe {
            if let Err(error) =
                CredReadW(PCWSTR(name.as_ptr()), CRED_TYPE_GENERIC, None, &mut pointer)
            {
                return if error.code() == windows::core::HRESULT::from_win32(ERROR_NOT_FOUND.0) {
                    Ok(None)
                } else {
                    Err(unavailable())
                };
            }
            struct Allocation(*mut CREDENTIALW);
            impl Drop for Allocation {
                fn drop(&mut self) {
                    unsafe {
                        CredFree(self.0.cast());
                    }
                }
            }
            let owned = Allocation(pointer);
            let credential = owned.0.as_ref().ok_or_else(unavailable)?;
            if credential.CredentialBlobSize > 2560
                || (credential.CredentialBlobSize > 0 && credential.CredentialBlob.is_null())
            {
                return Err(unavailable());
            }
            let bytes = if credential.CredentialBlobSize == 0 {
                vec![]
            } else {
                std::slice::from_raw_parts(
                    credential.CredentialBlob,
                    credential.CredentialBlobSize as usize,
                )
                .to_vec()
            };
            Ok(Some(bytes))
        }
    }
    pub(super) fn write(name: &str, bytes: &[u8]) -> Result<(), String> {
        let mut name = target(name);
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(name.as_mut_ptr()),
            CredentialBlobSize: bytes.len() as u32,
            CredentialBlob: bytes.as_ptr().cast_mut(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..Default::default()
        };
        // LOCAL_MACHINE persists for this logged-on user's store; other OS users do not inherit access.
        unsafe { CredWriteW(&credential, 0).map_err(|_| unavailable()) }
    }
    pub(super) fn delete(name: &str) -> Result<(), String> {
        let name = target(name);
        match unsafe { CredDeleteW(PCWSTR(name.as_ptr()), CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(()),
            Err(error) if error.code() == windows::core::HRESULT::from_win32(ERROR_NOT_FOUND.0) => {
                Ok(())
            }
            Err(_) => Err(unavailable()),
        }
    }
}
#[cfg(not(windows))]
mod native {
    pub(super) fn read(_: &str) -> Result<Option<Vec<u8>>, String> {
        Err("System credential storage is unsupported on this platform".into())
    }
    pub(super) fn write(_: &str, _: &[u8]) -> Result<(), String> {
        Err("System credential storage is unsupported on this platform".into())
    }
    pub(super) fn delete(_: &str) -> Result<(), String> {
        Err("System credential storage is unsupported on this platform".into())
    }
}
