//! Windows distributions launch as native GUI applications and protect running editors from replacement.

#[cfg(target_os = "windows")]
pub(crate) use windows::attach_parent_console;

/// Retain the Windows installer mutex until the last GUI process exits.
/// This guard signals installation safety, and does not impose a single-instance application policy.
pub(crate) struct InstallationGuard {
    #[cfg(target_os = "windows")]
    handle: *mut std::ffi::c_void,
}

/// Mark the GUI as running, returning the native OS error if Windows cannot create its mutex.
/// CLI SDK/export commands deliberately do not acquire this guard. Other platforms need no mutex.
pub(crate) fn installation_guard() -> std::io::Result<InstallationGuard> {
    #[cfg(target_os = "windows")]
    {
        windows::guard("Local\\Nanobug.Installer.Running")
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(InstallationGuard {})
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::InstallationGuard;
    use std::ffi::c_void;

    // Kernel object handles are process-owned. Keeping this tiny binding local avoids a new dependency.
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn AttachConsole(process: u32) -> i32;
        fn CreateMutexW(attributes: *const c_void, owner: i32, name: *const u16) -> *mut c_void;
        fn CloseHandle(handle: *mut c_void) -> i32;
        #[cfg(test)]
        fn OpenMutexW(access: u32, inherit: i32, name: *const u16) -> *mut c_void;
    }

    /// Reuse a terminal caller's existing console before CLI I/O, without allocating a console window.
    /// Explorer/installer launches have no parent console; failure (or an existing attachment) is expected.
    /// Caller-supplied redirected standard handles remain governed by Windows' STARTF_USESTDHANDLES.
    pub(crate) fn attach_parent_console() {
        // ATTACH_PARENT_PROCESS is a process-id sentinel; no pointers or owned handles cross this call.
        let _ = unsafe { AttachConsole(u32::MAX) };
    }

    /// Open or create a named mutex without taking ownership; its existence is the installer signal.
    pub(super) fn guard(name: &str) -> std::io::Result<InstallationGuard> {
        let wide: Vec<_> = name.encode_utf16().chain([0]).collect();
        // The string is NUL-terminated and lives across the call; null attributes request OS defaults.
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, wide.as_ptr()) };
        if handle.is_null() {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(InstallationGuard { handle })
        }
    }

    impl Drop for InstallationGuard {
        fn drop(&mut self) {
            // Successful construction owns one valid handle, closed exactly once by this guard.
            unsafe { CloseHandle(self.handle) };
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Probe with SYNCHRONIZE access and release the probe handle immediately.
        fn exists(name: &str) -> bool {
            let wide: Vec<_> = name.encode_utf16().chain([0]).collect();
            // The probe uses the same lifetime guarantees as creation and never owns the mutex.
            let handle = unsafe { OpenMutexW(0x0010_0000, 0, wide.as_ptr()) };
            if handle.is_null() {
                false
            } else {
                unsafe { CloseHandle(handle) };
                true
            }
        }

        #[test]
        fn distribution_mutex_survives_until_last_gui_guard_drops() {
            let name = format!("Local\\Nanobug.Installer.Test.{}", std::process::id());
            let first = guard(&name).unwrap();
            let second = guard(&name).unwrap();
            assert!(exists(&name));
            drop(first);
            assert!(exists(&name));
            drop(second);
            assert!(!exists(&name));
        }
    }
}
