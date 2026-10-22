//! Canonical debugger shapes shared by independent SDK providers and the host consumer.

/// Exact debug.session 1.1 signatures; optional methods are matched individually.
/// These messages do not choose a transport, language, or installed provider identity.
pub fn declaration() -> crate::service::Contract {
    serde_json::from_str(include_str!("debug-session.json"))
        .expect("the published debug.session contract is valid")
}

/// debug.session 1.2 adds optional explicit-thread frames and lazy, bounded variable pages.
/// Base 1.1 method shapes remain byte-for-byte equivalent; older providers still serve those calls.
/// References are valid only for their exact session, pause, thread and frame. Zero requests roots;
/// nonzero references must have been returned within that scope. Resuming revokes all references.
pub fn declaration_with_inspection() -> crate::service::Contract {
    let mut declaration = declaration();
    let extra: crate::service::Contract =
        serde_json::from_str(include_str!("debug-inspection.json"))
            .expect("the published debug inspection contract is valid");
    declaration.version = extra.version;
    declaration.methods.extend(extra.methods);
    declaration
}

/// A real stopped thread's opaque provider identity and display name.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Thread {
    pub id: u32,
    pub name: String,
}

/// One bounded thread report; selected is the actual stopped thread, zero when none is reported.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Threads {
    pub threads: Vec<Thread>,
    pub selected: u32,
}

/// One inert variable node. Expanding it requests children; it never evaluates an expression.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variable {
    pub name: String,
    pub value: String,
    /// Zero means leaf. The provider guards nonzero references with all four inspection identities.
    pub reference: u32,
    /// Known named/indexed children; zero may also mean that the adapter has not supplied a count.
    pub named: u32,
    pub indexed: u32,
}

/// At most 100 nodes per page; more asks the caller to explicitly request the next page.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variables {
    pub variables: Vec<Variable>,
    pub more: bool,
}
