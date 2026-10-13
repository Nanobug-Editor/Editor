//! Ordinary GUI launches join the existing profile owner before allocating project resources.
mod failure;
mod transport;
pub(crate) use failure::show as show_error;
use gpui_kit::{App, Global};
use rust_i18n::t;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
pub(crate) use transport::Server;

/// One launch selects its GUI profile and absolute target before transport or project resources start.
pub(crate) struct Launch {
    pub(crate) profile: PathBuf,
    pub(crate) target: Option<PathBuf>,
    /// Only main, before starting any threads, applies an explicitly selected GUI profile to legacy readers.
    pub(crate) explicit_profile: bool,
}

/// Resolve ordinary GUI arguments while retaining the existing explicit development-instance target.
/// `--profile <directory>` selects an independent GUI owner; `--` preserves a positional path's boundary.
/// Tooling handles its arguments first and never enters this parser or the GUI transport.
pub(crate) fn resolve() -> anyhow::Result<Launch> {
    if let Some(instance) = crate::plugin_development::instance::context() {
        return Ok(Launch {
            profile: instance.profile.clone(),
            target: Some(instance.workspace.clone()),
            explicit_profile: false,
        });
    }
    let mut arguments = std::env::args_os().skip(1);
    let mut profile = None;
    let mut target = None;
    let mut positional = false;
    while let Some(argument) = arguments.next() {
        if !positional && argument == "--" {
            positional = true;
        } else if !positional && argument == "--profile" {
            anyhow::ensure!(profile.is_none(), t!("projects.launch_duplicate_profile"));
            let value = arguments
                .next()
                .ok_or_else(|| anyhow::anyhow!(t!("projects.launch_missing_profile")))?;
            anyhow::ensure!(!value.is_empty(), t!("projects.launch_empty_profile"));
            profile = Some(PathBuf::from(value));
        } else {
            anyhow::ensure!(
                positional || !argument.to_string_lossy().starts_with("--"),
                t!(
                    "projects.launch_unknown_option",
                    option = argument.to_string_lossy()
                )
            );
            anyhow::ensure!(target.is_none(), t!("projects.launch_multiple_targets"));
            target = Some(PathBuf::from(argument));
        }
    }
    let explicit_profile = profile.is_some();
    let profile = match profile {
        Some(profile) => profile,
        None => super::shortcuts::bootstrap::default_path()
            .map_err(anyhow::Error::msg)?
            .parent()
            .ok_or_else(|| anyhow::anyhow!(t!("projects.launch_missing_default_profile")))?
            .to_owned(),
    };
    let cwd = std::env::current_dir()?;
    let absolute = |path: PathBuf| {
        if path.is_absolute() {
            path
        } else {
            cwd.join(path)
        }
    };
    Ok(Launch {
        profile: absolute(profile),
        target: target.map(absolute),
        explicit_profile,
    })
}

/// Only the primary launch may initialize a GPUI application and its shared plugin data owner.
pub(crate) enum Admission {
    Primary(Server),
    Delivered,
}

/// Coordinate one already-resolved target for this isolated user profile.
/// No project, plugin store or installation mutex is allocated by a delivered secondary launch.
pub(crate) fn enter(profile: &Path, target: Option<PathBuf>) -> anyhow::Result<Admission> {
    transport::enter(profile, target)
}

impl Global for Server {}

/// Mount incoming requests onto the same real GPUI project lifecycle used by startup.
/// The application retains the OS lease until its final shutdown, including exit cancellation.
pub(crate) fn attach(server: Server, profile: PathBuf, cx: &mut App) {
    cx.set_global(server);
    cx.spawn(async move |cx| {
        let mut receipts: BTreeMap<String, (std::time::Instant, transport::Response)> =
            BTreeMap::new();
        loop {
            // Native IO publishes to a bounded synchronous mailbox; only GPUI's executor wakes its UI task.
            cx.background_executor()
                .timer(std::time::Duration::from_millis(40))
                .await;
            cx.update(|cx| {
                while let Some(request) = cx.global::<Server>().next_request() {
                    let now = std::time::Instant::now();
                    // Retain acknowledgments longer than the client's complete retry attempt, even across a burst.
                    receipts.retain(|_, (expiry, _)| *expiry > now);
                    let response = if let Some((_, previous)) = receipts.get(&request.id) {
                        previous.clone()
                    } else if receipts.len() >= 1024 {
                        // Refuse overload rather than forgetting a live request and repeating its file action.
                        transport::Response::Rejected(
                            t!("projects.external_queue_busy").to_string(),
                        )
                    } else {
                        let result = if request.is_current() {
                            super::projects::open_request(
                                profile.clone(),
                                request.target.clone(),
                                cx,
                            )
                        } else {
                            Err(anyhow::anyhow!(t!("projects.external_dispatch_expired")))
                        };
                        let response = match result {
                            Ok(()) => transport::Response::Accepted,
                            Err(error) => transport::Response::Rejected(error.to_string()),
                        };
                        // A lost acknowledgment may be retried with the same identity, without repeating a file action.
                        receipts.insert(
                            request.id.clone(),
                            (now + std::time::Duration::from_secs(20), response.clone()),
                        );
                        response
                    };
                    request.complete(response);
                }
            });
        }
    })
    .detach();
}
