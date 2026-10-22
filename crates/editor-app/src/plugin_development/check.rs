//! A synchronous static verifier runs before any GPUI, profile, installer or plugin initialization.
use super::*;
use plugin_runtime::validation::{self, Code};
use sha2::{Digest, Sha256};

/// Process only `--plugin-check`; stable exit codes belong exclusively to this new tool mode.
pub(super) fn run() -> anyhow::Result<bool> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--plugin-check")) {
        return Ok(false);
    }
    let path = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("--plugin-check requires a ZIP"));
    let parsed = (|| -> anyhow::Result<(PathBuf, bool)> {
        let path = PathBuf::from(path?);
        let mut json = false;
        if let Some(option) = args.next() {
            anyhow::ensure!(option == "--format", "Expected --format json|text");
            let format = args
                .next()
                .ok_or_else(|| anyhow::anyhow!("Missing report format"))?;
            anyhow::ensure!(
                format == "json" || format == "text",
                "Unknown report format"
            );
            json = format == "json";
        }
        anyhow::ensure!(args.next().is_none(), "Unexpected plugin-check arguments");
        Ok((path, json))
    })();
    let (path, json) = match parsed {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let tool = super::report::tool();
    let report = validation::check(&path, tool);
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        println!(
            "Static admission: {} (guest execution: not exercised)",
            if report.passed { "passed" } else { "failed" }
        );
        for issue in &report.diagnostics {
            println!(
                "{}: {:?} {}",
                issue.severity,
                issue.code,
                issue.pointer.as_deref().unwrap_or("")
            );
        }
    }
    let code = if report.passed {
        0
    } else if report
        .diagnostics
        .iter()
        .any(|issue| issue.code == Code::InputIo)
    {
        2
    } else {
        1
    };
    // This entry is synchronous and has created no child, profile or runtime requiring Drop cleanup.
    std::process::exit(code);
}

/// Hash a binary by streaming fixed chunks; executable identity does not require a second full copy.
pub(super) fn hash_file(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut source = std::fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut bytes = [0u8; 64 * 1024];
    loop {
        let count = source.read(&mut bytes)?;
        if count == 0 {
            break;
        }
        hash.update(&bytes[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
