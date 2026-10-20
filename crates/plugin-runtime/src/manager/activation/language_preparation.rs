//! Explicitly selected language roles reuse existing native policies on a detached preparation worker.
use super::*;
use crate::{DirectProvider, LanguageService, StructureProvider};
use std::sync::{Mutex, atomic::AtomicU64};

/// All pure roles consume the component bytes already admitted by this job's immutable package read.
/// Sharing these borrowed inputs avoids rereading a different disk image after permission validation.
#[derive(Clone, Copy)]
pub(super) struct ComponentCode<'a> {
    pub(super) engine: &'a wasmtime::Engine,
    pub(super) bytes: &'a [u8],
    pub(super) assets: &'a Path,
    pub(super) compiled: &'a crate::instance::CompiledCode,
}

/// Validate a source intent against canonical workspace containment without granting file access.
/// New unsaved paths require a real contained parent; links outside the workspace are rejected.
pub(super) fn validate_document(
    workspace: &str,
    document: &plugin_protocol::language::DocumentContext,
) -> anyhow::Result<()> {
    anyhow::ensure!(document.valid(), "Invalid language document path");
    let workspace = Path::new(workspace).canonicalize()?;
    let path = workspace.join(&document.path);
    let physical = if path.exists() {
        path.canonicalize()?
    } else {
        path.parent()
            .ok_or_else(|| anyhow::anyhow!("Document has no parent"))?
            .canonicalize()?
            .join(path.file_name().unwrap())
    };
    anyhow::ensure!(
        physical.starts_with(&workspace),
        "Language document escaped workspace"
    );
    Ok(())
}

