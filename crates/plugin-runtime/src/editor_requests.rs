//! Owned editor operations use the common completion gate across worker and native UI threads.
use crate::request_state::Completion;
#[cfg(test)]
mod language_delivery_tests;
mod preparation;
use plugin_protocol::api::{
    CancelMode, CancellationEffect, EditorOperation, EditorValue, ErrorCode, Failure,
    RequestUpdate, ResourceHandle,
};
pub(crate) use preparation::PreparationConditions;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

/// Runtime-issued, read-only authority for native resources retained after an editor task completes.
/// It borrows the existing instance and delegated caller lifetimes, never a deadline or result gate.
/// Private fields prevent guests or native callers from constructing or reviving authority.
#[derive(Clone)]
pub struct EditorAuthority {
    owner: Arc<AtomicBool>,
    callers: Vec<Arc<AtomicBool>>,
}

impl EditorAuthority {
    /// Issue persistent lifetime metadata only after the instance has checked capability and grants.
    /// Internal issuers share one immutable lifetime chain instead of copying completion semantics.
    pub(crate) fn new(
        owner: Arc<AtomicBool>,
        context: Option<&crate::plugin_services::Context>,
    ) -> Self {
        Self {
            owner,
            callers: context.map_or_else(Vec::new, |context| context.lifetimes.clone()),
        }
    }
    /// Return whether the initiating instance and every delegated source remain authorized.
    /// Completing a task does not retire a persistent view; disable/cutover/trust revocation does.
    pub fn is_live(&self) -> bool {
        self.owner.load(Ordering::Acquire)
            && self
                .callers
                .iter()
                .all(|alive| alive.load(Ordering::Acquire))
    }
    /// A controlled service adds its own retirement boundary while retaining the initiating chain.
    /// Private issuance cannot lend or revive an instance or delegated caller.
    pub(crate) fn with_native_owner(&self, owner: Arc<AtomicBool>) -> Self {
        let mut callers = vec![self.owner.clone()];
        callers.extend(self.callers.iter().cloned());
        Self { owner, callers }
    }
    /// Compare exact sealed lifetime chains when retaining native resources between calls.
    /// `other` must also originate from a checked request. Equality grants no authority: callers
    /// still check `is_live` before use. Display names or package IDs cannot identify an incarnation.
    pub fn same_owner(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.owner, &other.owner)
            && self.callers.len() == other.callers.len()
            && self
                .callers
                .iter()
                .zip(&other.callers)
                .all(|(left, right)| Arc::ptr_eq(left, right))
    }
    /// Compare the issuing instance independently of a shrinking delegated caller chain.
    pub(crate) fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.owner, &other.owner)
    }
    /// A native result may add a monotonic source/configuration seal without changing its issuer.
    /// This narrows an existing lifetime only; it cannot grant permissions or revive an old owner.
    pub(crate) fn with_guard(&self, guard: Arc<AtomicBool>) -> Self {
        let mut authority = self.clone();
        authority.callers.push(guard);
        authority
    }
}

/// Clones share one completion gate; dropping an instance seals every outstanding call.
#[derive(Clone)]
pub struct EditorRequest {
    /// One runtime-owned clipboard snapshot, admitted independently of the asynchronous request slot.
    clipboard_offer: Option<Arc<crate::ClipboardOfferResource>>,
    /// Sealed post-drop data authority, independent of clipboard and filesystem roots.
    transfer_offer: Option<Arc<crate::TransferOfferResource>>,
    handle: ResourceHandle,
    operation: EditorOperation,
    workspace: String,
    /// Trusted runtime metadata; the guest supplies only a relative file name.
    data_root: std::path::PathBuf,
    completion: Completion<EditorValue>,
    /// Persistent views must outlive completion while still observing their initiating authority.
    authority: EditorAuthority,
    /// Only native-offered bytes can back an image save; the JSON operation contains an opaque handle.
    image_input: Option<std::sync::Arc<crate::ImageInputResource>>,
    /// Persistent authority is distinct from this transient completion handle.
    virtual_document: Option<std::sync::Arc<crate::VirtualDocumentResource>>,
    /// Publications retain their admitted collection through the native completion gate.
    diagnostic_collection: Option<std::sync::Arc<crate::DiagnosticCollection>>,
    /// The native request borrows one admitted marker owner and immutable package artwork.
    decoration: Option<std::sync::Arc<crate::DecorationResource>>,
    decoration_icons: std::collections::BTreeMap<String, Arc<[u8]>>,
    /// Immutable owned artwork admitted for native comparison controls, never ambient host paths.
    comparison_icons: std::collections::BTreeMap<String, Arc<[u8]>>,
    /// Source wait cancellation closes pending UI, while successful calls may retain owned work.
    parents: Vec<Completion<serde_json::Value>>,
    /// Native paths never cross the guest transport; the instance issues grants only at delivery.
    selection: std::sync::Arc<std::sync::Mutex<Option<Vec<std::path::PathBuf>>>>,
    /// Replaceable presentation belongs to this one request, never a second cancellable task.
    progress: std::sync::Arc<std::sync::Mutex<(String, Option<u8>)>>,
    /// Checked native roots and a separately releasable recovery resource never come from guest paths.
    workspace_request: Option<crate::WorkspaceRequest>,
    /// Checked provider, original caller and immutable imports never come from guest identity strings.
    language_control: Option<LanguageControlMetadata>,
    /// Only the registered-context gate can issue this native refresh proof.
    language_refresh: Option<(
        crate::LanguageContexts,
        crate::language_contexts::RegisteredContext,
        Option<Arc<AtomicBool>>,
        Arc<std::sync::Mutex<Option<Arc<AtomicBool>>>>,
    )>,
}

