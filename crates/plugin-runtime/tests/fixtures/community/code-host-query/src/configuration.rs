//! Cloud policy interprets plugin-only settings without opening files, secrets or timers during validation.
use plugin_protocol::settings::{Effective, Proposal};

/// Only an own secret key reference is retained; credential bytes never enter settings or Snapshot.
pub struct Configuration {
    /// Milliseconds between bounded reads; Changed resets an existing timer without replacing its owner.
    pub poll_interval_ms: u32,
    /// Deadline in milliseconds for the next transfer; changing it cannot rewind an admitted HTTP request.
    pub timeout_ms: u32,
    /// An own, user-scoped secret key name; empty means no credential is configured.
    pub key_ref: String,
}
impl Default for Configuration {
    fn default() -> Self {
        Self {
            poll_interval_ms: 25,
            timeout_ms: 30_000,
            key_ref: "api-token".into(),
        }
    }
}
impl Configuration {
    /// Decode the complete effective map without changing a live resource.
    /// The host checks declared types; the guest also requires one poll to fit within its deadline.
    pub fn parse(values: &Effective) -> Result<Self, Proposal> {
        let mut result = Self::default();
        let mut proposal = Proposal::default();
        for (key, minimum, maximum, target) in [
            ("poll_interval", 10, 1000, &mut result.poll_interval_ms),
            ("timeout", 100, 120_000, &mut result.timeout_ms),
        ] {
            match values.get(key).and_then(|entry| entry.value.as_u64()) {
                Some(value) if (minimum..=maximum).contains(&value) => *target = value as u32,
                _ => {
                    proposal
                        .errors
                        .insert(key.into(), "Invalid cloud timing bound".into());
                }
            }
        }
        match values.get("keyRef").and_then(|entry| entry.value.as_str()) {
            Some(key) if key.len() <= 128 && !key.chars().any(char::is_control) => {
                result.key_ref = key.into();
            }
            _ => {
                proposal
                    .errors
                    .insert("keyRef".into(), "Invalid private secret reference".into());
            }
        }
        if proposal.errors.is_empty() && result.timeout_ms < result.poll_interval_ms {
            proposal.errors.insert(
                "timeout".into(),
                "Timeout must cover one poll interval".into(),
            );
        }
        if proposal.errors.is_empty() {
            Ok(result)
        } else {
            Err(proposal)
        }
    }
}
