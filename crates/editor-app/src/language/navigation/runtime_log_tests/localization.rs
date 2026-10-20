//! Real retry transitions retain the selected UI language and the affected owner/project evidence.
use super::*;

/// Restore the process locale even if an assertion fails; run this native fixture serially.
struct RestoreLocale(String);
impl Drop for RestoreLocale {
    fn drop(&mut self) {
        rust_i18n::set_locale(&self.0);
    }
}

/// Actual failed startup, successful reconnect and exhausted retries must use the same locale as
/// their native status row. A temporary resource-only Package keeps failures out of other projects.
#[test]
#[ignore = "build plugin-runtime --example lsp_fixture first; run --test-threads=1 for process locale"]
fn native_lsp_status_and_retry_refusals_follow_the_selected_ui_locale() {
    let _restore = RestoreLocale(rust_i18n::locale().to_string());
    for (locale, waiting, recovered, paused, backoff) in [
        (
            "en",
            "Waiting for a limited retry",
            "Language service recovered",
            "Automatic retries paused",
            "retry in",
        ),
        (
            "zh-CN",
            "等待有限重试",
            "语言服务已恢复",
            "自动重试已暂停",
            "毫秒后重试",
        ),
    ] {
        rust_i18n::set_locale(locale);
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("crash");
        std::fs::write(&marker, "fail the actual startup").unwrap();
        let (mut manager, server) = installed_fixture(directory.path(), Some(&marker));
        assert_eq!(server.recovery_generation(), 0);
        assert!(server.prepare_at(Duration::ZERO).is_err());
        assert_eq!(server.recovery_generation(), 1);
        let status = server.recovery_status().unwrap();
        assert!(
            status.contains(waiting),
            "{locale} waiting status: {status}"
        );
        assert!(status.contains(OWNER) && status.contains("1/3"), "{status}");
        assert!(
            status.contains(&editor_core::display_path(directory.path()).to_string()),
            "{status}"
        );
        let refusal = format!("{:#}", server.prepare_at(Duration::ZERO).unwrap_err());
        assert!(
            refusal.contains(backoff),
            "{locale} retry refusal: {refusal}"
        );

        std::fs::remove_file(&marker).unwrap();
        server.prepare_at(Duration::from_secs(2)).unwrap();
        assert_eq!(
            server.recovery_generation(),
            1,
            "a successful reconnect cannot hide a missed failure"
        );
        let status = server.recovery_status().unwrap();
        assert!(
            status.contains(recovered),
            "{locale} recovered status: {status}"
        );

        std::fs::write(&marker, "fail the next live request").unwrap();
        assert!(
            server
                .definitions(
                    file_uri(&directory.path().join("source.unfamiliar")).unwrap(),
                    "unchanged".into(),
                    Position::default()
                )
                .is_err()
        );
        assert!(server.prepare_at(Duration::from_secs(8)).is_err());
        assert_eq!(server.recovery_state(), Some(RecoveryState::Paused));
        assert_eq!(server.recovery_generation(), 3);
        let status = server.recovery_status().unwrap();
        assert!(
            status.contains(paused) && status.contains("3/3"),
            "{locale}: {status}"
        );
        let refusal = format!(
            "{:#}",
            server.prepare_at(Duration::from_secs(20)).unwrap_err()
        );
        assert!(
            refusal.contains(paused),
            "{locale} paused refusal: {refusal}"
        );
        manager.disable(OWNER).unwrap();
        manager.shutdown_checked().unwrap();
        assert_eq!(server.service.process_count(), 0);
    }
}