/// Runtime-sealed metadata for the existing native language connection, unavailable to guest JSON.
#[derive(Clone)]
pub struct LanguageControlMetadata {
    /// An explicitly registered context narrows this request and its eventual native binding.
    pub(crate) selected_context: Option<crate::language_contexts::RegisteredContext>,
    /// Transient source-choice seal; it never narrows the shared process's persistent authority.
    pub(crate) selection_guard: Option<Arc<AtomicBool>>,
    pub(crate) contexts: crate::LanguageContexts,
    /// Host-owned collection epochs narrow pending results and bindings, including empty discovery.
    pub(crate) collection_guards: Vec<Arc<AtomicBool>>,
    /// Globally scoped provider (`package/local-provider`), owned by this exact issuing instance.
    pub provider: String,
    /// Exact current provider instance, sealed across delegation.
    pub owner: String,
    /// Original consumer incarnation; guest caller labels cannot replace it.
    pub consumer: String,
    /// Immediate caller owns the delegated provider; original consumer ownership remains separate.
    pub borrower: String,
    /// Logical workspace is runtime-issued and never taken from request arguments.
    pub scope: String,
    /// Optional original delegating provider document intent; standard source text is not embedded.
    pub binding_document: Option<plugin_protocol::language::DocumentContext>,
    /// Private issuance registry cannot be selected by a guest.
    pub(crate) bindings: crate::LanguageBindings,
    /// Result admission uses the same closed schema declared by the provider.
    pub response: plugin_protocol::service::Schema,
    /// Native code can attach these leases, but cannot construct a descriptor from an arbitrary path.
    pub imports: Vec<Arc<crate::NativeModuleLease>>,
}
impl EditorRequest {
    /// Observe this accepted request through its existing completion gate, without copying its payload.
    pub(crate) fn observe(&self, observation: crate::observability::Observation) {
        self.completion.observe(observation);
    }
    /// Controlled native owners retain the common completion policy and their complete sealed
    /// service/instance chain, even when no guest context exists to receive a notification.
    pub(crate) fn from_authority(
        handle: ResourceHandle,
        operation: EditorOperation,
        workspace: String,
        data_root: std::path::PathBuf,
        timeout_ms: u32,
        authority: EditorAuthority,
    ) -> Self {
        let mut request = Self::new(
            handle,
            operation,
            workspace,
            data_root,
            timeout_ms,
            authority.owner.clone(),
            None,
        );
        request.completion.lifetimes.push(authority.owner.clone());
        request
            .completion
            .lifetimes
            .extend(authority.callers.iter().cloned());
        request.authority = authority;
        request
    }
    /// Construction follows authority checks in the instance; only typed owned data crosses threads.
    pub(crate) fn new(
        handle: ResourceHandle,
        operation: EditorOperation,
        workspace: String,
        data_root: std::path::PathBuf,
        timeout_ms: u32,
        owner: Arc<AtomicBool>,
        context: Option<&crate::plugin_services::Context>,
    ) -> Self {
        let mut completion = Completion::new(timeout_ms);
        completion.lifetimes = context.map_or_else(Vec::new, |context| context.lifetimes.clone());
        let parents = context.map_or_else(Vec::new, |context| context.native_waits.clone());
        completion
            .lifetimes
            .extend(parents.iter().map(Completion::wait_lifetime));
        Self {
            handle,
            operation,
            workspace,
            data_root,
            completion,
            authority: EditorAuthority::new(owner, context),
            image_input: None,
            virtual_document: None,
            diagnostic_collection: None,
            clipboard_offer: None,
            transfer_offer: None,
            decoration: None,
            decoration_icons: Default::default(),
            comparison_icons: Default::default(),
            parents,
            selection: Default::default(),
            progress: Default::default(),
            workspace_request: None,
            language_control: None,
            language_refresh: None,
        }
    }
    pub fn handle(&self) -> &ResourceHandle {
        &self.handle
    }
    pub fn operation(&self) -> &EditorOperation {
        &self.operation
    }
    /// Attach only runtime-issued metadata after instance ownership and capability checks.
    pub(crate) fn with_clipboard_offer(
        mut self,
        offer: Arc<crate::ClipboardOfferResource>,
    ) -> Self {
        self.clipboard_offer = Some(offer);
        self
    }
    /// Native adapters may enumerate/select this sealed offer; guest JSON cannot provide its authority.
    pub fn clipboard_offer(&self) -> Option<&Arc<crate::ClipboardOfferResource>> {
        self.clipboard_offer.as_ref()
    }
    /// The instance attaches only an owned, actually dropped offer after its normal capability gate.
    pub(crate) fn with_transfer_offer(mut self, offer: Arc<crate::TransferOfferResource>) -> Self {
        self.transfer_offer = Some(offer);
        self
    }
    /// Native IO borrows the exact resource checked at admission; JSON cannot redirect its source.
    pub fn transfer_offer(&self) -> Option<&Arc<crate::TransferOfferResource>> {
        self.transfer_offer.as_ref()
    }
    /// Borrow the unforgeable lifetime authority for an admitted persistent native view.
    /// Hosts may clone it to observe revocation, but cannot mutate its runtime-owned lifetimes.
    pub fn authority(&self) -> &EditorAuthority {
        &self.authority
    }
    /// Borrow the control metadata after runtime ownership, schema and permission admission.
    pub fn language_control_metadata(&self) -> Option<&LanguageControlMetadata> {
        self.language_control.as_ref()
    }
    pub(crate) fn with_language_refresh(
        mut self,
        contexts: crate::LanguageContexts,
        context: crate::language_contexts::RegisteredContext,
    ) -> Self {
        self.authority = self.authority.with_guard(context.guard());
        self.completion.lifetimes.push(context.guard());
        let expected = match &self.operation {
            EditorOperation::RefreshLanguageContext { document, .. } => {
                contexts.refresh_guard(&context, &document.path)
            }
            _ => None,
        };
        self.language_refresh = Some((contexts, context, expected, Default::default()));
        self
    }

