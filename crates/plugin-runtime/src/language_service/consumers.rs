//! Native consumer retirement derives from sealed call chains, never a guest-supplied identity.
use super::*;
use plugin_protocol::api::{ErrorCode, Failure};

#[derive(Default)]
pub(super) struct Consumers {
    live: BTreeMap<String, Vec<crate::EditorAuthority>>,
    pending: BTreeSet<String>,
}
impl Consumers {
    /// A caller may reach the engine through several independently live middlemen. Release only
    /// when the final authorized chain retires; one middleman's exit cannot revoke another chain.
    fn collect(&mut self) {
        self.live.retain(|consumer, chains| {
            chains.retain(crate::EditorAuthority::is_live);
            if chains.is_empty() { self.pending.insert(consumer.clone()); false } else { true }
        });
    }
}

impl LanguageService {
    /// Register an already admitted control's original consumer and exact lifetime chain.
    pub(crate) fn retain_control_consumer(&self, consumer: &str, authority: &crate::EditorAuthority) -> Result<(), Failure> {
        if !self.is_active() || !authority.is_live() {
            return Err(Failure::new(ErrorCode::InvalidHandle,"Native control consumer retired"));
        }
        let mut consumers = self.consumers.lock().unwrap();
        consumers.collect();
        if !consumers.live.contains_key(consumer) && consumers.live.len()+consumers.pending.len() >= 128 {
            return Err(Failure::new(ErrorCode::LimitExceeded,"Native control consumer quota exceeded"));
        }
        let chains = consumers.live.entry(consumer.into()).or_default();
        if !chains.iter().any(|old|old.same_owner(authority)) {
            if chains.len()>=64 {return Err(Failure::new(ErrorCode::LimitExceeded,"Native consumer chain quota exceeded"));}
            chains.push(authority.clone());
        }
        Ok(())
    }
    /// Snapshot a bounded idempotent retirement batch. Failed writes leave it pending so the next
    /// request cannot inherit dead callers' native references after a dropped notification.
    pub fn retired_control_consumers(&self) -> Vec<String> {
        let mut consumers = self.consumers.lock().unwrap();
        consumers.collect();
        consumers.pending.iter().cloned().collect()
    }
    /// Acknowledge after the owned native pipe accepted the complete notification. This is host
    /// transport bookkeeping, not a guest-callable operation or a transferable context grant.
    pub fn acknowledge_control_consumers(&self, consumers: &[String]) {
        let mut state = self.consumers.lock().unwrap();
        for consumer in consumers {state.pending.remove(consumer);}
    }
}
