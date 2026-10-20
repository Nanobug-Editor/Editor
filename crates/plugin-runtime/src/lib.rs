//! Sandboxed component execution and transactional local package lifecycle.
mod capabilities;
mod dependencies;
pub use dependencies::{InstallControl, InstallStage, InstallerPrompt};
mod data_transaction;
mod document_events;
mod document_stream;
pub use document_stream::DocumentStream;
mod workspace_events;
pub use workspace_events::WorkspaceEvents;
mod diagnostics;
mod editor_requests;
pub use diagnostics::DiagnosticCollection;
mod direct;
pub use direct::{DirectProvider, LanguageFixOrigin, RequestCancellation};
mod decorations;
mod http;
mod trees;
mod virtual_documents;
pub use decorations::DecorationResource;
pub use trees::{TreeBranch, TreeResource, TreeSnapshot};
pub use virtual_documents::VirtualDocumentResource;
mod images;
pub use http::HttpConfiguration;
pub use images::{ImageResource, ImageState};
mod image_input;
pub use image_input::{
    HostImageInput, HostImageOrigin, IMAGE_INPUT_MAX_BATCH, IMAGE_INPUT_MAX_BATCH_BYTES,
    IMAGE_INPUT_MAX_BYTES, IMAGE_INPUT_MAX_RESIDENT_BYTES, IMAGE_INPUT_TIMEOUT_MS,
    ImageInputResource,
};
pub mod faults;
pub mod logs;
pub use logs::{LogLevel, LogRecord, RuntimeLogs};
mod request_state;
pub use document_events::DocumentEvents;
pub use editor_requests::{EditorAuthority, EditorRequest, LanguageControlMetadata};
mod workspace_resources;
pub use workspace_resources::{AuthorizedFile, FileGuard, WorkspaceBatch, WorkspaceRequest};
/// Secure disk preparation and current-session recovery do not own native document text or history.
pub mod workspace_io;
/// A caller-owned gate for one request; public so a host caller can await its own answer.
pub use plugin_protocol::api::RequestUpdate;
pub use request_state::Completion;
mod instance;
mod language_bindings;
mod language_contexts;
mod language_service;
pub use language_bindings::LanguageBindings;
pub use language_contexts::LanguageContexts;
pub use language_service::{LanguageService, ServiceProcess};
/// Shared project builds, validated development directories and atomic ZIP publication.
pub mod development;
mod structure;
pub use structure::{StructureProvider, StructureSnapshot};
mod manager;
/// Reviewed static catalogs and bounded downloads share the public package validation path.
pub mod marketplace;
mod migration;
/// Trusted native host features share the runtime's bounded process-tree supervisor.
pub mod native_processes;
mod package;
mod plugin_services;
mod process;
mod toolchains;
pub use instance::Instance;
pub use manager::{
    ActivationReceipt, Availability, DEBUG_CONTRACT, DEFAULT_STOP_GRACE_MS, DebugAbilities,
    DebugAnswer, DebugBreakpoint, DebugFrame, DebugRequest, DebugSession, DebugState,
    DebugVariable, DemandPreparation, DemandReason, DemandRequest, EXECUTION_CONTRACT,
    EXECUTION_START_TIMEOUT_MS, ExecutionFailure, ExecutionSnapshot, ExecutionState, HostExecution,
    InstallationPreparation, Installed, LanguageRole, Manager, NativeExecutionUpdate,
    PreparedDemand, PreparedInstallation, ProviderCandidate, RunEnvEntry, RunRequest, StopOptions,
    TargetOrigin, TargetRequest, debug_dependency_for_test, frames_from_value,
    variables_from_value,
};
mod native_work;
mod terminal_presentations;
pub use native_work::{PreparationRegistry, PreparationSnapshot};
pub use package::Package;
pub use plugin_protocol;
pub use terminal_presentations::{TerminalOwner, TerminalPresentation, TerminalPresentations};

/// Host-owned resources travel with every candidate; guests only see negotiated descriptions.
#[derive(Clone, Debug, Default)]
pub struct HostResources {
    /// Opaque project choices retain native scope/owner/configuration seals across source routing.
    pub contexts: LanguageContexts,
    /// General HTTP producers share a bounded host budget and host-selected proxy/TLS policy.
    pub http: HttpConfiguration,
    /// Trusted host-only profile override; an empty value hashes the canonical manager root.
    /// Guests and plugin settings cannot select it; update copies never change the namespace.
    pub secret_namespace: String,
    /// Native terminal projection is shared across providers; it never owns or duplicates a child.
    pub terminals: TerminalPresentations,
    /// Authenticated preparation observation shared with all delegated native-process owners.
    pub preparations: native_work::PreparationRegistry,
    /// No SDK is a supported host configuration; an export failure affects only a requesting guest.
    pub sdk: Option<Result<plugin_protocol::api::SdkDescriptor, String>>,
    /// One process-local log owner is shared by live, prepared and retired sources; never exposed to WASI.
    pub logs: RuntimeLogs,
    /// Immutable native module publications and consumer leases share the same manager lifetimes.
    pub modules: NativeModules,
    /// Delegated language providers redeem only native plans sealed to their original document intent.
    pub bindings: LanguageBindings,
}
mod native_modules;
mod native_path;
pub use native_modules::{
    NativeModuleAttachment, NativeModuleDescriptor, NativeModuleLease, NativeModules,
};