    /// Commit a runtime-issued context selection only after the native document revision is checked.
    /// Raw `EditorOperation` JSON has no proof and cannot select or refresh a provider by name.
    pub fn commit_language_refresh(&self) -> Result<(), Failure> {
        let EditorOperation::RefreshLanguageContext { document, .. } = &self.operation else {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Request is not a context refresh",
            ));
        };
        let (registry, context, expected, committed) =
            self.language_refresh.as_ref().ok_or_else(|| {
                Failure::new(
                    ErrorCode::PermissionDenied,
                    "Context refresh has no registered authority",
                )
            })?;
        if !self.authority.is_live() {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Context refresh retired",
            ));
        }
        let guard = registry.select_registered(context, &document.path, expected.as_ref())?;
        // Use the post-commit choice for delivery. The prior CAS guard was deliberately revoked
        // by this successful commit and cannot be reused as a success-notification condition.
        *committed.lock().unwrap() = Some(guard);
        Ok(())
    }
    pub(crate) fn with_language_control(mut self, metadata: LanguageControlMetadata) -> Self {
        if let Some(guard) = &metadata.selection_guard {
            self.completion.lifetimes.push(guard.clone());
        }
        if let Some(context) = &metadata.selected_context {
            self.authority = self.authority.with_guard(context.guard());
            self.completion.lifetimes.push(context.guard());
        }
        for guard in &metadata.collection_guards {
            self.authority = self.authority.with_guard(guard.clone());
            self.completion.lifetimes.push(guard.clone());
        }
        self.language_control = Some(metadata);
        self
    }
    /// Native publishers receive only the admitted collection, never a guest-created source string.
    pub fn diagnostic_collection(&self) -> Option<&std::sync::Arc<crate::DiagnosticCollection>> {
        self.diagnostic_collection.as_ref()
    }
    pub(crate) fn with_diagnostic_collection(
        mut self,
        collection: std::sync::Arc<crate::DiagnosticCollection>,
    ) -> Self {
        self.diagnostic_collection = Some(collection);
        self
    }
    /// Borrow checked IO authority; absence means this request cannot execute a workspace operation.
    pub fn workspace_request(&self) -> Option<&crate::WorkspaceRequest> {
        self.workspace_request.as_ref()
    }
    pub(crate) fn with_workspace_request(mut self, request: crate::WorkspaceRequest) -> Self {
        self.workspace_request = Some(request);
        self
    }
    /// Analysis conditions govern pending admission, while successful native effects retain the
    /// original owner/caller authority. The request and its receipt share the same monotonic phase.
    pub(crate) fn with_preparation(mut self, conditions: Arc<PreparationConditions>) -> Self {
        self.completion.bind_condition(move || conditions.is_live());
        self
    }
    /// Native virtual opens require runtime-issued owned authority.
    pub fn virtual_document(&self) -> Option<&std::sync::Arc<crate::VirtualDocumentResource>> {
        self.virtual_document.as_ref()
    }
    /// Native marker admission cannot fabricate a collection owner or read ambient icon paths.
    pub fn decoration(&self) -> Option<&Arc<crate::DecorationResource>> {
        self.decoration.as_ref()
    }
    pub fn decoration_icons(&self) -> &std::collections::BTreeMap<String, Arc<[u8]>> {
        &self.decoration_icons
    }
    /// Borrow the bounded SVG bytes checked for this request's publishing instance.
    pub fn comparison_icons(&self) -> &std::collections::BTreeMap<String, Arc<[u8]>> {
        &self.comparison_icons
    }
    /// Seal admitted comparison artwork before the request crosses into the native UI queue.
    pub(crate) fn with_comparison_icons(
        mut self,
        icons: std::collections::BTreeMap<String, Arc<[u8]>>,
    ) -> Self {
        self.comparison_icons = icons;
        self
    }
    pub(crate) fn with_decorations(
        mut self,
        resource: Arc<crate::DecorationResource>,
        icons: std::collections::BTreeMap<String, Arc<[u8]>>,
    ) -> Self {
        self.decoration = Some(resource);
        self.decoration_icons = icons;
        self
    }
    /// Attach after the instance admits ownership, permissions and quota.
    pub(crate) fn with_virtual_document(
        mut self,
        resource: std::sync::Arc<crate::VirtualDocumentResource>,
    ) -> Self {
        self.virtual_document = Some(resource);
        self
    }
    /// The native writer borrows immutable authorized pixels and the original document/selection binding.
    pub fn image_input(&self) -> Option<&crate::ImageInputResource> {
        self.image_input.as_deref()
    }
    /// Attach only after the instance has checked handle ownership, permissions and name shape.
    pub(crate) fn with_image_input(
        mut self,
        input: std::sync::Arc<crate::ImageInputResource>,
    ) -> Self {
        self.image_input = Some(input);
        self
    }
    pub fn workspace(&self) -> &str {
        &self.workspace
    }
    /// The UI revalidates containment at execution rather than trusting a guest-provided native path.
    pub fn data_root(&self) -> &std::path::Path {
        &self.data_root
    }
    pub fn begin(&self) -> bool {
        self.expire_parents();
        if self
            .diagnostic_collection
            .as_ref()
            .is_some_and(|collection| !collection.is_live())
        {
            self.completion.finish(Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Diagnostic collection was revoked",
            )));
            return false;
        }
        if self
            .decoration
            .as_ref()
            .is_some_and(|resource| !resource.is_live())
        {
            self.completion.finish(Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Decoration resource was revoked",
            )));
            return false;
        }
        if self
            .virtual_document
            .as_ref()
            .is_some_and(|resource| !resource.is_live())
        {
            self.completion.finish(Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Virtual resource was revoked",
            )));
            return false;
        }
        self.completion.begin()
    }
    pub fn finish(&self, result: Result<EditorValue, Failure>) {
        self.expire_parents();
        if let Some(metadata) = &self.language_control {
            if metadata.imports.iter().any(|lease| !lease.is_live()) {
                self.completion.finish(Err(Failure::new(
                    ErrorCode::InvalidHandle,
                    "Native module was revoked",
                )));
                return;
            }
            if let Ok(value) = &result {
                let validation = match value {
                    EditorValue::Language(value) | EditorValue::LanguageBinding { value, .. }
                        if serde_json::to_vec(value).is_ok_and(|bytes| {
                            bytes.len() <= plugin_protocol::language_control::MAX_CONTROL_BYTES
                        }) =>
                    {
                        metadata.response.accepts(value)
                    }
                    _ => Err(Failure::new(
                        ErrorCode::InvalidRequest,
                        "Invalid language control result",
                    )),
                };
                if let Err(error) = validation {
                    self.completion.finish(Err(error));
                    return;
                }
            }
        }
        // A normal completion cannot fabricate selected handles or bypass the trusted picker seam.
        if matches!(
            self.operation,
            EditorOperation::Interaction {
                operation: plugin_protocol::interaction::Operation::Select { .. }
                    | plugin_protocol::interaction::Operation::SelectWithAccess { .. }
            }
        ) && result.is_ok()
        {
            self.completion.finish(Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Selection requires native paths",
            )));
            return;
        }
        let result = if let (
            EditorOperation::SaveImageInput { input, name },
            Some(resource),
            Ok(value),
        ) = (&self.operation, &self.image_input, &result)
        {
            if !matches!(value, EditorValue::ImageSaved { input: saved, document, name: saved_name }
                if saved == input && document == &resource.document && saved_name == name)
            {
                Err(Failure::new(
                    ErrorCode::InvalidRequest,
                    "Image save receipt does not match its owned request",
                ))
            } else {
                result
            }
        } else {
            result
        };
        self.completion.finish(result);
    }
    /// Retain the original consumer before dispatch. Exact service, workspace and document admission
    /// prevents an unrelated plan from receiving lifecycle metadata or native context identities.
    pub fn register_language_consumer(
        &self,
        service: &crate::LanguageService,
    ) -> Result<(), Failure> {
        let metadata = self.language_control.as_ref().ok_or_else(|| {
            Failure::new(ErrorCode::InvalidRequest, "Not a native control request")
        })?;
        let EditorOperation::LanguageRequest { request: control } = &self.operation else {
            unreachable!()
        };
        if self.status().is_terminal()
            || !service.admits_control(
                &self.authority,
                std::path::Path::new(&self.workspace),
                &control.document,
            )
            || metadata.provider != format!("{}/{}", service.owner, service.provider.id)
            || metadata
                .selected_context
                .as_ref()
                .is_some_and(|selected| !service.matches_registered_context(selected))
        {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Native control context changed",
            ));
        }
        service.retain_control_consumer(&metadata.consumer, &self.authority)
    }
    /// Seal a successful module attachment to the actual native plan used by this request.
    /// Callers must not call this with another plan; provider ownership, schema and live authority are
    /// verified here as well as at redemption. Failure and cancellation never grant a usable binding.
    pub fn finish_language_binding(
        &self,
        value: serde_json::Value,
        service: Arc<crate::LanguageService>,
    ) {
        let result = (|| {
            let metadata = self.language_control.as_ref().ok_or_else(|| {
                Failure::new(ErrorCode::InvalidRequest, "Not a language control request")
            })?;
            let expected = format!("{}/{}", service.owner, service.provider.id);
            let EditorOperation::LanguageRequest { request: control } = &self.operation else {
                unreachable!()
            };
            if expected != metadata.provider
                || metadata
                    .selected_context
                    .as_ref()
                    .is_some_and(|selected| !service.matches_registered_context(selected))
                || !self.authority.is_live()
                || self.status().is_terminal()
                || !service.admits_control(
                    &self.authority,
                    std::path::Path::new(&self.workspace),
                    &control.document,
                )
            {
                return Err(Failure::new(
                    ErrorCode::InvalidHandle,
                    "Language binding native owner changed",
                ));
            }
            metadata.response.accepts(&value)?;
            service.adopt_registered_context(&metadata.contexts, &metadata.owner, &metadata.scope);
            if serde_json::to_vec(&value).map_or(true, |bytes| {
                bytes.len() > plugin_protocol::language_control::MAX_CONTROL_BYTES
            }) {
                return Err(Failure::new(
                    ErrorCode::LimitExceeded,
                    "Language binding response exceeds quota",
                ));
            }
            let binding = metadata
                .bindings
                .issue(metadata, service, self.authority.clone())?;
            Ok(EditorValue::LanguageBinding { value, binding })
        })();
        self.finish(result);
    }
    /// Complete a native selection with user-chosen paths. No guest-supplied path is accepted here.
    /// Cancellation, timeout and retirement win over a late callback. The worker validates paths
    /// and allocates instance-owned handles before delivering the final result to the guest.
    pub fn finish_selection(&self, paths: Vec<std::path::PathBuf>) {
        self.expire_parents();
        if !matches!(
            self.operation,
            EditorOperation::Interaction {
                operation: plugin_protocol::interaction::Operation::Select { .. }
                    | plugin_protocol::interaction::Operation::SelectWithAccess { .. }
            }
        ) {
            self.completion.finish(Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Not a selection request",
            )));
            return;
        }
        let mut selection = self.selection.lock().unwrap();
        if selection.is_some() || self.completion.status().is_terminal() {
            return;
        }
        *selection = Some(paths);
        self.completion.finish(Ok(EditorValue::Unit));
    }
    /// Consume trusted picker data once, while the owning instance is delivering this completion.
    pub(crate) fn take_selection(&self) -> Option<Vec<std::path::PathBuf>> {
        self.selection.lock().unwrap().take()
    }
    pub fn status(&self) -> RequestUpdate {
        self.expire_parents();
        self.check_language_update(self.completion.status())
    }
    /// Native progress reads the latest bounded state without locking a WASM instance.
    pub fn progress(&self) -> (String, Option<u8>) {
        self.progress.lock().unwrap().clone()
    }
    /// Replace visible progress only while its original pending lifetime is still valid.
    pub(crate) fn update_progress(
        &self,
        message: String,
        percent: Option<u8>,
    ) -> Result<(), Failure> {
        if self.status().is_terminal()
            || !matches!(
                self.operation,
                EditorOperation::Interaction {
                    operation: plugin_protocol::interaction::Operation::Progress { .. }
                }
            )
        {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Progress task ended or has another kind",
            ));
        }
        *self.progress.lock().unwrap() = (message, percent);
        Ok(())
    }
    /// A native user dismissal seals this wait; it never claims to undo an entered side effect.
    pub fn cancel_from_host(&self, mode: CancelMode) {
        let _ = self.completion.cancel(mode, ErrorCode::Cancelled);
    }
    pub(crate) fn update(&self) -> (u64, RequestUpdate) {
        self.expire_parents();
        let (version, update) = self.completion.update();
        (version, self.check_language_update(update))
    }

    /// A terminal result can wait for guest delivery while its source is reselected. Project only
    /// that obsolete control success to an explicit error; completed external effects are not undone.
    pub(crate) fn check_language_update(&self, update: RequestUpdate) -> RequestUpdate {
        let control_stale = self
            .language_control
            .as_ref()
            .and_then(|metadata| metadata.selection_guard.as_ref())
            .is_some_and(|guard| !guard.load(Ordering::Acquire));
        let refresh_stale = self
            .language_refresh
            .as_ref()
            .is_some_and(|(_, _, _, committed)| {
                committed
                    .lock()
                    .unwrap()
                    .as_ref()
                    .is_some_and(|guard| !guard.load(Ordering::Acquire))
            });
        if matches!(&update, RequestUpdate::Completed { result: Ok(_) })
            && (control_stale || refresh_stale)
        {
            RequestUpdate::Completed {
                result: Err(Failure::new(
                    ErrorCode::InvalidHandle,
                    "Source context changed before result delivery",
                )),
            }
        } else {
            update
        }
    }
    pub(crate) fn retire(&self) {
        self.completion.retire();
    }
    pub fn enter_side_effect(&self) -> bool {
        self.expire_parents();
        self.completion.enter_side_effect()
    }
    pub fn expire(&self, now: Instant) {
        self.expire_parents();
        self.completion.expire(now);
    }
    /// Observe parent deadlines before checking their cancellation token on a native thread.
    fn expire_parents(&self) {
        for parent in &self.parents {
            parent.status();
        }
    }
    pub(crate) fn cancel(
        &self,
        mode: CancelMode,
        reason: ErrorCode,
    ) -> Result<CancellationEffect, Failure> {
        self.completion.cancel(mode, reason)
    }
}
pub(crate) struct PendingRequest {
    pub context: Option<crate::plugin_services::Context>,
    pub call: EditorRequest,
    pub sent: bool,
    pub reported: u64,
}
