//! Profile-owned workers and native locations are shared across project windows.

use super::{model, model::Panel, now_ms};
use crate::*;
use editor_history::{History, Policy, Worker};

/// Application ownership keeps accepted history writes alive when an individual project closes.
#[derive(Default)]
struct ProfileWorkers {
    /// Profile workers are shared only within this native application instance.
    workers: HashMap<PathBuf, Worker>,
    /// Legacy fixtures without an explicit profile must not write artificial versions into user data.
    #[cfg(test)]
    test_profile: Option<tempfile::TempDir>,
}
impl gpui_kit::Global for ProfileWorkers {}

/// Local view state and background access for the selected user profile.
#[derive(Default)]
pub(crate) struct State {
    /// Missing worker means thread/profile resolution failed and saves must still remain usable.
    pub(crate) worker: Option<Worker>,
    /// Policy location belongs exclusively to the selected user profile.
    pub(crate) paths: Option<platform_windows::history::HistoryPaths>,
    /// Repeated observations of one failure do not flood host messages.
    pub(crate) last_error: Option<String>,
    /// One current skip explanation is visible without flooding messages for repeated watcher events.
    pub(super) skipped: Option<(PathBuf, editor_history::SkipReason)>,
    /// At most one bounded metadata page and one readonly historical preview are mounted.
    pub(crate) panel: Option<Panel>,
    /// Every navigation/cancel supersedes earlier asynchronous results.
    pub(super) generation: u64,
    /// A shutdown revokes previously prepared saves even if a failed close later permits new saves.
    pub(super) save_generation: u64,
    /// Cleanup seals user saves until its owner reports failure and keeps the document available.
    pub(super) saves_sealed: bool,
    /// Deleted recovery buffers carry create-only authority and source identity until saved.
    pub(super) pending_create: HashMap<u64, model::PendingCreate>,
    /// Open native document incarnations retain logical identity through an observed disk deletion.
    pub(super) identities: HashMap<u64, editor_history::FileId>,
    /// Native events from the app's own atomic replacement must not split a logical file's history.
    pub(super) local_saves: HashMap<PathBuf, platform_windows::history::FileStamp>,
    /// A pending native destination dialog is cancelled with its originating view/window.
    pub(super) picker: Option<platform_windows::file_picker::FilePickerControl>,
}

impl State {
    /// Resolve native locations and start storage initialization away from the GPUI thread.
    pub(crate) fn new(profile: Option<&Path>, workspace: Option<&Path>, cx: &mut App) -> Self {
        if !cx.has_global::<ProfileWorkers>() {
            cx.set_global(ProfileWorkers::default());
        }
        #[cfg(test)]
        let test_profile = {
            let normal = dirs::config_dir().map(|root| root.join("MeEditor"));
            if profile.is_none() || profile == normal.as_deref() {
                let profiles = cx.global_mut::<ProfileWorkers>();
                let directory = profiles.test_profile.get_or_insert_with(|| {
                    tempfile::tempdir().expect("isolated history test profile")
                });
                Some(directory.path().to_owned())
            } else {
                None
            }
        };
        #[cfg(test)]
        let profile = test_profile.as_deref().or(profile);
        let paths = match platform_windows::history::HistoryPaths::for_profile(profile) {
            Ok(paths) => paths,
            Err(error) => {
                return Self {
                    last_error: Some(error.to_string()),
                    ..Default::default()
                };
            }
        };
        if let Some(workspace) = workspace {
            if let Err(error) = paths.validate_workspace(workspace) {
                return Self {
                    last_error: Some(error.to_string()),
                    ..Default::default()
                };
            }
        }
        if let Some(worker) = cx.global::<ProfileWorkers>().workers.get(&paths.root) {
            let status = worker.status();
            if !status.initialized || status.available {
                return Self {
                    worker: Some(worker.clone()),
                    paths: Some(paths),
                    ..Default::default()
                };
            }
        }
        let initialize = paths.clone();
        let executor = cx.background_executor().clone();
        let timers = executor.clone();
        let worker = Worker::on_executor(
            move || {
                let policy = load_policy(&initialize.settings)?;
                let history = History::open_with_legacy_io(
                    initialize.root,
                    policy,
                    Some(std::sync::Arc::new(
                        platform_windows::history::NativeLegacyIo,
                    )),
                )?;
                history.maintain(now_ms())?;
                Ok(history)
            },
            move |task| executor.spawn(task).detach(),
            move |duration| {
                use futures::FutureExt as _;
                timers.timer(duration).boxed()
            },
        );
        cx.global_mut::<ProfileWorkers>()
            .workers
            .insert(paths.root.clone(), worker.clone());
        Self {
            worker: Some(worker),
            paths: Some(paths),
            ..Default::default()
        }
    }
}

/// Missing settings use defaults; malformed settings are preserved and reported rather than reset.
pub(crate) fn load_policy(path: &Path) -> editor_history::Result<Policy> {
    match std::fs::read(path) {
        Ok(bytes) => {
            let policy: Policy = serde_json::from_slice(&bytes)
                .map_err(|error| editor_history::Error::InvalidPolicy(error.to_string()))?;
            policy.validate()?;
            Ok(policy)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Policy::default()),
        Err(error) => Err(error.into()),
    }
}
