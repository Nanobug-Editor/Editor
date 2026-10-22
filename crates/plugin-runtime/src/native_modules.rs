//! Immutable native adapters are leased to a consumer and stopped before an exporter is retired.
//! These modules run outside WASM; the permission is explicit and conveys no guest filesystem root.
use crate::{EditorAuthority, LanguageService};
use plugin_protocol::{
    api::{ErrorCode, Failure},
    language_control::ModuleExport,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
};
mod validation;
pub(crate) use validation::validate_exports;

/// Trusted paths are delivered only to the managed native worker, never serialized to a guest.
#[derive(Clone, Debug)]
pub struct NativeModuleDescriptor {
    /// Export key within the admitted package; the engine treats it as opaque metadata.
    pub export_id: String,
    /// Canonical entry below the immutable package root, verified before delivery.
    pub entry: PathBuf,
    /// Canonical package directory; no private or project directory can substitute for it.
    pub package_root: PathBuf,
    /// Exact sealed module directory, independent of the entry's nesting depth.
    pub module_root: PathBuf,
    /// Root-relative complete file closure; native loaders reject all additional neighbors.
    pub files: BTreeMap<String, String>,
    /// Digest of the exact installed archive or validated development package.
    pub package_digest: String,
    /// SHA-256 of the entry itself, in addition to the pinned complete file closure.
    pub sha256: String,
    /// Public adapter interface implemented by these bytes.
    pub abi: String,
    /// Plugin-defined engine ranges, checked in the managed worker before code loading.
    pub engine: BTreeMap<String, semver::VersionReq>,
}

/// A bound worker pins the whole immutable closure until it has stopped, even after revocation.
pub struct NativeModuleLease {
    descriptor: NativeModuleDescriptor,
    source: String,
    consumer: String,
    alive: AtomicBool,
    authority: EditorAuthority,
    workspace: PathBuf,
    document: plugin_protocol::language::DocumentContext,
    source_alive: Arc<AtomicBool>,
    services: Mutex<Vec<Weak<LanguageService>>>,
    files: Mutex<Vec<std::fs::File>>,
    /// Ancestor pins prevent a verified entry path from being retargeted before native loading.
    path: Mutex<Option<crate::native_path::CheckedPath>>,
}
impl NativeModuleLease {
    /// Read the host-verified entry, closure identity, adapter ABI and engine constraints.
    pub fn descriptor(&self) -> &NativeModuleDescriptor {
        &self.descriptor
    }
    /// Check both original incarnations; completed calls do not revoke an attached worker.
    pub fn is_live(&self) -> bool {
        self.alive.load(Ordering::Acquire)
            && self.source_alive.load(Ordering::Acquire)
            && self.authority.is_live()
    }
    /// Bind before module loading. A revoke racing with attachment seals the service immediately.
    /// Returns InvalidHandle when either participant has already retired; no native load may follow.
    pub fn attach(self: &Arc<Self>, service: &Arc<LanguageService>) -> Result<(), Failure> {
        let mut services = self.services.lock().unwrap();
        if !self.is_live()
            || !service.admits_control(&self.authority, &self.workspace, &self.document)
        {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Native module lease does not authorize this service context",
            ));
        }
        if !services
            .iter()
            .any(|weak| weak.upgrade().is_some_and(|old| Arc::ptr_eq(&old, service)))
        {
            service.retain_module(self.clone())?;
            services.push(Arc::downgrade(service));
        }
        Ok(())
    }
    fn revoke(&self) {
        self.alive.store(false, Ordering::Release);
        let services = std::mem::take(&mut *self.services.lock().unwrap());
        for service in services.into_iter().filter_map(|weak| weak.upgrade()) {
            service.module_revoked(self);
        }
        // Every attached process has been reaped before package replacement/deletion is permitted.
        self.files.lock().unwrap().clear();
        self.path.lock().unwrap().take();
    }
    /// Consumers can independently retain identical bytes. A different exporter incarnation never
    /// substitutes for the revoked publication, even when package names and hashes happen to match.
    pub(crate) fn same_code(&self, other: &Self) -> bool {
        self.source == other.source
            && self.descriptor.export_id == other.descriptor.export_id
            && self.descriptor.package_digest == other.descriptor.package_digest
            && self.descriptor.entry == other.descriptor.entry
            && self.descriptor.abi == other.descriptor.abi
            && Arc::ptr_eq(&self.source_alive, &other.source_alive)
    }
}

/// Manager-only publication contains no mutable project or private directory.
#[derive(Clone)]
pub(crate) struct Publisher {
    pub instance: String,
    pub scope: String,
    pub root: PathBuf,
    pub digest: String,
    pub alive: Arc<AtomicBool>,
    pub exports: BTreeMap<String, ModuleExport>,
}

#[derive(Default)]
struct Registry {
    sources: BTreeMap<String, Publisher>,
    leases: Vec<Weak<NativeModuleLease>>,
}

