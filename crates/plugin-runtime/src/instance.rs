//! Component isolation, capability enforcement and bounded guest calls.
use super::process::Processes;
use plugin_protocol::*;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use wasmtime::{
    Engine, Store,
    component::{Component, HasSelf, Linker},
};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxView, WasiView};
mod capability_calls;
mod cloud;
mod commands;
mod compiled;
mod data_files;
mod decorations;
mod diagnostics;
mod direct;
mod document_events;
mod editor_requests;
pub(crate) mod file_discovery;
mod icons;
mod image_inputs;
mod language_contexts;
mod language_control;
mod observation;
mod plugin_services;
mod preferences;
mod presentation;
mod private;
mod process_calls;
mod resource_roots;
mod scene_history;
mod selected_resources;
mod service_preparation;
mod service_replies;
mod settings;
mod stdio;
mod structure;
mod terminal_calls;
mod tool_metadata;
mod tools;
mod trees;
mod workspace_events;
mod workspace_files;
pub(crate) use compiled::CompiledCode;
use resource_roots::ResourceRoots;
pub(crate) use workspace_files::NativeDocumentRequests;
wasmtime::component::bindgen!({path:"../plugin-protocol/wit",world:"plugin",require_store_data_send:true});

struct State {
    /// Safe metadata cached once; diagnostics never retain the original scope path.
    observation_owner: Option<crate::observability::Owner>,
    /// Host correlation for imports nested in the current guest callback.
    observed_parent: Option<u64>,
    plugin_services: plugin_services::Services,
    /// Every instance has one negotiated capability transport; there is no legacy fallback.
    api: api::Negotiated,
    /// Extended menu metadata requires an explicitly declared floor, not only a newer host inventory.
    tree_menu_1_1: bool,
    /// Cached metadata updates must opt in to 1.2 without silently widening older tree contracts.
    tree_update_1_2: bool,
    /// Initial tree location and semantic tones require an explicit 1.3 baseline.
    tree_presentation_1_3: bool,
    /// Structural cache deltas require an explicit 1.4 minimum, independently of host inventory.
    tree_patch_1_4: bool,
    /// Non-default tree pointer gestures require an explicit 1.5 minimum.
    tree_interaction_1_5: bool,
    /// Required tree floors explicitly admit cascading action menus and single-click directories.
    tree_action_menus_1_6: bool,
    /// Checkboxes, inline counts and disclosure requests require an explicit 1.7 floor.
    tree_checks_1_7: bool,
    /// Immutable metadata never becomes a preopened directory or guest-controlled file root.
    host_resources: crate::HostResources,
    native_diagnostics: crate::faults::NativeDiagnostics,
    /// Native discovery shares the enclosing WASM call deadline instead of refreshing it per request.
    call_deadline: std::time::Instant,
    roots: ResourceRoots,
    /// Slots and pending completions are owned by this exact WASM instance.
    editor_requests: std::collections::BTreeMap<u64, crate::editor_requests::PendingRequest>,
    /// Native inputs and their slots share this incarnation; pending writers retain their own immutable payload.
    image_inputs: std::collections::BTreeMap<u64, image_inputs::Input>,
    /// User-selected targets remain separate from workspace and private-data roots.
    selected_resources: std::collections::BTreeMap<u64, selected_resources::Selection>,
    /// IO leases and recovery slots remain separate from mutable native document state.
    file_root_lifetimes:
        std::collections::BTreeMap<u64, std::sync::Arc<std::sync::atomic::AtomicBool>>,
    workspace_batches: std::collections::BTreeMap<u64, std::sync::Arc<crate::WorkspaceBatch>>,
    file_subscriptions: std::collections::BTreeMap<u64, crate::workspace_events::Subscription>,
    /// Pull streams and timers reuse this instance's revocable handle table.
    http_streams: std::collections::BTreeMap<u64, crate::http::Stream>,
    timers: std::collections::BTreeMap<u64, cloud::Timer>,
    declared_panels: BTreeSet<String>,
    /// Editor toolbar authority is confined to this package's declared workspace preview surfaces.
    declared_editor_panels: BTreeSet<String>,
    /// Auxiliary tools never gain authority to publish the whole file layout.
    declared_layout_panels: BTreeSet<String>,
    /// Work windows bind explicitly opened documents rather than a selected preview file.
    declared_window_panels: BTreeSet<String>,
    subscriptions: std::collections::BTreeMap<u64, crate::document_events::Subscription>,
    /// Rich events require a separate opt-in handle so older SDKs never see unknown variants.
    document_streams: std::collections::BTreeMap<u64, crate::document_stream::Subscription>,
    /// Persistent readonly authority survives request completion but not instance retirement.
    virtual_documents:
        std::collections::BTreeMap<u64, std::sync::Arc<crate::VirtualDocumentResource>>,
    /// The native document owns diagnostic data; runtime slots retain only source authority.
    diagnostic_collections:
        std::collections::BTreeMap<u64, std::sync::Arc<crate::DiagnosticCollection>>,
    /// Owned metadata survives UI publication, while retirement seals every native projection.
    trees: std::collections::BTreeMap<u64, std::sync::Arc<crate::TreeResource>>,
    /// Workspace chrome and filename metadata are revoked with their exact publisher incarnation.
    presentations:
        std::collections::BTreeMap<u64, std::sync::Arc<crate::WorkspacePresentationResource>>,
    /// Marker collections retain only unforgeable lifetime/document metadata; native text owns positions.
    decorations: std::collections::BTreeMap<u64, std::sync::Arc<crate::DecorationResource>>,
    preference_subscriptions: std::collections::BTreeMap<u64, preferences::Subscription>,
    wasi: WasiCtx,
    table: ResourceTable,
    limits: crate::faults::MemoryBudget,
    permissions: BTreeSet<String>,
    processes: Processes,
    /// Service definitions and process handles cannot be retargeted by guest request fields.
    services: std::collections::BTreeMap<String, plugin_protocol::process::Service>,
    /// Native control is restricted to declarations owned by this exact package incarnation.
    language_providers: std::collections::BTreeMap<String, plugin_protocol::language::Provider>,
    /// Validated package-only exports become visible only after activation has committed.
    module_exports:
        std::collections::BTreeMap<String, plugin_protocol::language_control::ModuleExport>,
    process_handles: std::collections::BTreeMap<u64, (api::ResourceHandle, String)>,
    /// Original calling workspace stays fixed even for resources held by an application provider.
    process_display_scopes: std::collections::BTreeMap<u64, String>,
    /// Each native process separately pins its immutable runtime files until it exits or is retired.
    process_dependencies: std::collections::BTreeMap<u64, Vec<std::sync::Arc<std::fs::File>>>,
    workspace: PathBuf,
    data: PathBuf,
    assets: PathBuf,
    active: bool,
    /// The trusted manager opens private directory authority and irreversible credential mutations
    /// only after durable publication.
    /// Flushing staged private files is preparation work and never grants this authority.
    activation_committed: bool,
    /// A bounded discovery invocation can read granted resources without launching native work.
    language_hook: bool,
    /// Package opt-in and current callback identity never grant native execution authority.
    service_preparation_hook: bool,
    service_preparation_call: bool,
    /// Separate stateless language workers reject every host operation, including during Prepare.
    language_pure: bool,
    /// Typed outputs are accepted only while the matching pure callback is running.
    language_completion_call: bool,
    /// Only this callback may publish a structure proposal; it cannot impersonate completion data.
    language_structure_call: bool,
    /// Only the experimental pure callback may return direct feature data.
    language_direct_call: bool,
    /// Read-only discovery can release only the file roots allocated by its own hook.
    language_hook_checkpoint: Option<u64>,
    /// Migration grants access exclusively to a transaction's isolated private-data copy.
    migrating: bool,
    /// Initialization writes commit only with the version switch; failed activation cannot edit old settings.
    staged_writes: Option<std::collections::BTreeMap<PathBuf, Vec<u8>>>,
}
impl WasiView for State {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}
impl editor::plugin::host::Host for State {
    /// Deny both ungranted calls and side effects during update preparation.
    fn request(&mut self, payload: String) -> Result<String, String> {
        self.capability_request(&payload)
    }
}
/// Reject traversal, absolute/device paths and symlink escapes before any host file access.
pub(crate) fn safe_path(root: &Path, relative: &str, existing: bool) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(
        !relative.is_empty()
            && !relative.contains(['\\', ':'])
            && !relative.split('/').any(api::is_windows_device_segment)
            && Path::new(relative)
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_))),
        "Unsafe relative path"
    );
    let root = root.canonicalize()?;
    let path = root.join(relative);
    let checked = if existing || path.exists() {
        path.canonicalize()?
    } else {
        let parent = path.parent().unwrap().canonicalize()?;
        parent.join(path.file_name().unwrap())
    };
    anyhow::ensure!(checked.starts_with(&root), "Path escapes capability root");
    Ok(checked)
}
#[cfg(test)]
mod tests {
    use super::*;
    /// File capabilities stay under their canonical root, including absent write destinations.
    #[test]
    fn file_capability_cannot_escape_its_root() {
        let root = tempfile::tempdir().unwrap();
        for path in [
            "../outside",
            "/outside",
            "C:/outside",
            "a/../../outside",
            "a\\outside",
        ] {
            assert!(safe_path(root.path(), path, false).is_err());
        }
        assert!(safe_path(root.path(), "settings.json", false).is_ok());
    }
}

