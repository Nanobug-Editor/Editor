//! Only synthetic values in a unique system-credential namespace are used by this native test.
#[cfg(windows)]
#[test]
fn system_credentials_round_trip_replace_delete_and_isolate() {
    use platform_windows::credentials::CredentialStore;
    // A native UUID identifies only this synthetic facility test; no user credential target is guessed.
    let id = format!("{:?}", unsafe {
        windows::Win32::System::Com::CoCreateGuid().unwrap()
    });
    let first = CredentialStore::new(&format!("Nanobug/qa/{id}/first")).unwrap();
    let second = CredentialStore::new(&format!("Nanobug/qa/{id}/second")).unwrap();
    struct Cleanup(CredentialStore);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = self.0.delete();
        }
    }
    let cleanup = Cleanup(first);
    assert_eq!(cleanup.0.read().unwrap(), None);
    cleanup.0.write(b"fake-test-key-1").unwrap();
    assert_eq!(cleanup.0.read().unwrap().unwrap(), b"fake-test-key-1");
    assert_eq!(second.read().unwrap(), None);
    cleanup.0.write(b"fake-test-key-2").unwrap();
    assert_eq!(cleanup.0.read().unwrap().unwrap(), b"fake-test-key-2");
    cleanup.0.delete().unwrap();
    assert_eq!(cleanup.0.read().unwrap(), None);
}