/// Shared, host-owned registry; guests cannot add paths or revive retired publications.
#[derive(Clone, Default)]
pub struct NativeModules(Arc<Mutex<Registry>>);
impl std::fmt::Debug for NativeModules {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeModules").finish_non_exhaustive()
    }
}
impl NativeModules {
    pub(crate) fn publish(&self, publisher: Publisher) {
        self.0
            .lock()
            .unwrap()
            .sources
            .insert(publisher.instance.clone(), publisher);
    }
    /// Static publications observe availability revocation without requiring a lifecycle component.
    pub(crate) fn sweep_dead(&self) {
        let leases = {
            let mut registry = self.0.lock().unwrap();
            registry
                .sources
                .retain(|_, source| source.alive.load(Ordering::Acquire));
            registry.leases.retain(|lease| lease.strong_count() != 0);
            registry
                .leases
                .iter()
                .filter_map(Weak::upgrade)
                .filter(|lease| !lease.is_live())
                .collect::<Vec<_>>()
        };
        for lease in leases {
            lease.revoke();
        }
    }
    /// Disable, failed update, trust loss and stop use the same synchronous native cleanup boundary.
    pub(crate) fn revoke_owner(&self, owner: &str) {
        let leases = {
            let mut registry = self.0.lock().unwrap();
            registry.sources.remove(owner);
            registry.leases.retain(|lease| lease.strong_count() != 0);
            registry
                .leases
                .iter()
                .filter_map(Weak::upgrade)
                .filter(|lease| lease.source == owner || lease.consumer == owner)
                .collect::<Vec<_>>()
        };
        for lease in leases {
            lease.revoke();
        }
    }
    /// Admission requires the exact live exporter, original scope and sealed consumer authority.
    pub(crate) fn import(
        &self,
        source: &str,
        scope: &str,
        consumer: &str,
        export: &str,
        authority: EditorAuthority,
        workspace: &Path,
        document: &plugin_protocol::language::DocumentContext,
    ) -> Result<Arc<NativeModuleLease>, Failure> {
        let existing = self
            .0
            .lock()
            .unwrap()
            .leases
            .iter()
            .filter_map(Weak::upgrade)
            .find(|lease| {
                lease.source == source
                    && lease.consumer == consumer
                    && lease.descriptor.export_id == export
                    && lease.authority.same_owner(&authority)
                    && lease.workspace == workspace
                    && lease.document == *document
                    && lease.is_live()
            });
        let publisher = self
            .0
            .lock()
            .unwrap()
            .sources
            .get(source)
            .cloned()
            .filter(|publisher| publisher.scope == scope && publisher.alive.load(Ordering::Acquire))
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::CapabilityUnavailable,
                    "Module exporter is not live in this scope",
                )
            })?;
        let definition = publisher
            .exports
            .get(export)
            .ok_or_else(|| Failure::new(ErrorCode::NotFound, "Module export was not declared"))?;
        let (package_root, module_root, entry, files, path) = (|| -> anyhow::Result<_> {
            anyhow::ensure!(
                validation::hash_valid(&publisher.digest),
                "Unverified package identity"
            );
            // Windows canonicalization adds a verbatim prefix even for an ordinary safe path.
            // Validate physical ancestry instead of rejecting this harmless spelling difference.
            let package_root = crate::native_path::normalized_target(&publisher.root)?;
            let root = crate::instance::safe_path(&package_root, &definition.root, true)?;
            let path = crate::native_path::pin_path(&root, false)?;
            let entry = crate::instance::safe_path(&root, &definition.entry, true)?;
            let files = validation::verify_directory(&root, definition)?;
            Ok((package_root, root, entry, files, path))
        })()
        .map_err(|error| Failure::new(ErrorCode::InvalidPath, error.to_string()))?;
        // File pins deny replacement of known files, but cannot prohibit creation of a new neighbor.
        // Re-enumerate even for an idempotent import before returning its previous sealed authority.
        if let Some(existing) = existing {
            if existing.is_live() {
                return Ok(existing);
            }
        }
        let lease = Arc::new(NativeModuleLease {
            descriptor: NativeModuleDescriptor {
                export_id: export.into(),
                entry,
                package_root,
                module_root,
                files: definition.files.clone(),
                package_digest: publisher.digest,
                sha256: definition.sha256.clone(),
                abi: definition.abi.clone(),
                engine: definition.engine.clone(),
            },
            source: source.into(),
            consumer: consumer.into(),
            alive: AtomicBool::new(true),
            authority,
            workspace: workspace.to_owned(),
            document: document.clone(),
            source_alive: publisher.alive,
            services: Default::default(),
            files: Mutex::new(files),
            path: Mutex::new(Some(path)),
        });
        let mut registry = self.0.lock().unwrap();
        registry.leases.retain(|lease| lease.strong_count() != 0);
        if registry.leases.len() >= 256 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Native module lease quota exceeded",
            ));
        }
        if !registry.sources.contains_key(source) || !lease.is_live() {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Module exporter retired during verification",
            ));
        }
        registry.leases.push(Arc::downgrade(&lease));
        Ok(lease)
    }
}
