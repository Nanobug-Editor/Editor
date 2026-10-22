//! A scripted WIT host injects failures at the actual external boundary of the unmodified Git WASM.
use super::*;
use std::collections::BTreeMap;
use wasmtime::{
    Engine, Store,
    component::{Component, HasSelf, Linker},
};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxView, WasiView};

wasmtime::component::bindgen!({path:"../plugin-protocol/wit",world:"plugin",require_store_data_send:true});

/// This WIT boundary owns accepted resources and injects failures without mocking ResourceGroup.
struct Host {
    /// No preopened files, inherited environment or network authority is supplied.
    wasi: WasiCtx,
    /// WASI resources are separate from the explicit protocol-owned timer map.
    table: ResourceTable,
    /// Accepted resource identities observable independently of guest-local fields.
    timers: BTreeMap<u64, api::ResourceHandle>,
    /// Accepted document streams stay independent of timer cleanup and terminal receipts.
    subscriptions: BTreeMap<u64, api::ResourceHandle>,
    /// A successful pair permits testing a later initialization failure with two owned resources.
    accept_documents: bool,
    /// Allocation sequence exposes attempts to replace an incompletely released owner.
    next: u64,
    /// A finite number of host-side release errors, decremented by explicit cleanup attempts.
    close_failures: usize,
}
impl WasiView for Host {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}
impl editor::plugin::host::Host for Host {
    /// Decode the public request and preserve its correlation ID in both successful and failed replies.
    fn request(&mut self, payload: String) -> Result<String, String> {
        let request: api::Request = serde_json::from_str(&payload).unwrap();
        let result = match request.operation {
            api::Operation::Timer {
                operation:
                    plugin_runtime::plugin_protocol::timers::Operation::Start { delay_ms, repeat },
            } => {
                assert_eq!((delay_ms, repeat), (30_000, true));
                self.next += 1;
                let handle = api::ResourceHandle {
                    instance: "scripted-git".into(),
                    scope: "private-workspace".into(),
                    resource: self.next,
                };
                self.timers.insert(self.next, handle.clone());
                Ok(api::Value::Resource(handle))
            }
            api::Operation::SubscribeDocumentEvents if self.accept_documents => {
                self.next += 1;
                let handle = api::ResourceHandle {
                    instance: "scripted-git".into(),
                    scope: "private-workspace".into(),
                    resource: self.next,
                };
                self.subscriptions.insert(self.next, handle.clone());
                Ok(api::Value::Resource(handle))
            }
            api::Operation::SubscribeDocumentEvents => Err(api::Failure::new(
                api::ErrorCode::LimitExceeded,
                "primary-subscription-failure",
            )),
            api::Operation::CloseResource { handle } => {
                assert_eq!(
                    self.timers
                        .get(&handle.resource)
                        .or_else(|| self.subscriptions.get(&handle.resource)),
                    Some(&handle)
                );
                if self.close_failures > 0 {
                    self.close_failures -= 1;
                    Err(api::Failure::new(
                        api::ErrorCode::OperationFailed,
                        "release temporarily unavailable",
                    ))
                } else {
                    self.timers.remove(&handle.resource);
                    self.subscriptions.remove(&handle.resource);
                    Ok(api::Value::Unit)
                }
            }
            api::Operation::OpenData => Err(api::Failure::new(
                api::ErrorCode::PermissionDenied,
                "primary-data-initialization-failure",
            )),
            _ => panic!("unexpected external operation in failed initialization"),
        };
        Ok(serde_json::to_string(&api::Response {
            id: request.id,
            result,
        })
        .unwrap())
    }
}

/// Run the exact independently packaged component with a private WASI context and injected WIT transport.
fn instance(close_failures: usize, accept_documents: bool) -> (Store<Host>, Plugin) {
    let package = candidate();
    let engine = Engine::default();
    let component = Component::new(
        &engine,
        &package.files[package.manifest.component.as_ref().unwrap()],
    )
    .unwrap();
    let mut linker = Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker).unwrap();
    Plugin::add_to_linker::<Host, HasSelf<Host>>(&mut linker, |state| state).unwrap();
    let mut store = Store::new(
        &engine,
        Host {
            wasi: WasiCtx::builder().build(),
            table: ResourceTable::new(),
            timers: BTreeMap::new(),
            subscriptions: BTreeMap::new(),
            accept_documents,
            next: 0,
            close_failures,
        },
    );
    let plugin = Plugin::instantiate(&mut store, &component, &linker).unwrap();
    (store, plugin)
}

