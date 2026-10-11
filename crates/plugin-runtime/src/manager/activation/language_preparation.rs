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
}

impl DemandPreparation {
    /// The guest may select an approved plan; paths and executable overrides remain host-validated.
    pub(super) fn prepare_language_service(
        &self,
        code: Option<ComponentCode<'_>>,
        mut owner: Option<&mut Instance>,
        authority: Option<crate::EditorAuthority>,
        entry: &Installed,
        mut provider: plugin_protocol::language::Provider,
        settings: plugin_protocol::settings::Effective,
    ) -> anyhow::Result<LanguageService> {
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
        let workspace = Path::new(&self.environment.workspace).canonicalize()?;
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
            .map(|plan| crate::dependencies::cached(&self.root, plan))
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
        Ok(language_service)
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
        )
    }
}
