//! All project owners in one process share a profile writer, policy and temporary roots.

use crate::{Error, History, Policy, Result};
use rusqlite::Connection;
use std::{
    collections::HashMap,
    fs::File,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock, RwLock, Weak},
};

/// Weak ownership does not keep closed profiles, connections or protection leases alive.
struct Profile {
    /// Shared durable connection used by every owner of this profile.
    connection: Weak<Mutex<Connection>>,
    /// Shared active settings; projects do not retain independent obsolete policies.
    policy: Weak<RwLock<Policy>>,
    /// Shared serialization for compound operations.
    activity: Weak<Mutex<()>>,
    /// Shared in-flight roots, including native restore preimages.
    pins: Weak<Mutex<HashMap<i64, usize>>>,
    /// OS-backed exclusive writer ownership across processes.
    writer_lock: Weak<File>,
    /// Native access cannot change while operations or leases share the profile.
    legacy_io: Weak<Option<Arc<dyn crate::LegacyIo>>>,
}

/// Registry is keyed by canonical private root, not by workspace or currently focused tab.
static PROFILES: OnceLock<Mutex<HashMap<PathBuf, Profile>>> = OnceLock::new();

/// Reuse root's complete live profile or create it under the registry lock to prevent double opening.
pub(crate) fn open(
    root: PathBuf,
    policy: Policy,
    adapter_type: Option<std::any::TypeId>,
    create: impl FnOnce(PathBuf, Policy) -> Result<History>,
) -> Result<History> {
    let mut profiles = PROFILES
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| Error::Unavailable)?;
    if let Some(history) = profiles
        .get(&root)
        .and_then(|entry| entry.upgrade(root.clone()))
    {
        if adapter_type.is_some()
            && adapter_type
                != history
                    .legacy_io
                    .as_ref()
                    .as_ref()
                    .map(|adapter| adapter.as_ref().type_id())
        {
            return Err(Error::Unavailable);
        }
        history.set_policy(policy)?;
        return Ok(history);
    }
    profiles.retain(|_, entry| entry.connection.strong_count() != 0);
    let history = create(root.clone(), policy)?;
    profiles.insert(
        root,
        Profile {
            connection: Arc::downgrade(&history.connection),
            policy: Arc::downgrade(&history.policy),
            activity: Arc::downgrade(&history.activity),
            pins: Arc::downgrade(&history.pins),
            writer_lock: Arc::downgrade(&history.writer_lock),
            legacy_io: Arc::downgrade(&history.legacy_io),
        },
    );
    Ok(history)
}

impl Profile {
    /// Upgrade all shared lifetimes together; a closed profile can be initialized again.
    fn upgrade(&self, root: PathBuf) -> Option<History> {
        Some(History {
            connection: self.connection.upgrade()?,
            root: Arc::new(root),
            policy: self.policy.upgrade()?,
            activity: self.activity.upgrade()?,
            pins: self.pins.upgrade()?,
            writer_lock: self.writer_lock.upgrade()?,
            legacy_io: self.legacy_io.upgrade()?,
        })
    }
}
