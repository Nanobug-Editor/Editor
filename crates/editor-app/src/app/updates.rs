//! Profile-wide host updates share one background state across empty/project windows and settings.

mod download;
mod install;
mod portable;
pub(crate) mod release;
#[cfg(test)]
pub(crate) mod tests;

use crate::*;
use download::Prepared;
use gpui_kit::{AnyElement, Global};
use release::{Edition, Identity, Release};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// A single map keeps update state independent of any document, workspace trust or settings window.
#[derive(Default)]
struct Services(HashMap<PathBuf, Entity<Updater>>);
impl Global for Services {}

/// Only the explicit restart button can arm an installation for the successful quit commit point.
struct PendingInstall {
    updater: Entity<Updater>,
}
impl Global for PendingInstall {}

/// User-only discovery preferences never authorize downloading or installing a release automatically.
#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct Preferences {
    pub(crate) automatic: bool,
    pub(crate) preview: bool,
}

impl Preferences {
    /// Read the selected profile; a prerelease build defaults to its own preview channel.
    fn load(profile: &Path, prerelease: bool) -> anyhow::Result<Self> {
        let path = profile.join("updates/preferences.json");
        if !path.exists() {
            return Ok(Self {
                automatic: true,
                preview: prerelease,
            });
        }
        Ok(serde_json::from_slice(&download::read_small_file(
            &path, 4096,
        )?)?)
    }
}

/// Operations are serialized so stale discovery cannot replace a selected or downloaded candidate.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    Loading,
    Idle,
    Checking,
    Downloading,
    Ready,
    Installing,
}

/// One application-owned model performs IO off-thread; views only project this retained state.
pub(crate) struct Updater {
    /// Explicit GUI profile owns preferences, staging and installation outcomes.
    pub(crate) profile: PathBuf,
    pub(crate) identity: Identity,
    pub(crate) preferences: Preferences,
    pub(crate) phase: Phase,
    /// A failed refresh preserves the last usable release details and notification.
    pub(crate) available: Option<Release>,
    pub(crate) error: Option<String>,
    pub(crate) checked: bool,
    prepared: Option<Prepared>,
    /// Cancellation is cooperative and the transfer also enforces finite network timeouts.
    cancel: Arc<AtomicBool>,
    /// Streaming progress is bounded by the immutable asset size; it carries no document state.
    pub(crate) received: Arc<AtomicU64>,
}

/// Initialize exactly once for `profile`; callers can use the same entity from any native window.
pub(crate) fn ensure(profile: &Path, cx: &mut App) -> Entity<Updater> {
    if !cx.has_global::<Services>() {
        cx.set_global(Services::default());
    }
    if let Some(service) = cx.global::<Services>().0.get(profile) {
        return service.clone();
    }
    let profile = profile.to_owned();
    let service = cx.new(|cx| Updater::new(profile.clone(), cx));
    cx.global_mut::<Services>()
        .0
        .insert(profile, service.clone());
    service
}

/// Read an existing profile service without starting network work during a render or menu build.
fn lookup(profile: &Path, cx: &App) -> Option<Entity<Updater>> {
    cx.try_global::<Services>()?.0.get(profile).cloned()
}

/// One explicit host update item is shared by the empty and project application menus.
pub(crate) fn menu_item(profile: &Path, cx: &App) -> plugin_runtime::plugin_protocol::ui::MenuItem {
    let label = lookup(profile, cx)
        .map(|service| {
            let updater = service.read(cx);
            if updater.prepared.is_some() {
                t!("updates.menu_ready").to_string()
            } else if updater.available.is_some() {
                t!("updates.menu_available").to_string()
            } else {
                t!("updates.menu_check").to_string()
            }
        })
        .unwrap_or_else(|| t!("updates.menu_check").to_string());
    plugin_runtime::plugin_protocol::ui::MenuItem {
        id: "item-host-update".into(),
        label,
        disabled: false,
        separator_before: true,
    }
}

/// Notify at the application trigger only while this host has a newer release or a ready package.
pub(crate) fn has_update(profile: &Path, cx: &App) -> bool {
    lookup(profile, cx).is_some_and(|service| service.read(cx).available.is_some())
}