/// Serialized dispatch exposes the same completion/failure contract as the real runtime transport.
fn dispatch(
    store: &mut Store<Host>,
    plugin: &Plugin,
    message: api::Input,
) -> Result<api::Output, api::Failure> {
    let input = serde_json::to_string(&api::Invocation { id: 1, message }).unwrap();
    let output = plugin.call_dispatch(&mut *store, &input).unwrap().unwrap();
    serde_json::from_str::<api::Completion>(&output)
        .unwrap()
        .result
}

/// Timer admission precedes subscription rejection; the earlier actual host resource must be released.
#[test]
#[ignore = "build independent Git 0.1.24; set NANOBUG_GIT_CANDIDATE; injected external WIT failure"]
fn git_sdk_subscription_failure_releases_the_accepted_timer() {
    let (mut store, plugin) = instance(0, false);
    let failure = dispatch(&mut store, &plugin, api::Input::Activate).unwrap_err();
    assert_eq!(failure.code, api::ErrorCode::LimitExceeded);
    assert!(failure.message.contains("primary-subscription-failure"));
    assert_eq!(
        store.data().next,
        1,
        "the timer was actually admitted before subscription failure"
    );
    assert!(
        store.data().timers.is_empty(),
        "accepted host timer must be released by the guest"
    );
}

/// Failed reset cannot discard the owner or start another timer; a later explicit attempt can recover.
#[test]
#[ignore = "build independent Git 0.1.24; set NANOBUG_GIT_CANDIDATE; finite injected release failures"]
fn git_sdk_partial_cleanup_retains_owner_across_reset_and_bounded_retries() {
    let (mut store, plugin) = instance(3, false);
    let failure = dispatch(&mut store, &plugin, api::Input::Activate).unwrap_err();
    assert_eq!(failure.code, api::ErrorCode::LimitExceeded);
    assert!(failure.message.contains("primary-subscription-failure"));
    assert!(failure.message.contains("Background cleanup incomplete"));
    assert!(store.data().timers.contains_key(&1));
    let prepare = api::Input::Prepare {
        environment: Environment::default(),
        snapshot: None,
        api: api::Negotiated {
            base: semver::Version::new(1, 0, 0),
            capabilities: Default::default(),
        },
    };
    let failure = dispatch(&mut store, &plugin, prepare).unwrap_err();
    assert_eq!(failure.code, api::ErrorCode::InvalidState);
    assert_eq!(
        dispatch(&mut store, &plugin, api::Input::Activate)
            .unwrap_err()
            .code,
        api::ErrorCode::InvalidState
    );
    assert_eq!(
        store.data().next,
        1,
        "failed owner blocks replacement allocation"
    );
    assert!(store.data().timers.contains_key(&1));
    assert_eq!(
        dispatch(&mut store, &plugin, api::Input::Activate)
            .unwrap_err()
            .code,
        api::ErrorCode::LimitExceeded
    );
    assert_eq!(
        store.data().next,
        2,
        "allocation resumes only after the original release succeeds"
    );
    assert!(store.data().timers.is_empty());
}

/// One failed subscription release must not prevent the timer's release or lose the failed owner.
#[test]
#[ignore = "build independent Git 0.1.24; set NANOBUG_GIT_CANDIDATE; actual two-resource cleanup"]
fn git_sdk_partial_pair_cleanup_continues_and_reset_retries_only_failed_owner() {
    let (mut store, plugin) = instance(1, true);
    let failure = dispatch(&mut store, &plugin, api::Input::Activate).unwrap_err();
    assert_eq!(failure.code, api::ErrorCode::PermissionDenied);
    assert!(
        failure
            .message
            .contains("primary-data-initialization-failure")
    );
    assert!(failure.message.contains("Background cleanup incomplete"));
    assert!(
        store.data().timers.is_empty(),
        "one release failure cannot block another resource"
    );
    assert!(store.data().subscriptions.contains_key(&2));
    assert_eq!(store.data().next, 2);
    let prepare = || api::Input::Prepare {
        environment: Environment::default(),
        snapshot: None,
        api: api::Negotiated {
            base: semver::Version::new(1, 0, 0),
            capabilities: Default::default(),
        },
    };
    store.data_mut().close_failures = 1;
    assert_eq!(
        dispatch(&mut store, &plugin, prepare()).unwrap_err().code,
        api::ErrorCode::InvalidState
    );
    assert!(store.data().subscriptions.contains_key(&2));
    assert_eq!(store.data().next, 2, "reset cannot replace a pending owner");
    dispatch(&mut store, &plugin, prepare()).unwrap();
    assert!(store.data().timers.is_empty() && store.data().subscriptions.is_empty());
    dispatch(&mut store, &plugin, prepare()).unwrap();
    assert_eq!(
        store.data().next,
        2,
        "repeated reset must not allocate any replacement resource"
    );
}
