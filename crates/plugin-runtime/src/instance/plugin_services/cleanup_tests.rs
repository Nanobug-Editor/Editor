//! Native commands must be able to retire their instance's ambient reads without borrowing siblings.
use super::*;

/// A native right-click starts under a new call context, while periodic processes predate that call.
/// Provider handles remain private to delegated sources, dead commands and other incarnations.
#[test]
fn direct_host_cleanup_retires_ambient_resources_without_delegating_provider_authority() {
    let mut roots = ResourceRoots::new("cleanup-fixture", false, 8192);
    let api::Value::Resource(handle) = roots.open(RootKind::Process(7)).unwrap() else {
        panic!("instance-owned process slot");
    };
    let mut services = Services::default();
    services.principal = roots.principal("provider", &Default::default());
    let mut context = CallContext {
        lifetimes: vec![std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
            true,
        ))],
        native_waits: vec![],
        menu: None,
        origin: broker::InvocationOrigin::HostCommand {
            target: services.principal.instance.clone(),
        },
        caller: services.principal.clone(),
        ancestry: vec![services.principal.instance.clone()],
        permissions: Default::default(),
    };
    assert!(
        services.permits_cleanup(&roots, &context, &handle),
        "a genuine host command must be able to cancel an earlier ambient observation"
    );
    context.origin = broker::InvocationOrigin::Delegated;
    assert!(!services.permits_cleanup(&roots, &context, &handle));
    context.origin = broker::InvocationOrigin::HostCommand {
        target: services.principal.instance.clone(),
    };
    context.ancestry.push("nested-hop".into());
    assert!(!services.permits_cleanup(&roots, &context, &handle));
    context.ancestry.pop();
    let mut other = ResourceRoots::new("cleanup-fixture", false, 8192);
    let api::Value::Resource(foreign) = other.open(RootKind::Process(8)).unwrap() else {
        panic!("foreign slot")
    };
    assert!(!services.permits_cleanup(&roots, &context, &foreign));
    context.lifetimes[0].store(false, std::sync::atomic::Ordering::Release);
    assert!(!services.permits_cleanup(&roots, &context, &handle));
    context.lifetimes[0].store(true, std::sync::atomic::Ordering::Release);
    // Existing delegated cleanup remains possible after revocation, but only for that caller's
    // recorded allocation. The ambient handle above deliberately had no such call owner.
    context.origin = broker::InvocationOrigin::Delegated;
    services
        .resources
        .insert(handle.resource, (handle.clone(), context.clone()));
    context.lifetimes[0].store(false, std::sync::atomic::Ordering::Release);
    assert!(services.permits_cleanup(&roots, &context, &handle));
    roots.remove(&handle);
    assert!(!services.permits_cleanup(&roots, &context, &handle));
}