/// The application-menu badge remains distinct from the independent plugin-manager indicator.
pub(crate) fn indicator(profile: &Path, cx: &App) -> Option<AnyElement> {
    has_update(profile, cx).then(|| {
        div()
            .id("host-update-dot")
            .debug_selector(|| "host-update-dot".into())
            .absolute()
            .right(px(1.))
            .top(px(1.))
            .size(px(7.))
            .rounded_full()
            .bg(cx.theme().primary)
            .into_any_element()
    })
}

/// The native helper mode is routed before GUI startup and deliberately never acquires a GUI mutex.
pub(crate) fn run_cli() -> anyhow::Result<bool> {
    install::run_cli()
}

impl Updater {
    /// Load only application-owned files off-thread before starting periodic discovery.
    fn new(profile: PathBuf, cx: &mut Context<Self>) -> Self {
        let executable = std::env::current_exe().unwrap_or_default();
        let service = Self {
            profile: profile.clone(),
            identity: Identity {
                version: semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap(),
                edition: Edition::detect(&executable),
                executable: executable.clone(),
            },
            preferences: Preferences {
                automatic: true,
                preview: false,
            },
            phase: Phase::Loading,
            available: None,
            prepared: None,
            error: None,
            checked: false,
            cancel: Arc::new(AtomicBool::new(false)),
            received: Arc::new(AtomicU64::new(0)),
        };
        let load = cx.background_executor().spawn(async move {
            let identity = Identity::load(executable);
            let preferences = Preferences::load(&profile, !identity.version.pre.is_empty());
            let prepared = Prepared::load(&profile, &identity);
            let outcome =
                download::read_small_file(&profile.join("updates/result.json"), 128 * 1024)
                    .ok()
                    .and_then(|data| serde_json::from_slice::<install::Outcome>(&data).ok());
            (identity, preferences, prepared, outcome)
        });
        cx.spawn(async move |this, cx| {
            let (identity, preferences, prepared, outcome) = load.await;
            let _ = this.update(cx, |this, cx| {
                this.identity = identity;
                match preferences {
                    Ok(value) => this.preferences = value,
                    Err(error) => this.error = Some(error.to_string()),
                }
                match prepared {
                    Ok(Some(value)) => {
                        this.available = Some(value.release.clone());
                        this.prepared = Some(value);
                    }
                    Ok(None) => {}
                    Err(error) => this.error = Some(error.to_string()),
                }
                if let Some(error) = outcome.and_then(|outcome| outcome.error) {
                    this.error = Some(error);
                }
                this.phase = if this.prepared.is_some() {
                    Phase::Ready
                } else {
                    Phase::Idle
                };
                cx.notify();
                cx.refresh_windows();
            });
        })
        .detach();
        // Production checks begin after startup settles. Unit/native fixture apps never contact GitHub implicitly.
        #[cfg(not(test))]
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(5)).await;
            loop {
                if this
                    .update(cx, |this, cx| {
                        if this.preferences.automatic && this.prepared.is_none() {
                            this.check(cx);
                        }
                    })
                    .is_err()
                {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_secs(6 * 60 * 60))
                    .await;
            }
        })
        .detach();
        service
    }

    /// Active work blocks changing channel or starting another operation against the same candidate.
    pub(crate) fn busy(&self) -> bool {
        matches!(
            self.phase,
            Phase::Loading | Phase::Checking | Phase::Downloading | Phase::Installing
        )
    }

    /// Check GitHub in a bounded worker and preserve cached release information on failure.
    pub(crate) fn check(&mut self, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.phase = Phase::Checking;
        self.error = None;
        let identity = self.identity.clone();
        let preview = self.preferences.preview;
        let task = cx
            .background_executor()
            .spawn(async move { download::check(&identity, preview) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.checked = true;
                match result {
                    Ok(release) => {
                        if this.prepared.as_ref().is_some_and(|cached| {
                            release
                                .as_ref()
                                .is_none_or(|release| release.version != cached.release.version)
                        }) {
                            this.prepared = None;
                            let _ = std::fs::remove_file(this.profile.join("updates/ready.json"));
                        }
                        this.available = release;
                    }
                    Err(error) => this.error = Some(format!("{error:#}")),
                }
                this.phase = if this.prepared.is_some() {
                    Phase::Ready
                } else {
                    Phase::Idle
                };
                cx.notify();
                cx.refresh_windows();
            });
        })
        .detach();
        cx.notify();
        cx.refresh_windows();
    }

    /// Persist an explicitly chosen discovery policy atomically and recheck the selected channel.
    pub(crate) fn configure(&mut self, automatic: bool, preview: bool, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let preferences = Preferences { automatic, preview };
        match download::save_json(&self.profile.join("updates/preferences.json"), &preferences) {
            Ok(()) => {
                let changed = self.preferences.preview != preview;
                self.preferences = preferences;
                self.error = None;
                if changed {
                    // A channel change revokes the previous choice even if the next request fails.
                    self.available = None;
                    self.prepared = None;
                    let _ = std::fs::remove_file(self.profile.join("updates/ready.json"));
                    self.phase = Phase::Idle;
                    self.check(cx);
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        cx.notify();
        cx.refresh_windows();
    }

    /// Stage the selected release; cancellation or any failure leaves the running application untouched.
    pub(crate) fn download(&mut self, cx: &mut Context<Self>) {
        if self.busy() || self.prepared.is_some() {
            return;
        }
        let Some(release) = self.available.clone() else {
            return;
        };
        if self.identity.edition == Edition::Browse {
            cx.open_url(&release.url);
            return;
        }
        self.phase = Phase::Downloading;
        self.error = None;
        self.cancel = Arc::new(AtomicBool::new(false));
        self.received.store(0, Ordering::Relaxed);
        let profile = self.profile.clone();
        let identity = self.identity.clone();
        let cancel = self.cancel.clone();
        let progress = self.received.clone();
        let task = cx.background_executor().spawn(async move {
            download::prepare(&profile, &identity, release, cancel, progress)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(prepared) => {
                        this.prepared = Some(prepared);
                        this.phase = Phase::Ready;
                    }
                    Err(error) => {
                        this.phase = Phase::Idle;
                        if !this.cancel.load(Ordering::Relaxed) {
                            this.error = Some(format!("{error:#}"));
                        }
                    }
                }
                cx.notify();
                cx.refresh_windows();
            });
        })
        .detach();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                if !this
                    .update(cx, |this, cx| {
                        cx.notify();
                        cx.refresh_windows();
                        this.phase == Phase::Downloading
                    })
                    .unwrap_or(false)
                {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    /// A cancellation request is observed before reads, after reads and before publishing ready state.
    pub(crate) fn cancel_download(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Arm installation, then let the existing all-window consent and cleanup coordinator decide exit.
    pub(crate) fn restart(&mut self, cx: &mut Context<Self>) {
        if self.phase != Phase::Ready
            || self.prepared.is_none()
            || cx.has_global::<PendingInstall>()
        {
            return;
        }
        self.phase = Phase::Installing;
        self.error = None;
        let updater = cx.entity();
        cx.set_global(PendingInstall { updater });
        cx.defer(app::projects::quit_application);
        cx.notify();
    }
}

/// Abandon the armed install when any close prompt is cancelled or any shutdown stage fails.
pub(crate) fn cancel_install(cx: &mut App) {
    if cx.has_global::<PendingInstall>() {
        let pending = cx.remove_global::<PendingInstall>();
        pending.updater.update(cx, |updater, cx| {
            updater.phase = if updater.prepared.is_some() {
                Phase::Ready
            } else {
                Phase::Idle
            };
            cx.notify();
        });
        cx.refresh_windows();
    }
}

/// Called only after every project and shared runtime acknowledged shutdown, before removing roots.
pub(crate) fn finish_exit(cx: &mut App) -> Result<(), String> {
    if !cx.has_global::<PendingInstall>() {
        return Ok(());
    }
    let pending = cx.remove_global::<PendingInstall>();
    pending.updater.update(cx, |updater, cx| {
        let result = updater
            .prepared
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No prepared host update"))
            .and_then(|prepared| {
                install::handoff(&updater.profile, &updater.identity.executable, prepared)
            });
        if let Err(error) = &result {
            updater.error = Some(format!("{error:#}"));
            updater.phase = Phase::Ready;
            cx.notify();
        }
        result.map_err(|error| error.to_string())
    })
}
