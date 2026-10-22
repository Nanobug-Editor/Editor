//! Structured negotiation shares the exact implementation used by the legacy Rust string result.
use super::*;

/// Stable Rust-level negotiation categories; these do not add guest wire error variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NegotiationKind {
    /// The base protocol range excludes this host.
    Base,
    /// More than 128 combined declarations were supplied.
    Limit,
    /// A capability identity is empty or oversized.
    Identity,
    /// One capability was declared both required and optional.
    Duplicate,
    /// A required capability or its requested version is unavailable.
    Required,
}
/// Typed failure keeps machine decisions independent from localized diagnostic text.
#[derive(Clone, Debug)]
pub struct NegotiationFailure {
    /// Why admission failed before activation.
    pub kind: NegotiationKind,
    /// Original declaration identity when known; report exporters must validate it as metadata.
    pub capability: Option<String>,
    /// Required SemVer range, when a version mismatch caused failure.
    pub required: Option<VersionReq>,
    /// Available host version, if the capability exists.
    pub available: Option<Version>,
}
impl std::fmt::Display for NegotiationFailure {
    /// Preserve existing string diagnostics while exposing a separate stable typed cause.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Fields are public for inspection/construction; Display must also tolerate missing evidence.
        let required = self
            .required
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "unspecified".into());
        let available = self
            .available
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "unavailable".into());
        match self.kind {
            NegotiationKind::Base => write!(
                f,
                "Unsupported base API: requires {required}, host {available}"
            ),
            NegotiationKind::Limit => f.write_str("Too many capability requirements"),
            NegotiationKind::Identity => f.write_str("Invalid capability identity"),
            NegotiationKind::Duplicate => write!(
                f,
                "Capability is both required and optional: {}",
                self.capability.as_deref().unwrap_or_default()
            ),
            NegotiationKind::Required => write!(
                f,
                "Required capability unavailable: {} {required}",
                self.capability.as_deref().unwrap_or_default()
            ),
        }
    }
}
impl std::error::Error for NegotiationFailure {}
impl Requirements {
    /// Negotiate `base` and `available` through the same rules as `negotiate`, retaining typed causes.
    /// Optional absence is an intentional omission, never an admission failure or permission grant.
    pub fn negotiate_detailed(
        &self,
        base: Version,
        available: &BTreeMap<String, Version>,
    ) -> Result<Negotiated, NegotiationFailure> {
        let failure = |kind, capability, required, available| NegotiationFailure {
            kind,
            capability,
            required,
            available,
        };
        if !self.base.matches(&base) {
            return Err(failure(
                NegotiationKind::Base,
                None,
                Some(self.base.clone()),
                Some(base),
            ));
        }
        if self.required.len() + self.optional.len() > 128 {
            return Err(failure(NegotiationKind::Limit, None, None, None));
        }
        let mut capabilities = BTreeMap::new();
        for (id, range) in self.required.iter().chain(&self.optional) {
            if id.is_empty() || id.len() > 128 {
                return Err(failure(NegotiationKind::Identity, None, None, None));
            }
            if self.required.contains_key(id) && self.optional.contains_key(id) {
                return Err(failure(
                    NegotiationKind::Duplicate,
                    Some(id.clone()),
                    None,
                    None,
                ));
            }
            if let Some(version) = available.get(id).filter(|version| range.matches(version)) {
                capabilities.insert(id.clone(), version.clone());
            } else if self.required.contains_key(id) {
                return Err(failure(
                    NegotiationKind::Required,
                    Some(id.clone()),
                    Some(range.clone()),
                    available.get(id).cloned(),
                ));
            }
        }
        Ok(Negotiated { base, capabilities })
    }
}
