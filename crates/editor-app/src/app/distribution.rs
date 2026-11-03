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
        let guard = windows::guard("Local\\Nanobug.Installer.Running")?;
        windows::ensure_absent("Local\\Nanobug.Update.Applying")?;
        Ok(guard)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(InstallationGuard {})
    }
}

/// Hold a separate update lease while replacing files; GUI admission rejects this active lease.
#[cfg(windows)]
pub(crate) fn update_guard() -> std::io::Result<InstallationGuard> {
    windows::exclusive_guard("Local\\Nanobug.Update.Applying")
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

    /// Create a presence lease only if no updater already holds one; existing owners are untouched.
    pub(super) fn exclusive_guard(name: &str) -> std::io::Result<InstallationGuard> {
        let wide: Vec<_> = name.encode_utf16().chain([0]).collect();
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, wide.as_ptr()) };
        // Read GetLastError before freeing allocations or calling any other native function.
        let error = std::io::Error::last_os_error();
        if handle.is_null() {
            return Err(error);
        }
        let guard = InstallationGuard { handle };
        if error.raw_os_error() == Some(183) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "A Nanobug update is already running",
            ));
        }
        Ok(guard)
    }

    /// Test a presence lease without acquiring ownership or keeping it alive after this check.
    pub(super) fn ensure_absent(name: &str) -> std::io::Result<()> {
        let wide: Vec<_> = name.encode_utf16().chain([0]).collect();
        let handle = unsafe { OpenMutexW(0x00100000, 0, wide.as_ptr()) };
        if handle.is_null() {
            let error = std::io::Error::last_os_error();
            return if error.raw_os_error() == Some(2) {
                Ok(())
            } else {
                Err(error)
            };
        }
        unsafe { CloseHandle(handle) };
        Err(std::io::Error::new(
            std::io::ErrorKind::WouldBlock,
            "Nanobug is applying an update; retry after installation finishes",
        ))
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

        /// A second updater and new GUI admission cannot cross an active replacement lease.
        #[test]
        fn host_update_lease_blocks_other_updaters_and_admission() {
            let name = format!("Local\\Nanobug.Update.Test.{}", std::process::id());
            let lease = exclusive_guard(&name).unwrap();
            assert!(exclusive_guard(&name).is_err());
            assert!(ensure_absent(&name).is_err());
            drop(lease);
            ensure_absent(&name).unwrap();
        }
    }
}
