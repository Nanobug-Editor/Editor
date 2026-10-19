//! A bounded broker routes owned values; it never holds a WASM store or calls a guest under its lock.
use crate::request_state::Completion;
use plugin_protocol::{
    api::{ErrorCode, Failure, ResourceHandle},
    service::{Caller, Choice, Contract, Dependency, Method},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};
mod availability;
mod registry;

pub(crate) type Shared = Arc<Mutex<Broker>>;

#[derive(Clone)]
pub(crate) struct Provider {
    /// Static guest intent; host participants use their sealed incarnation without a guest demand.
    pub availability: Option<crate::Availability>,
    pub alive: Arc<AtomicBool>,
    /// The exact running guest's lifetime is additional to its logical metadata lifetime.
    pub owner_alive: Option<Arc<AtomicBool>>,
    pub caller: Caller,
    pub contracts: BTreeMap<String, Contract>,
}
#[derive(Clone)]
pub(crate) struct Reference {
    pub provider: Provider,
    pub contract: String,
    pub dependency: Dependency,
    revision: u64,
    /// A live session keeps its provider after the user's choice changes for future launches.
    pinned: bool,
    /// Clones of one open share first birth; a retired binding is never reset to an unbound reference.
    binding: Arc<OnceLock<BoundOwner>>,
}
/// One immutable first owner makes references and pending native sessions observe the same birth.
struct BoundOwner {
    instance: String,
    alive: Arc<AtomicBool>,
}
/// A selection revision never rolls back when a provider is removed and later selected again.
#[derive(Default)]
struct Selection {
    providers: Vec<String>,
    preference: Option<String>,
    revision: u64,
}
/// Only the Manager's typed command route creates a host origin; guest caller strings are descriptive.
#[derive(Clone)]
pub(crate) enum InvocationOrigin {
    Delegated,
    HostCommand { target: String },
}

/// The original source and shrinking authority survive nested calls; ancestry bounds asynchronous cycles.
#[derive(Clone)]
pub(crate) struct Context {
    pub lifetimes: Vec<Arc<AtomicBool>>,
    /// Pending native interactions observe source waits without revoking already owned programs.
    pub native_waits: Vec<Completion<Value>>,
    /// A native menu target is descriptive context, separate from schema-checked arguments.
    pub menu: Option<plugin_protocol::commands::Context>,
    /// An internal admission record is never serialized to, or accepted from, a guest.
    pub origin: InvocationOrigin,
    pub caller: Caller,
    pub ancestry: Vec<String>,
    pub permissions: BTreeSet<String>,
}
impl Context {
    /// A native command acts on its exact provider incarnation; nested calls cannot inherit this identity.
    pub(crate) fn direct_host_command(&self, instance: &str) -> bool {
        matches!(&self.origin, InvocationOrigin::HostCommand { target } if target == instance)
            && self.ancestry.len() == 1
            && self.ancestry[0] == instance
            && self
                .lifetimes
                .iter()
                .all(|alive| alive.load(Ordering::Acquire))
            && self.native_waits.iter().all(|wait| {
                !matches!(
                    wait.status(),
                    plugin_protocol::api::RequestUpdate::Cancelled { .. }
                )
            })
    }

    /// Picker access additionally requires the provider's own grant, never a delegated sibling's.
    pub(crate) fn direct_host_selection(&self, instance: &str) -> bool {
        self.direct_host_command(instance) && self.permissions.contains("files.select")
    }

    /// Each hop keeps the original owner, shrinks authority and adds the target's revocation boundary.
    pub(crate) fn delegate(
        &self,
        provider: &Provider,
        signature: &Method,
    ) -> Result<Self, Failure> {
        if self.ancestry.len() >= 8
            || self.ancestry.contains(&provider.caller.instance)
            || self.ancestry.contains(&provider.identity())
            || (provider.availability.is_some()
                && self.caller.plugin == provider.caller.plugin
                && self.caller.scope == provider.caller.scope)
        {
            return Err(Failure::new(
                ErrorCode::Conflict,
                "Service call cycle or depth limit detected",
            ));
        }
        if !signature.permissions.is_subset(&self.permissions)
            || !signature
                .permissions
                .is_subset(&provider.caller.permissions)
        {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Caller and provider must both authorize the service method",
            ));
        }
        let mut next = self.clone();
        next.permissions = signature.permissions.clone();
        next.ancestry.push(provider.caller.instance.clone());
        next.lifetimes.push(provider.alive.clone());
        // Logical availability survives HotApply. An already running participant also seals
        // continuations with its exact owner, so a trap cannot lend authority to a late callback.
        if let Some(alive) = &provider.owner_alive {
            next.lifetimes.push(alive.clone());
        }
        Ok(next)
    }
}
#[derive(Clone)]
pub(crate) struct Call {
    pub handle: ResourceHandle,
    pub reference: Reference,
    /// First enqueue seals this invocation's configuration, independently of a reusable open reference.
    /// Shared clones and a demand requeue cannot replace it with a later hot configuration.
    pub availability: Arc<OnceLock<Option<crate::Availability>>>,
    pub method: String,
    pub signature: Method,
    pub arguments: Value,
    pub context: Context,
    pub completion: Completion<Value>,
}
pub(crate) struct Pending {
    pub return_context: Option<Context>,
    pub call: Call,
    pub reported: u64,
}

/// Activation waits retain literal calls and their original deadline; no guest callback runs under this lock.
pub(crate) struct Waiting {
    pub call: Call,
    pub token: crate::Availability,
    pub demand: crate::DemandRequest,
}

/// Only host UI writes choices; project files and plugin requests have no setter.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Preferences {
    pub user: BTreeMap<String, String>,
    pub projects: BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Default)]