impl DemandPreparation {
    /// The guest may select an approved plan; paths and executable overrides remain host-validated.
    pub(super) fn prepare_language_service(
        &self,
        assets: &Path,
        code: Option<ComponentCode<'_>>,
        mut owner: Option<&mut Instance>,
        authority: Option<crate::EditorAuthority>,
        entry: &Installed,
        mut provider: plugin_protocol::language::Provider,
        settings: plugin_protocol::settings::Effective,
        document: Option<&plugin_protocol::language::DocumentContext>,
    ) -> anyhow::Result<(
        LanguageService,
        Option<plugin_protocol::language::ProjectContext>,
    )> {
        let workspace = Path::new(&self.environment.workspace).canonicalize()?;
        if let Some(document) = document {
            validate_document(&self.environment.workspace, document)?;
        }
        // A readonly hook receives only an already registered, exact live owner's selection.
        // Its isolated worker identity must never replace the owning business instance.
        let selected = document.and_then(|document| {
            let instance = self
                .existing_owner
                .as_deref()
                .or_else(|| owner.as_ref().and_then(|owner| owner.instance_id()))?;
            self.host_resources.contexts.selected(
                instance,
                self.token.scope(),
                &provider.id,
                &document.path,
            )
        });
        let candidates = std::iter::once(&provider.service)
            .chain(&provider.alternatives)
            .map(|id| {
                anyhow::ensure!(
                    entry.grants.contains(&format!("process.service.{id}")),
                    "LSP service permission denied"
                );
                Ok((
                    id.clone(),
                    entry
                        .manifest
                        .services
                        .get(id)
                        .ok_or_else(|| anyhow::anyhow!("Missing LSP service"))?
                        .clone(),
                ))
            })
            .collect::<anyhow::Result<BTreeMap<_, _>>>()?;
        let proposal = if provider.hook {
            owner
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("LSP hook instance is unavailable"))?
                .prepare_language(plugin_protocol::language::Context {
                    provider: provider.id.clone(),
                    workspace: self.environment.workspace.clone(),
                    document: document.cloned(),
                    selected_context: selected
                        .as_ref()
                        .map(|context| context.reference.descriptor.clone()),
                    // Demand uses an already approved dependency plan; discovery retains its separate flag.
                    preparing_dependencies: false,
                    package_root: String::new(),
                    data_root: String::new(),
                    settings: settings.clone(),
                    candidates: candidates.clone(),
                })?
        } else {
            Default::default()
        };
        let mut service = candidates
            .get(proposal.service.as_ref().unwrap_or(&provider.service))
            .ok_or_else(|| anyhow::anyhow!("LSP hook selected an undeclared service"))?
            .clone();
        if let Some(plan) = proposal.installation {
            anyhow::ensure!(
                entry.grants.contains("dependencies.prepare"),
                "Dependency preparation permission required"
            );
            service.installation = Some(plan);
        }
        if proposal.program.is_some() || proposal.args.is_some() {
            anyhow::ensure!(
                entry.manifest.permissions.contains("process.exec")
                    && entry.grants.contains("process.exec"),
                "Dynamic LSP startup requires process.exec permission"
            );
            if let Some(program) = proposal.program {
                service.program = program;
                service.installation = None;
                service.search_paths.clear();
            }
            if let Some(args) = proposal.args {
                service.args = args;
            }
            anyhow::ensure!(service.valid(), "Invalid dynamic LSP startup");
        }
        if let Some(key) = &provider.executable_setting {
            if let Some(value) = settings.get(key).filter(|value| {
                matches!(
                    value.source,
                    plugin_protocol::settings::Source::User
                        | plugin_protocol::settings::Source::Project
                )
            }) {
                service.program = value
                    .value
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Invalid explicit executable"))?
                    .into();
                service.installation = None;
                // An explicit user selection must fail visibly instead of silently falling back.
                service.search_paths.clear();
                anyhow::ensure!(
                    Path::new(&service.program).is_absolute(),
                    "Explicit executable must be an absolute path"
                );
            }
        }
        let context = if document.is_some() {
            let context = proposal
                .context
                .unwrap_or(plugin_protocol::language::ProjectContext {
                    key: "workspace".into(),
                    generation: 0,
                });
            anyhow::ensure!(
                context.valid(),
                "LSP hook returned an invalid project identity"
            );
            Some(context)
        } else {
            anyhow::ensure!(
                proposal.context.is_none(),
                "Project identity requires document-scoped discovery"
            );
            None
        };
        let root = if let Some(relative) = proposal.project_root {
            crate::package::validate_relative(&relative)?;
            workspace.join(relative).canonicalize()?
        } else {
            workspace.clone()
        };
        anyhow::ensure!(
            root.is_dir() && root.starts_with(&workspace),
            "LSP project root escaped workspace"
        );
        if let Some(selected) = &selected {
            anyhow::ensure!(
                selected.is_live()
                    && selected.root == root
                    && context
                        .as_ref()
                        .is_some_and(|context| context.key == selected.reference.descriptor.key
                            && context.generation == selected.reference.descriptor.generation),
                "LSP hook did not preserve the selected registered context"
            );
        }
        if let Some(options) = proposal.initialization_options {
            provider.initialization_options = options;
        }
        if let Some(configuration) = proposal.configuration {
            provider.configuration = configuration;
        }
        anyhow::ensure!(
            provider.valid(),
            "LSP hook returned invalid or oversized configuration"
        );
        let prepared = service
            .installation
            .as_ref()
            .map(|plan| {
                if service.on_demand {
                    self.prepare_language_dependency(assets, plan, &provider.id, document)
                } else {
                    crate::dependencies::cached(&self.root, plan)
                }
            })
            .transpose()?;
        let program = if let Some(prepared) = &prepared {
            prepared.program.clone()
        } else {
            crate::toolchains::resolve_service(&service)?
        };
        let args = if let Some(prepared) = &prepared {
            prepared.args(&service.args)?
        } else {
            service.args
        };
        let mut language_service = LanguageService::new(
            entry.manifest.id.clone(),
            provider,
            root,
            program,
            args,
            self.host_resources.logs.clone(),
            crate::language_service::LanguagePolicy {
                // Original negotiation and installation consent remain separate from server flags.
                capabilities: crate::capabilities::negotiate(&entry.manifest)?.capabilities,
                permissions: entry.grants.clone(),
                workspace,
                data: self.data.clone(),
                application: entry.manifest.scope
                    == plugin_protocol::api::InstanceScope::Application,
                inherited: authority,
            },
        );
        if let Some(selected) = selected {
            language_service.set_registered_context(self.host_resources.contexts.clone(), selected);
        }
        if language_service.provider.completion_hook {
            // This independent instance sees no private snapshot and grants no host operation, even in Prepare.
            let instance = self.prepare_pure_worker(code.ok_or_else(|| {
                api::Failure::new(
                    api::ErrorCode::CapabilityUnavailable,
                    "Completion requires a component",
                )
            })?)?;
            language_service.completion = Some(crate::language_service::CompletionHook {
                instance: std::sync::Mutex::new(Some(instance)),
                settings,
                sequence: std::sync::atomic::AtomicU64::new(1),
            });
        }
        language_service.dependencies = prepared.map(|prepared| prepared.locks).unwrap_or_default();
        Ok((language_service, context))
    }

    /// Prepare only this version's selected immutable plan, observing its original source waits and
    /// scope lifetime at every I/O barrier. Cache hits work offline and are pinned through publication;
    /// installers still require their separate explicit installation workflow and cannot run here.
    fn prepare_language_dependency(
        &self,
        assets: &Path,
        plan: &plugin_protocol::dependencies::Plan,
        provider: &str,
        document: Option<&plugin_protocol::language::DocumentContext>,
    ) -> anyhow::Result<crate::dependencies::Prepared> {
        anyhow::ensure!(
            self.entry.grants.contains("dependencies.prepare"),
            "Dependency preparation permission required"
        );
        let cached = crate::dependencies::cached(&self.root, plan).ok();
        anyhow::ensure!(
            cached.is_some()
                || plan
                    .artifacts
                    .iter()
                    .all(|artifact| artifact.installer.is_none()),
            "Prepare native installer dependencies through plugin installation"
        );
        let provider = format!("{}/{}", self.entry.manifest.id, provider);
        let target = document.cloned();
        let progress_provider = provider.clone();
        let progress_target = target.clone();
        let progress_waiters = self.waiters.clone();
        let waiters = self.waiters.clone();
        let alive = self.alive.clone();
        let host_alive = self.host_alive.clone();
        let control = crate::InstallControl::new(move |stage| {
            for waiter in progress_waiters.lock().unwrap().iter().filter(|waiter| {
                waiting_for_language(waiter, &progress_provider, progress_target.as_ref())
            }) {
                waiter
                    .completion
                    .progress(format!("Preparing language dependency: {stage:?}"));
            }
        })
        .with_guard(move || {
            alive.load(Ordering::Acquire)
                && host_alive.load(Ordering::Acquire)
                && waiters
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|waiter| waiting_for_language(waiter, &provider, target.as_ref()))
        });
        let _lock = crate::dependencies::lock(&self.root, &control)?;
        let prepared = match cached {
            Some(prepared) => prepared,
            None => crate::dependencies::prepare(&self.root, assets, plan, &control)?,
        };
        // Separate receipts retain other contexts' backend versions after their transports exit.
        // Only the package's immutable declared/proposed plan is persisted, never source paths/text.
        control.check()?;
        let retained = BTreeMap::from([("language".to_string(), plan)]);
        let identity = format!("{:x}", Sha256::digest(serde_json::to_vec(&retained)?));
        let receipt = self
            .root
            .join("dependency-receipts")
            .join(&self.entry.manifest.id)
            .join(format!("{}-language-{identity}.json", self.entry.digest));
        atomic_write(&receipt, &serde_json::to_vec(&retained)?)?;
        Ok(prepared)
    }
    /// A selected direct callback inherits only the exact admitted owner and compiles on this worker.
    pub(super) fn prepare_direct(
        &self,
        code: ComponentCode<'_>,
        authority: crate::EditorAuthority,
        entry: &Installed,
        declaration: plugin_protocol::direct::Provider,
        settings: plugin_protocol::settings::Effective,
    ) -> anyhow::Result<DirectProvider> {
        anyhow::ensure!(
            entry.grants.contains("editor.read"),
            "Direct language requires editor.read"
        );
        let instance = self.prepare_pure_worker(code)?;
        Ok(DirectProvider {
            owner: entry.manifest.id.clone(),
            declaration,
            instance: Mutex::new(Some(instance)),
            settings,
            authority,
            active: AtomicBool::new(true),
            sequence: AtomicU64::new(1),
            workspace_read: entry.grants.contains("workspace.read"),
        })
    }
    /// All authority checks precede pure initialization; no private snapshot, IO or process is restored.
    pub(super) fn prepare_structure(
        &self,
        code: ComponentCode<'_>,
        entry: &Installed,
        declaration: plugin_protocol::structure::Provider,
        settings: plugin_protocol::settings::Effective,
    ) -> anyhow::Result<StructureProvider> {
        anyhow::ensure!(
            entry.grants.contains("editor.read"),
            "Structure requires editor.read permission"
        );
        let instance = self.prepare_pure_worker(code)?;
        Ok(StructureProvider {
            owner: entry.manifest.id.clone(),
            declaration,
            instance: Mutex::new(Some(instance)),
            settings,
            entry: entry.clone(),
            root: self.root.clone(),
            active: AtomicBool::new(true),
            sequence: AtomicU64::new(1),
        })
    }

    /// This shared isolated initialization grants no host operation, IO/process restoration or private Snapshot.
    fn prepare_pure_worker(&self, code: ComponentCode<'_>) -> anyhow::Result<Instance> {
        Instance::prepare_language_worker(
            code.engine,
            code.bytes,
            &self.entry.manifest,
            &self.entry.grants,
            self.environment.clone(),
            self.data.clone(),
            code.assets.to_owned(),
            self.host_resources.clone(),
            code.compiled,
        )
    }
}

/// A different document or a cancelled source cannot keep this backend's download alive by itself.
fn waiting_for_language(
    waiter: &Waiter,
    provider: &str,
    document: Option<&plugin_protocol::language::DocumentContext>,
) -> bool {
    !waiter.completion.status().is_terminal()
        && match (&waiter.reason, document) {
            (
                DemandReason::Language {
                    provider: candidate,
                    role: LanguageRole::Lsp | LanguageRole::Formatter,
                },
                None,
            ) => candidate == provider,
            (
                DemandReason::LanguageDocument {
                    provider: candidate,
                    document: target,
                    ..
                },
                Some(document),
            ) => candidate == provider && target == document,
            _ => false,
        }
}
