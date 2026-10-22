//! Portable target bindings and provider-owned preparation over versioned plugin services.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
/// Discovery never runs a target; bindings are portable provider data, not executable host commands.
pub const CONTRACT: &str = "run.targets";
/// Exact 1.0 signatures shared by an independent provider and the host consumer.
pub fn declaration() -> crate::service::Contract {
    serde_json::from_str(include_str!("run-targets.json")).expect("published run.targets contract")
}

/// Canonical run.targets 1.1 preserves literal provider prefixes before the configuration's argv.
/// Program-only 1.0 remains independently selectable; a 1.0 consumer never discards a newer prefix.
pub fn declaration_with_arguments() -> crate::service::Contract {
    let mut contract = declaration();
    contract.version = semver::Version::new(1, 1, 0);
    let crate::service::Schema::Record { fields, optional } =
        &mut contract.methods.get_mut("prepare").unwrap().result
    else {
        unreachable!()
    };
    fields.insert(
        "prefix_args".into(),
        crate::service::Schema::Array {
            items: Box::new(crate::service::Schema::String { max_bytes: 4096 }),
            max_items: 128,
        },
    );
    *optional = BTreeSet::from(["prefix_args".into()]);
    contract
}

/// One bounded native program and its literal argument prefix. User arguments append in order.
/// Absence/empty prefix retains 1.0 behavior; combined launch bounds are checked again by execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prepared {
    pub program: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prefix_args: Vec<String>,
}
impl Prepared {
    /// Reject malformed program/argv receipts before they can enter a native launch sequence.
    pub fn validate(&self) -> Result<(), String> {
        if self.program.trim().is_empty()
            || self.program.len() > 4096
            || self.program.contains('\0')
            || self.prefix_args.len() > 128
            || self
                .prefix_args
                .iter()
                .any(|value| value.len() > 4096 || value.contains('\0'))
            || self.prefix_args.iter().map(String::len).sum::<usize>() > 65536
        {
            return Err("Invalid prepared target program or argument prefix".into());
        }
        Ok(())
    }
}
impl From<String> for Prepared {
    /// Program-only receipts have no provider arguments, matching the published 1.0 contract.
    fn from(program: String) -> Self {
        Self {
            program,
            prefix_args: Vec::new(),
        }
    }
}
impl From<&str> for Prepared {
    /// A borrowed program name has the same argument-free semantics as a 1.0 receipt.
    fn from(program: &str) -> Self {
        program.to_owned().into()
    }
}
/// One provider candidate. The host namespaces identity with the authenticated provider incarnation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub identity: String,
    pub label: String,
    pub source: String,
    pub target_type: String,
    pub type_version: u32,
    /// Versioned JSON containing only portable fields; no credentials, grants or machine paths.
    pub binding: String,
}