pub(crate) struct Broker {
    providers: BTreeMap<String, Provider>,
    queue: VecDeque<Call>,
    /// These slots also count toward the existing 256-call queue and Manager's shared 64 activation waits.
    waiting: VecDeque<Waiting>,
    /// Deferred callbacks are observed by the manager on its next tick, outside the guest's lock.
    completed: VecDeque<(Call, Result<Value, Failure>)>,
    pub preferences: Preferences,
    selections: BTreeMap<(String, String), Selection>,
    /// Host participants supply deterministic defaults; persisted user/project choices take precedence.
    host_defaults: BTreeMap<(String, String), String>,
}
impl Broker {
    /// Discovery exposes static typed command metadata in the caller's original logical scope.
    pub(crate) fn commands(&self, caller: &Caller) -> Vec<plugin_protocol::commands::Descriptor> {
        self.providers
            .values()
            .filter(|provider| {
                provider.alive.load(Ordering::Acquire) && provider.caller.scope == caller.scope
            })
            .flat_map(|provider| {
                provider.contracts.iter().filter_map(|(id, contract)| {
                    let command = id
                        .strip_prefix(plugin_protocol::commands::CONTRACT_PREFIX)?
                        .split_once('/')?
                        .1;
                    Some(plugin_protocol::commands::Descriptor {
                        plugin: provider.caller.plugin.clone(),
                        command: command.into(),
                        signature: contract.methods.get("invoke")?.clone(),
                    })
                })
            })
            .collect()
    }

    /// An explicitly named command retains its logical publication and one first birth, without preferences.
    pub(crate) fn command_reference(
        &self,
        caller: &Caller,
        plugin: &str,
        command: &str,
    ) -> Result<Reference, Failure> {
        let id = plugin_protocol::commands::contract(plugin, command);
        let provider = self
            .providers
            .values()
            .find(|provider| {
                provider.caller.plugin == plugin
                    && provider.caller.scope == caller.scope
                    && provider.alive.load(Ordering::Acquire)
                    && provider.contracts.contains_key(&id)
            })
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::CapabilityUnavailable,
                    "Typed command is unavailable in this caller scope",
                )
            })?;
        let dependency = Dependency {
            version: semver::VersionReq::parse("^1").unwrap(),
            optional: false,
            methods: provider.contracts[&id].methods.clone(),
        };
        Ok(Reference::new(provider.clone(), id, dependency, 0, true))
    }
    /// Reply admission validates unchanged source authority, original shape and the 64 KiB bound.
    pub(crate) fn complete_deferred(
        &mut self,
        call: Call,
        result: Result<Value, Failure>,
    ) -> Result<(), Failure> {
        if !self.context_alive(&call.context) || call.completion.status().is_terminal() {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Invocation source or deadline ended",
            ));
        }
        if self.completed.len() >= 128
            || serde_json::to_vec(&result).map_or(true, |bytes| bytes.len() > 65536)
        {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Deferred reply quota exceeded",
            ));
        }
        if let Ok(value) = &result {
            call.signature.result.accepts(value)?;
        }
        call.completion.finish(result.clone());
        if !matches!(
            call.completion.status(),
            plugin_protocol::api::RequestUpdate::Completed { .. }
        ) {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Invocation was cancelled before completion",
            ));
        }
        self.completed.push_back((call, result));
        Ok(())
    }
    /// Manager observations are delivered once, without holding a guest store under the broker lock.
    pub(crate) fn take_completed(&mut self) -> Vec<(Call, Result<Value, Failure>)> {
        self.completed.drain(..).collect()
    }
    /// Every delegation hop must still be the exact active incarnation that authorized the work.
    pub fn context_alive(&self, context: &Context) -> bool {
        context
            .lifetimes
            .iter()
            .all(|alive| alive.load(Ordering::Acquire))
            && context.native_waits.iter().all(|wait| {
                !matches!(
                    wait.status(),
                    plugin_protocol::api::RequestUpdate::Cancelled { .. }
                )
            })
    }
    pub fn new(preferences: Preferences) -> Self {
        Self {
            preferences,
            ..Default::default()
        }
    }
    pub fn enqueue(&mut self, call: Call) -> Result<(), Failure> {
        let provider = self.reference_provider(&call.reference)?;
        let _ = call.availability.set(provider.availability.clone());
        self.validate_call(&call)?;
        self.queue
            .retain(|call| !call.completion.update().1.is_terminal());
        if self.queue.len() + self.waiting.len() >= 256 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Service queue is full",
            ));
        }
        self.queue.push_back(call);
        Ok(())
    }
    /// A tick dispatches at most 32 callbacks; nested calls wait for a later tick instead of reentering stores.
    pub fn take_batch(&mut self) -> Vec<Call> {
        let count = self.queue.len().min(32);
        self.queue.drain(..count).collect()
    }
    /// Transfer bounded waits to the Manager; status/admission and new demands run outside this lock.
    pub(crate) fn take_waiting(&mut self) -> Vec<Waiting> {
        let count = self.waiting.len().min(32);
        self.waiting.drain(..count).collect()
    }
    /// A pending activation replaces the taken call's queue slot instead of opening an unbounded queue.
    pub(crate) fn wait(&mut self, waiting: Waiting) -> Result<(), Failure> {
        if self.queue.len() + self.waiting.len() >= 256 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Service queue is full",
            ));
        }
        self.waiting.push_back(waiting);
        Ok(())
    }
}

/// Application and workspace contracts may share a name without sharing provider preferences.
pub(crate) fn choice_key(scope: &str, contract: &str) -> String {
    format!(
        "{}/{contract}",
        if scope == "application" {
            "application"
        } else {
            "workspace"
        }
    )
}
