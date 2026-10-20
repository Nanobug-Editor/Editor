//! OS-observed fault injection is limited to a known acceptance child and its exact descendants.
use super::*;
use serde::Deserialize;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::os::windows::process::CommandExt as _;
use std::process::{Child, Stdio};

/// Creation identity prevents a later PID from being treated as the original owned service.
#[derive(Clone, Deserialize)]
pub(super) struct Owned {
    #[serde(rename = "ProcessId")]
    pid: u32,
    #[serde(rename = "ParentProcessId")]
    parent: u32,
    #[serde(rename = "CommandLine")]
    command: Option<String>,
    #[serde(rename = "Created")]
    created: String,
}

/// Query only an exact owned parent, with hidden direct PowerShell execution and no user command.
fn children(parent: u32) -> anyhow::Result<Vec<Owned>> {
    ensure!(parent > 0, "owned parent PID missing");
    let output = powershell(&format!(
        "Get-CimInstance Win32_Process -Filter 'ParentProcessId = {parent}' | Select-Object ProcessId,ParentProcessId,CommandLine,@{{Name='Created';Expression={{$_.CreationDate.ToUniversalTime().Ticks.ToString()}}}} | ConvertTo-Json -Compress"
    ))?;
    if output.trim().is_empty() {
        return Ok(Vec::new());
    }
    let value: serde_json::Value = serde_json::from_str(&output)?;
    let records: Vec<Owned> = if value.is_array() {
        serde_json::from_value(value)?
    } else {
        vec![serde_json::from_value(value)?]
    };
    ensure!(
        records.iter().all(|child: &Owned| child.parent == parent),
        "OS parent identity changed"
    );
    Ok(records)
}

/// At this point the actual scenario has opened only A and has proved its adapted Vue semantics.
pub(super) fn owned_outer() -> anyhow::Result<Owned> {
    let workers = children(std::process::id())?
        .into_iter()
        .filter(|child| {
            child
                .command
                .as_ref()
                .is_some_and(|line| line.replace('\\', "/").contains("native/main.mjs"))
        })
        .collect::<Vec<_>>();
    let mut admitted = Vec::new();
    for worker in workers {
        if children(worker.pid)?.iter().any(|child| {
            child
                .command
                .as_ref()
                .is_some_and(|line| line.contains("engine-bootstrap.cjs"))
        }) {
            admitted.push(worker);
        }
    }
    ensure!(
        admitted.len() == 1,
        "first source must own exactly one adapted stdio worker, found {}",
        admitted.len()
    );
    let worker = admitted.pop().unwrap();
    eprintln!("inner Vue failure owned outer PID {}", worker.pid);
    Ok(worker)
}

/// Revalidate the original parent/creation identity before touching one fully admitted adapter.
pub(super) fn terminate_adapter(owner: &Owned) -> anyhow::Result<()> {
    ensure!(
        owner.parent == std::process::id(),
        "foreign test process owner"
    );
    ensure!(
        children(owner.parent)?.iter().any(|current| {
            current.pid == owner.pid
                && current.created == owner.created
                && current.command == owner.command
        }),
        "original owned stdio worker disappeared"
    );
    let candidates = children(owner.pid)?
        .into_iter()
        .filter(|child| {
            child
                .command
                .as_ref()
                .is_some_and(|line| line.contains("engine-bootstrap.cjs"))
        })
        .collect::<Vec<_>>();
    ensure!(
        candidates.len() == 1,
        "exact owned admitted adapter missing, found {}",
        candidates.len()
    );
    let inner = &candidates[0];
    let created: u64 = inner
        .created
        .parse()
        .context("invalid owned child creation timestamp")?;
    ensure!(
        inner.pid != owner.pid && inner.pid != std::process::id(),
        "invalid inner process identity"
    );
    // The fixed numeric filters are rechecked in the same hidden process that terminates the leaf;
    // no tree kill, executable-name kill or project-supplied command is used.
    powershell(&format!(
        "$victim = [System.Diagnostics.Process]::GetProcessById({}); try {{ $null = $victim.Handle; $owned = @(Get-CimInstance Win32_Process -Filter 'ProcessId = {} AND ParentProcessId = {}'); if ($owned.Count -ne 1 -or $owned[0].CommandLine -notlike '*engine-bootstrap.cjs*' -or $owned[0].CreationDate.ToUniversalTime().Ticks -ne {created}) {{ throw 'Owned adapter changed' }}; $victim.Kill(); if (-not $victim.WaitForExit(5000)) {{ throw 'Owned adapter did not exit' }} }} finally {{ $victim.Dispose() }}",
        inner.pid, inner.pid, owner.pid
    ))?;
    eprintln!(
        "inner Vue failure killed adapter PID {} only; outer PID {} retained for natural EOF",
        inner.pid, owner.pid
    );
    Ok(())
}

/// A typed error from a permanently closed engine is insufficient: the managed boundary must end.
pub(super) fn wait_retired(owner: &Owned) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while children(owner.parent)?
        .iter()
        .any(|current| current.pid == owner.pid && current.created == owner.created)
    {
        ensure!(
            Instant::now() < deadline,
            "admitted adapter died but original stdio worker {} stayed alive",
            owner.pid
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    Ok(())
}

/// Every exit path owns and reaps its exact helper; a background job timeout cannot do that for us.
struct Helper(Child);
impl Drop for Helper {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Fixed scripts run outside the UI thread, with a real child deadline and bounded file output.
/// Separate private files avoid pipe backpressure; overflow, wait failure and timeout all reap
/// the owned helper through `Helper`, even when the surrounding async job has stopped observing it.
fn powershell(script: &str) -> anyhow::Result<String> {
    const MAX_OUT: u64 = 256 * 1024;
    const MAX_ERR: u64 = 16 * 1024;
    let program =
        PathBuf::from(std::env::var_os("SystemRoot").context("Windows directory unavailable")?)
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    let mut helper = Helper(
        Command::new(program)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                script,
            ])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW: the test owns only its original GPUI HWND.
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout.try_clone()?))
            .stderr(Stdio::from(stderr.try_clone()?))
            .spawn()?,
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        ensure!(
            stdout.metadata()?.len() <= MAX_OUT && stderr.metadata()?.len() <= MAX_ERR,
            "owned process observation exceeded bound"
        );
        if let Some(status) = helper.0.try_wait()? {
            break status;
        }
        ensure!(Instant::now() < deadline, "owned process helper timed out");
        std::thread::sleep(Duration::from_millis(10));
    };
    stdout.seek(SeekFrom::Start(0))?;
    stderr.seek(SeekFrom::Start(0))?;
    let mut out = Vec::new();
    let mut errors = Vec::new();
    stdout.take(MAX_OUT + 1).read_to_end(&mut out)?;
    stderr.take(MAX_ERR + 1).read_to_end(&mut errors)?;
    ensure!(
        status.success(),
        "owned process observation failed: {}",
        String::from_utf8_lossy(&errors)
    );
    ensure!(
        out.len() as u64 <= MAX_OUT && errors.len() as u64 <= MAX_ERR,
        "owned process observation exceeded bound"
    );
    String::from_utf8(out).context("owned process observation was not UTF-8")
}