/// One isolated plugin has its own Store, WASI table, resource handles and fuel budget.
pub struct Instance {
    pub(crate) diagnostics: Vec<crate::faults::Diagnostic>,
    /// UI preview publication is tied to the latest authorized input for each declared surface.
    pub(crate) preview_sources: std::collections::BTreeMap<String, Option<api::DocumentVersion>>,
    /// File authority is independent of native text revisions and revoked on file/provider changes.
    pub(crate) file_sources: std::collections::BTreeMap<String, Option<api::FileContext>>,
    /// Configuration belongs to the same owner as its runtime resources, not the currently selected workspace.
    pub(crate) configuration: plugin_protocol::settings::Effective,
    /// Host invocation IDs cannot be confused with stale completions after another call.
    next_call: u64,
    /// Immutable code may initialize later isolated role Stores while this exact owner remains live.
    compiled: CompiledCode,
    store: Store<State>,
    bindings: Plugin,
    /// Validated documents keyed by their declared panel; publication is atomic across one reply.
    pub views: std::collections::BTreeMap<String, std::sync::Arc<ui::Document>>,
    /// Bounded incarnation-local tombstones prevent removed scene identities from acquiring new input.
    scene_identities: BTreeSet<(String, String, u64)>,
    /// Last successful native presentation, scoped to this incarnation's panel and canvas node.
    scene_presentations: std::collections::BTreeMap<(String, String), ui::CanvasEvent>,
    panels: std::collections::BTreeSet<String>,
    pub error: Option<String>,
}
impl Instance {
    /// The identity changes on recovery as well as replacement; old host callbacks must not follow it.
    pub(crate) fn instance_id(&self) -> Option<&str> {
        self.store.data().active.then_some(
            self.store
                .data()
                .plugin_services
                .principal
                .instance
                .as_str(),
        )
    }
    pub fn engine() -> anyhow::Result<Engine> {
        let mut config = wasmtime::Config::new();
        config
            .wasm_component_model(true)
            .consume_fuel(true)
            .epoch_interruption(true);
        let engine = Engine::new(&config)?;
        let weak = engine.weak();
        // A weak engine reference lets the shared watchdog terminate after the manager is dropped.
        std::thread::Builder::new()
            .name("plugin-epoch-clock".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(
                        crate::faults::EPOCH_TICK_MS,
                    ));
                    let Some(engine) = weak.upgrade() else { break };
                    engine.increment_epoch();
                }
            })?;
        Ok(engine)
    }
    /// Embedders without host SDK metadata retain the ordinary component preparation entry point.
    pub fn prepare(
        engine: &Engine,
        bytes: &[u8],
        manifest: &Manifest,
        grants: &BTreeSet<String>,
        environment: Environment,
        data: PathBuf,
        assets: PathBuf,
        snapshot: Option<Snapshot>,
    ) -> anyhow::Result<Self> {
        Self::prepare_with_resources(
            engine,
            bytes,
            manifest,
            grants,
            environment,
            data,
            assets,
            snapshot,
            Default::default(),
        )
    }
    /// Every candidate receives the same immutable host inputs before any lifecycle callback.
    pub(crate) fn prepare_with_resources(
        engine: &Engine,
        bytes: &[u8],
        manifest: &Manifest,
        grants: &BTreeSet<String>,
        environment: Environment,
        data: PathBuf,
        assets: PathBuf,
        snapshot: Option<Snapshot>,
        host_resources: crate::HostResources,
    ) -> anyhow::Result<Self> {
        Self::prepare_with_mode(
            engine,
            bytes,
            manifest,
            grants,
            environment,
            data,
            assets,
            snapshot,
            host_resources,
            None,
            false,
        )
    }
    /// Language workers never restore a private snapshot or activate package resources.
    pub(crate) fn prepare_language_worker(
        engine: &Engine,
        bytes: &[u8],
        manifest: &Manifest,
        grants: &BTreeSet<String>,
        environment: Environment,
        data: PathBuf,
        assets: PathBuf,
        host_resources: crate::HostResources,
        compiled: &CompiledCode,
    ) -> anyhow::Result<Self> {
        Self::prepare_with_mode(
            engine,
            bytes,
            manifest,
            grants,
            environment,
            data,
            assets,
            None,
            host_resources,
            Some(compiled),
            true,
        )
    }
    /// Build the same isolated transport, choosing pure authority before the first guest instruction runs.
    fn prepare_with_mode(
        engine: &Engine,
        bytes: &[u8],
        manifest: &Manifest,
        grants: &BTreeSet<String>,
        mut environment: Environment,
        data: PathBuf,
        assets: PathBuf,
        snapshot: Option<Snapshot>,
        host_resources: crate::HostResources,
        compiled: Option<&CompiledCode>,
        pure: bool,
    ) -> anyhow::Result<Self> {
        let api = super::capabilities::negotiate(manifest)?;
        manifest
            .plugin_services
            .validate()
            .map_err(anyhow::Error::msg)?;
        let application = manifest.scope == api::InstanceScope::Application;
        if application {
            environment.workspace.clear();
        }
        anyhow::ensure!(
            manifest.permissions.is_subset(grants),
            "Plugin needs additional permission consent"
        );
        if !pure {
            std::fs::create_dir_all(&data)?;
        }
        let compiled = CompiledCode::prepare(
            engine,
            bytes,
            compiled,
            &host_resources.logs,
            &manifest.id,
            pure,
        )?;
        let component = compiled.component();
        let mut linker = Linker::<State>::new(engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;
        Plugin::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)?;
        let stdout = stdio::capture(
            host_resources.logs.clone(),
            &manifest.id,
            crate::LogLevel::Info,
            "wasi/stdout",
        )?;
        let stderr = stdio::capture(
            host_resources.logs.clone(),
            &manifest.id,
            crate::LogLevel::Warning,
            "wasi/stderr",
        )?;
        let native_diagnostics =
            crate::faults::NativeDiagnostics::with_logs(host_resources.logs.clone());
        let mut wasi = WasiCtx::builder();
        wasi.stdout(stdout).stderr(stderr);
        // No preopened directories, environment inheritance or network access is granted to WASI.
        let mut state = State {
            observation_owner: None,
            observed_parent: None,
            plugin_services: Default::default(),
            api,
            host_resources,
            native_diagnostics,
            call_deadline: std::time::Instant::now(),
            roots: ResourceRoots::new(&environment.workspace, application, manifest.storage_limit),
            editor_requests: Default::default(),
            image_inputs: Default::default(),
            selected_resources: Default::default(),
            file_root_lifetimes: Default::default(),
            workspace_batches: Default::default(),
            file_subscriptions: Default::default(),
            http_streams: Default::default(),
            timers: Default::default(),
            subscriptions: Default::default(),
            document_streams: Default::default(),
            virtual_documents: Default::default(),
            diagnostic_collections: Default::default(),
            trees: Default::default(),
            presentations: Default::default(),
            tree_menu_1_1: trees::requires_menu_floor(manifest.api.as_ref()),
            tree_update_1_2: trees::requires_cached_update_floor(manifest.api.as_ref()),
            tree_presentation_1_3: trees::requires_presentation_floor(manifest.api.as_ref()),
            tree_patch_1_4: trees::requires_patch_floor(manifest.api.as_ref()),
            tree_interaction_1_5: trees::requires_interaction_floor(manifest.api.as_ref()),
            tree_action_menus_1_6: trees::requires_action_menu_floor(manifest.api.as_ref()),
            tree_checks_1_7: trees::requires_check_floor(manifest.api.as_ref()),
            decorations: Default::default(),
            preference_subscriptions: Default::default(),
            declared_panels: manifest
                .panels
                .iter()
                .map(|panel| panel.id.clone())
                .collect(),
            declared_editor_panels: manifest
                .panels
                .iter()
                .filter(|panel| panel.position == "editor")
                .map(|panel| panel.id.clone())
                .collect(),
            declared_layout_panels: manifest
                .panels
                .iter()
                .filter(|panel| {
                    matches!(panel.position.as_str(), "editor" | "window") && !panel.auxiliary
                })
                .map(|panel| panel.id.clone())
                .collect(),
            declared_window_panels: manifest
                .panels
                .iter()
                .filter(|panel| panel.position == "window")
                .map(|panel| panel.id.clone())
                .collect(),
            wasi: wasi.build(),
            table: ResourceTable::new(),
            limits: Default::default(),
            permissions: manifest.permissions.clone(),
            processes: Processes::default(),
            services: manifest.services.clone(),
            language_providers: manifest
                .language_servers
                .iter()
                .map(|provider| (provider.id.clone(), provider.clone()))
                .collect(),
            process_handles: Default::default(),
            module_exports: manifest.module_exports.clone(),
            process_display_scopes: Default::default(),
            process_dependencies: Default::default(),
            workspace: PathBuf::from(&environment.workspace),
            data,
            assets,
            active: false,
            activation_committed: false,
            language_hook: false,
            service_preparation_hook: manifest.service_preparation_hook,
            service_preparation_call: false,
            language_pure: pure,
            language_completion_call: false,
            language_structure_call: false,
            language_direct_call: false,
            language_hook_checkpoint: None,
            migrating: false,
            staged_writes: Some(Default::default()),
        };
        state.plugin_services.principal =
            state.roots.principal(&manifest.id, &manifest.permissions);
        let principal = &state.plugin_services.principal;
        let mut owner = crate::observability::Owner::new(
            &principal.plugin,
            &principal.instance,
            &principal.scope,
        );
        // SemVer build metadata can be arbitrarily large. Diagnostics omit oversized identity
        // metadata rather than copying it on every call or pretending a truncated version is real.
        if manifest.version.len() <= 128 {
            owner.package_version = Some(manifest.version.clone());
        } else {
            owner.metadata_partial = true;
        }
        // Package-owned asset roots use an immutable digest directory. Arbitrary host test roots
        // have no such evidence and deliberately report unavailable metadata.
        owner.package_digest = state
            .assets
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|digest| {
                digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
            .map(str::to_owned);
        state.observation_owner = Some(owner);
        state.plugin_services.declarations = manifest.plugin_services.clone();
        state.plugin_services.commands = manifest
            .commands
            .iter()
            .filter_map(|command| {
                command
                    .signature
                    .clone()
                    .map(|method| (command.id.clone(), method))
            })
            .collect();
        let mut store = Store::new(engine, state);
        store.limiter(|s| &mut s.limits);
        store.set_fuel(100_000_000)?;
        store.set_epoch_deadline(crate::faults::CALL_DEADLINE_MS / crate::faults::EPOCH_TICK_MS);
        let bindings =
            Plugin::instantiate(&mut store, component, &linker).inspect_err(|error| {
                store.data().host_resources.logs.append(
                    &manifest.id,
                    crate::LogLevel::Error,
                    "wasm/instantiate",
                    format!("{error:#}"),
                );
            })?;
        let mut instance = Self {
            diagnostics: Vec::new(),
            preview_sources: Default::default(),
            file_sources: Default::default(),
            configuration: Default::default(),
            next_call: 1,
            compiled,
            store,
            bindings,
            views: Default::default(),
            scene_identities: Default::default(),
            scene_presentations: Default::default(),
            panels: manifest.panels.iter().map(|p| p.id.clone()).collect(),
            error: None,
        };
        instance.prepare_state(environment, snapshot)?;
        Ok(instance)
    }
    /// Immutable negotiated authority lets manager ingress reject unavailable optional interfaces.
    pub(crate) fn negotiated(&self) -> &api::Negotiated {
        &self.store.data().api
    }

    /// Every call gets a finite instruction budget; a trap cannot unwind through the host.
    pub fn call(&mut self, message: api::Input) -> anyhow::Result<api::Output> {
        anyhow::ensure!(
            !self.store.data().roots.retired,
            "Plugin is paused; restart this instance"
        );
        let operation = crate::faults::operation(&message);
        let stage = match &message {
            api::Input::Activate => crate::observability::Stage::Activation,
            api::Input::Prepare { .. } => crate::observability::Stage::Preparation,
            _ => crate::observability::Stage::Guest,
        };
        let mut observation = self.store.data().host_resources.observations.begin(
            self.store.data().observation_identity(),
            stage,
            None,
        );
        self.store.data_mut().observed_parent = Some(observation.id());
        let result = self.call_inner(message);
        self.store.data_mut().observed_parent = None;
        // call_inner installs a fresh finite budget for each invocation; remaining fuel is
        // meaningful only relative to that budget, never as a wall-clock CPU measurement.
        observation.fuel(self.store.get_fuel().ok().map(|remaining| {
            let budget = if operation == "snapshot" {
                crate::faults::SNAPSHOT_FUEL
            } else {
                crate::faults::CALL_FUEL
            };
            budget.saturating_sub(remaining)
        }));
        observation.finish(if result.is_ok() {
            crate::observability::Outcome::Completed
        } else {
            crate::observability::Outcome::Failed
        });
        self.sample_resources();
        if let Err(error) = &result {
            // A rejected scene is a typed publication result, not a WASM trap. Keep the
            // committed owner alive before generic fault handling can revoke its old views.
            let rejected_scene = self.preserve_rejected_scene(error);
            let principal = &self.store.data().plugin_services.principal;
            let budget = error.downcast_ref::<wasmtime::Trap>().is_some_and(|trap| {
                matches!(trap, wasmtime::Trap::OutOfFuel | wasmtime::Trap::Interrupt)
            });
            let message: String = format!(
                "{}: {error:#}",
                if budget {
                    "WASM execution budget exceeded"
                } else if rejected_scene {
                    "Canvas publication rejected"
                } else {
                    "WASM operation failed"
                }
            )
            .chars()
            .take(4096)
            .collect();
            self.error = Some(format!(
                "{} [{}] {operation}: {message}",
                principal.plugin, principal.scope
            ));
            // Log at the originating call, not when a later UI selection happens to publish diagnostics.
            self.store.data().host_resources.logs.append(
                &principal.plugin,
                crate::LogLevel::Error,
                &format!("wasm/{operation}"),
                format!("scope={} {message}", principal.scope),
            );
            self.diagnostics.push(crate::faults::Diagnostic::new(
                principal.plugin.clone(),
                principal.scope.clone(),
                operation,
                message,
            ));
            if self.diagnostics.len() > 32 {
                self.diagnostics.remove(0);
            }
            if error.downcast_ref::<api::Failure>().is_none() && !rejected_scene {
                // Publish the provider fault before retirement cancels retained invocations.
                // Otherwise a trap is silently rewritten as a caller-initiated cancellation.
                let failure = api::Failure::new(
                    api::ErrorCode::OperationFailed,
                    self.error
                        .clone()
                        .unwrap_or_else(|| "Service provider failed".into()),
                );
                for incoming in self.store.data().plugin_services.incoming.values() {
                    incoming.call.completion.finish(Err(failure.clone()));
                }
                self.stop();
            }
        }
        result.map_err(|error| {
            let principal = &self.store.data().plugin_services.principal;
            let operation = self
                .diagnostics
                .last()
                .map(|report| report.operation.as_str())
                .unwrap_or("call");
            error.context(format!(
                "plugin={} scope={} operation={operation}",
                principal.plugin, principal.scope
            ))
        })
    }
    /// Validation and publication share the same budgeted call; a failing result never partially publishes UI.
    fn call_inner(&mut self, mut message: api::Input) -> anyhow::Result<api::Output> {
        anyhow::ensure!(
            !self.store.data().roots.retired,
            "Instance has been retired"
        );
        // Application owners receive appearance updates without inheriting a selected workspace.
        if self.store.data().roots.application {
            match &mut message {
                api::Input::Prepare { environment, .. }
                | api::Input::Event {
                    event: api::Notification::Theme(environment),
                    ..
                } => environment.workspace.clear(),
                _ => {}
            }
        }
        // Snapshot serialization may traverse the full configured scrollback, while input and
        // paint events stay on the smaller interactive budget. Both calls remain fuel-bounded.
        let fuel = if matches!(message, api::Input::Snapshot) {
            crate::faults::SNAPSHOT_FUEL
        } else {
            crate::faults::CALL_FUEL
        };
        self.store.set_fuel(fuel)?;
        let timeout = if matches!(message, api::Input::Snapshot) {
            crate::faults::SNAPSHOT_DEADLINE_MS
        } else {
            crate::faults::CALL_DEADLINE_MS
        };
        self.store
            .set_epoch_deadline(timeout / crate::faults::EPOCH_TICK_MS);
        self.store.data_mut().call_deadline =
            std::time::Instant::now() + std::time::Duration::from_millis(timeout);
        let (payload, invocation_id) = self.encode_invocation(message)?;
        let result = self
            .bindings
            .call_dispatch(&mut self.store, &payload)?
            .map_err(anyhow::Error::msg)?;
        // Observe only encoded size and validation/publication time; never retain guest UI bytes.
        let mut publication = self.store.data().host_resources.observations.begin(
            self.store.data().observation_identity(),
            crate::observability::Stage::Publication,
            self.store.data().observed_parent,
        );
        publication.bytes(result.len());
        let published = (|| {
            anyhow::ensure!(
                result.len() <= 32 * 1024 * 1024,
                "Plugin reply quota exceeded"
            );
            let reply = self.decode_completion(&result, invocation_id)?;
            let mut panels = std::collections::BTreeSet::new();
            for view in &reply.views {
                anyhow::ensure!(
                    self.panels.contains(&view.panel),
                    "Undeclared panel: {}",
                    view.panel
                );
                anyhow::ensure!(
                    panels.insert(&view.panel),
                    "Duplicate panel in plugin reply"
                );
            }
            // Publish only after every document validates; an invalid peer never partly updates the UI.
            self.remember_scenes(&reply.views);
            for view in &reply.views {
                self.views.insert(
                    view.panel.clone(),
                    std::sync::Arc::new(view.document.clone()),
                );
            }
            self.reconcile_scene_presentations();
            // Revoke against the newly published opt-in, before a second public event can use old slots.
            // Accepted writers pin their payload independently and still deliver the original save receipt.
            self.reconcile_image_inputs();
            self.error = None;
            Ok(reply)
        })();
        publication.finish(if published.is_ok() {
            crate::observability::Outcome::Completed
        } else {
            crate::observability::Outcome::Failed
        });
        published
    }
    pub fn activate(&mut self) -> anyhow::Result<()> {
        self.store.data_mut().active = true;
        self.call(api::Input::Activate)?;
        Ok(())
    }
    /// Attach before activation so native retirement reports survive this particular incarnation.
    pub(crate) fn connect_diagnostics(&mut self, diagnostics: crate::faults::NativeDiagnostics) {
        self.store.data_mut().native_diagnostics = diagnostics;
    }
    /// Preparation always supplies this instance's negotiated interfaces, including on rollback.
    pub(crate) fn prepare_state(
        &mut self,
        environment: Environment,
        snapshot: Option<Snapshot>,
    ) -> anyhow::Result<api::Output> {
        self.call(api::Input::Prepare {
            environment,
            snapshot,
            api: self.store.data().api.clone(),
        })
    }
    /// Native owners route typed notifications without a second event codec.
    pub fn notify(
        &mut self,
        panel: Option<String>,
        event: api::Notification,
    ) -> anyhow::Result<api::Output> {
        self.call(api::Input::Event { panel, event })
    }
    /// Apply buffered initialization writes while retaining originals until registry commit succeeds.
    pub(crate) fn commit_data(&mut self) -> anyhow::Result<Vec<(PathBuf, Option<Vec<u8>>)>> {
        let writes = self
            .store
            .data_mut()
            .staged_writes
            .take()
            .unwrap_or_default();
        let mut originals = vec![];
        let result = (|| {
            for (path, bytes) in writes {
                let old = match std::fs::read(&path) {
                    Ok(bytes) => Some(bytes),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                    Err(e) => return Err(e.into()),
                };
                originals.push((path.clone(), old));
                super::package::atomic_write(&path, &bytes)?;
            }
            Ok::<_, anyhow::Error>(())
        })();
        if let Err(error) = result {
            Self::rollback_data(&originals)?;
            return Err(error);
        }
        Ok(originals)
    }
    /// Publish authority only after the owner manager has committed its version/configuration.
    pub(crate) fn commit_activation(&mut self) {
        debug_assert!(
            self.store.data().active
                && !self.store.data().roots.retired
                && self.store.data().staged_writes.is_none()
        );
        self.store.data_mut().activation_committed = true;
        self.sample_resources();
        let state = self.store.data();
        if state.permissions.contains("native.modules") && !state.module_exports.is_empty() {
            state
                .host_resources
                .modules
                .publish(crate::native_modules::Publisher {
                    instance: state.plugin_services.principal.instance.clone(),
                    scope: state.plugin_services.principal.scope.clone(),
                    root: state.assets.clone(),
                    digest: state
                        .assets
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    alive: state.plugin_services.alive.clone(),
                    exports: state.module_exports.clone(),
                });
        }
    }
    /// Restore only files touched by this transaction; unrelated plugin data stays intact.
    pub(crate) fn rollback_data(originals: &[(PathBuf, Option<Vec<u8>>)]) -> anyhow::Result<()> {
        for (path, bytes) in originals.iter().rev() {
            if let Some(bytes) = bytes {
                super::package::atomic_write(path, bytes)?;
            } else if path.exists() {
                std::fs::remove_file(path)?;
            }
        }
        Ok(())
    }
    /// Seal published work at retirement, after any final snapshot has used its still-live owned resources.
    pub(crate) fn quiesce(&mut self) {
        // Revoke registered configuration metadata before another callback can select its old plan.
        self.store
            .data()
            .host_resources
            .contexts
            .revoke_owner(&self.store.data().plugin_services.principal.instance);
        // Stop borrowed native workers before revoking source metadata or allowing package cleanup.
        self.store
            .data()
            .host_resources
            .modules
            .revoke_owner(&self.store.data().plugin_services.principal.instance);
        self.store
            .data()
            .host_resources
            .bindings
            .revoke_owner(&self.store.data().plugin_services.principal.instance);
        self.store.data_mut().activation_committed = false;
        self.store.data_mut().http_streams.clear();
        self.store.data_mut().timers.clear();
        self.store.data_mut().clear_temporary_data();
        self.clear_image_inputs();
        self.store.data_mut().clear_selections();
        self.store.data_mut().clear_workspace_resources();
        self.store.data_mut().file_subscriptions.clear();
        // Native cleanup keeps its original observer before quiesce seals delegated service roots.
        self.store.data_mut().retire_native_processes();
        self.store.data_mut().plugin_services.clear();
        self.store.data_mut().subscriptions.clear();
        self.store.data_mut().document_streams.clear();
        for resource in self.store.data_mut().virtual_documents.values() {
            resource.revoke();
        }
        self.store.data_mut().virtual_documents.clear();
        for collection in self.store.data_mut().diagnostic_collections.values() {
            collection.revoke();
        }
        self.store.data_mut().diagnostic_collections.clear();
        for tree in self.store.data().trees.values() {
            tree.revoke();
        }
        self.store.data_mut().trees.clear();
        for resource in self.store.data().presentations.values() {
            resource.revoke();
        }
        self.store.data_mut().presentations.clear();
        for resource in self.store.data().decorations.values() {
            resource.revoke();
        }
        self.store.data_mut().decorations.clear();
        self.store.data_mut().preference_subscriptions.clear();
        for request in self.store.data_mut().editor_requests.values() {
            request.call.retire();
        }
        self.store.data_mut().editor_requests.clear();
    }
    pub fn stop(&mut self) {
        self.quiesce();
        self.preview_sources.clear();
        self.file_sources.clear();
        self.store.data_mut().roots.retire();
        self.views.clear();
        self.scene_presentations.clear();
        self.store.data_mut().processes.clear();
        self.store.data_mut().process_handles.clear();
        self.store.data_mut().process_display_scopes.clear();
        self.store.data_mut().process_dependencies.clear();
        self.store.data_mut().active = false;
        // Only the final real maps can attest to cleanup; an earlier sample retains stale process slots.
        self.sample_resources();
    }
    pub fn process_count(&self) -> usize {
        self.store.data().processes.len()
    }
    /// Observable ownership count includes native views and file/process resources.
    pub fn resource_count(&self) -> usize {
        self.store.data().roots.len() + self.views.len() + self.process_count()
    }
    /// Immutable instance authority is shared by every native language/diagnostic consumer.
    pub(crate) fn editor_authority(&self) -> crate::EditorAuthority {
        crate::EditorAuthority::new(self.store.data().plugin_services.alive.clone(), None)
    }
    pub fn process_ids(&self) -> Vec<u32> {
        self.store.data().processes.ids()
    }
    pub fn poll(&mut self) -> anyhow::Result<bool> {
        self.reconcile_image_inputs();
        self.retire_service_sources();
        let trees_changed = self.prune_trees()?;
        self.poll_service_replies()?;
        let revoked = self.poll_service_revocations()?;
        self.poll_service_requests()?;
        self.poll_editor_requests()?;
        self.poll_document_events()?;
        self.poll_document_streams()?;
        self.poll_workspace_events()?;
        let preferences_changed = self.poll_preferences()?;
        let timers_changed = self.poll_timers()?;
        let events = self.store.data_mut().poll_processes()?;
        // Tree retirement and timer callbacks both change the instance's published native state.
        let changed =
            revoked || trees_changed || preferences_changed || timers_changed || !events.is_empty();
        for event in events {
            // Exit removes its slot, but the last callback still inherits the originating service authority.
            let context = if let api::Notification::Process { handle, .. } = &event {
                self.store
                    .data()
                    .plugin_services
                    .resources
                    .get(&handle.resource)
                    .map(|(_, context)| context.clone())
            } else {
                None
            };
            if self.store.data_mut().accept_process_event(&event) {
                let finished = if let api::Notification::Process {
                    handle,
                    update: process::Update::Exited { .. } | process::Update::Terminated,
                } = &event
                {
                    Some(handle.resource)
                } else {
                    None
                };
                let result = self
                    .call_with_service_context(context, api::Input::Event { panel: None, event });
                if let Some(slot) = finished {
                    self.store
                        .data_mut()
                        .plugin_services
                        .resources
                        .remove(&slot);
                }
                result?;
            }
        }
        Ok(changed)
    }
    pub fn snapshot(&mut self) -> anyhow::Result<Snapshot> {
        self.call(api::Input::Snapshot)?
            .snapshot
            .ok_or_else(|| anyhow::anyhow!("Plugin did not return its declared state"))
    }
}
