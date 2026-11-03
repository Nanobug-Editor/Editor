//! A copied native helper waits for normal GUI exit, verifies the package again, then installs it.

use super::{
    download::{Prepared, read_small_file, save_json},
    release::Edition,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// One explicit install request binds the package, original executable and owning user profile.
#[derive(Deserialize, Serialize)]
struct Job {
    /// Process whose acknowledged shutdown authorizes this application replacement.
    parent_pid: u32,
    /// User-selected GUI profile, preserved when reopening after installation.
    profile: PathBuf,
    /// Original installation executable; the helper runs from private staging instead.
    executable: PathBuf,
    /// Previously verified package; integrity is checked again immediately before application.
    prepared: Prepared,
    /// Inno's native wizard/progress language follows the user's current UI locale.
    chinese: bool,
}

/// The next GUI startup shows installation failure without losing its ready package or retry path.
#[derive(Deserialize, Serialize)]
pub(super) struct Outcome {
    pub(super) version: String,
    pub(super) error: Option<String>,
}

/// Prepare a native handoff only at the successful end of the existing application quit coordinator.
pub(super) fn handoff(profile: &Path, executable: &Path, prepared: Prepared) -> Result<()> {
    prepared.verify(profile, Edition::detect(executable))?;
    ensure!(
        prepared.edition != Edition::Browse,
        "This build does not support in-place updates"
    );
    let helper = prepared.directory.join("update-helper.exe");
    ensure!(
        super::download::file_digest(&helper)? == super::download::file_digest(executable)?,
        "Update helper integrity changed"
    );
    let job = Job {
        parent_pid: std::process::id(),
        profile: profile.to_owned(),
        executable: executable.to_owned(),
        prepared,
        chinese: rust_i18n::locale().starts_with("zh"),
    };
    let path = job.prepared.directory.join("apply.json");
    save_json(&path, &job)?;
    let mut command = Command::new(helper);
    command
        .arg("--apply-host-update")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        command.creation_flags(0x08000000);
    }
    command
        .spawn()
        .context("Could not start the host update helper")?;
    Ok(())
}

/// Recognize the private helper mode before GUI admission, plugin CLI probes or installer mutex creation.
pub(crate) fn run_cli() -> Result<bool> {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--apply-host-update")) {
        return Ok(false);
    }
    let path = PathBuf::from(arguments.next().context("Missing host update job")?);
    ensure!(
        arguments.next().is_none(),
        "Unexpected host update arguments"
    );
    run_job(&path)?;
    Ok(true)
}

/// Execute only the current helper's own job, retain a readable result, and reopen the selected profile.
fn run_job(path: &Path) -> Result<()> {
    let job: Job = serde_json::from_slice(&read_small_file(path, 128 * 1024)?)?;
    ensure!(
        path.file_name() == Some(std::ffi::OsStr::new("apply.json")),
        "Unexpected update job filename"
    );
    ensure!(
        path.canonicalize()?.parent() == Some(job.prepared.directory.canonicalize()?.as_path()),
        "Unexpected update job location"
    );
    ensure!(
        std::env::current_exe()?.canonicalize()?
            == job
                .prepared
                .directory
                .join("update-helper.exe")
                .canonicalize()?,
        "Host update job belongs to another helper"
    );
    ensure!(
        job.executable.is_absolute() && job.profile.is_absolute(),
        "Update paths must be absolute"
    );
    let result = apply(&job);
    let outcome = Outcome {
        version: job.prepared.release.version.clone(),
        error: result.as_ref().err().map(|error| format!("{error:#}")),
    };
    save_json(&job.profile.join("updates/result.json"), &outcome)?;
    if result.is_ok() {
        let _ = fs::remove_file(job.profile.join("updates/ready.json"));
    }
    let mut command = Command::new(&job.executable);
    command
        .arg("--profile")
        .arg(&job.profile)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
        .spawn()
        .context("Could not reopen Nanobug after host update")?;
    result
}

/// Keep waiting and verification ahead of all application writes; the installer never force-closes editors.
fn apply(job: &Job) -> Result<()> {
    #[cfg(not(windows))]
    anyhow::bail!("Native host installation is currently supported on Windows only");
    #[cfg(windows)]
    {
        // Publish the apply lease before waiting for the old GUI lease to disappear. New GUI
        // admission creates its running lease before checking ours, closing the startup race.
        let _guard = crate::app::distribution::update_guard()?;
        wait_for_exit(job.parent_pid)?;
        job.prepared
            .verify(&job.profile, Edition::detect(&job.executable))?;
        let directory = job
            .executable
            .parent()
            .context("Missing application directory")?;
        match job.prepared.edition {
            Edition::Installed => {
                let status = Command::new(job.prepared.package()?)
                    .args([
                        "/SILENT",
                        "/SP-",
                        "/NORESTART",
                        "/NOCLOSEAPPLICATIONS",
                        "/NORESTARTAPPLICATIONS",
                    ])
                    .arg(format!("/DIR={}", directory.display()))
                    .arg(format!(
                        "/LOG={}",
                        job.prepared.directory.join("installer.log").display()
                    ))
                    .arg(if job.chinese {
                        "/LANG=chinesesimplified"
                    } else {
                        "/LANG=english"
                    })
                    .status()?;
                ensure!(status.success(), "Nanobug installer returned {status}");
            }
            Edition::Portable => super::portable::apply(
                &job.prepared.package()?,
                directory,
                &job.prepared.directory,
            )?,
            Edition::Browse => anyhow::bail!("This build does not support in-place updates"),
        }
        Ok(())
    }
}

/// Wait on the original process handle, then allow other profile instances one minute to exit normally.
#[cfg(windows)]
fn wait_for_exit(pid: u32) -> Result<()> {
    use std::{
        ffi::c_void,
        time::{Duration, Instant},
    };
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, id: u32) -> *mut c_void;
        fn WaitForSingleObject(handle: *mut c_void, timeout: u32) -> u32;
        fn OpenMutexW(access: u32, inherit: i32, name: *const u16) -> *mut c_void;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }
    // SYNCHRONIZE grants no process modification rights. The handle remains stable across PID reuse.
    let process = unsafe { OpenProcess(0x00100000, 0, pid) };
    if !process.is_null() {
        let result = unsafe { WaitForSingleObject(process, 120_000) };
        unsafe { CloseHandle(process) };
        ensure!(result == 0, "Nanobug did not finish shutting down");
    } else {
        ensure!(
            std::io::Error::last_os_error().raw_os_error() == Some(87),
            "Could not observe Nanobug shutdown"
        );
    }
    let name: Vec<u16> = "Local\\Nanobug.Installer.Running"
        .encode_utf16()
        .chain([0])
        .collect();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let mutex = unsafe { OpenMutexW(0x00100000, 0, name.as_ptr()) };
        if mutex.is_null() {
            ensure!(
                std::io::Error::last_os_error().raw_os_error() == Some(2),
                "Could not inspect running Nanobug instances"
            );
            return Ok(());
        }
        unsafe { CloseHandle(mutex) };
        ensure!(
            Instant::now() < deadline,
            "Other Nanobug instances are still running; close them and retry the update"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}
