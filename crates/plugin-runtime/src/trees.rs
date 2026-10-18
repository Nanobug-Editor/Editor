//! Bounded tree resources shared by the serial guest worker and native Base tree projection.
//! Only provider metadata is retained here; text, document selection and tree scrolling stay native.
use crate::EditorAuthority;
use plugin_protocol::{
    api::{ErrorCode, Failure, ResourceHandle},
    tree,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// One branch's publication and status. Existing pages remain visible during a continuation failure.
#[derive(Clone, Default)]
pub struct TreeBranch {
    pub children: Vec<String>,
    pub next: Option<String>,
    pub loaded: bool,
    pub loading: bool,
    pub error: Option<String>,
}
/// Immutable native projection, small enough to clone under the tree's fixed metadata quota.
#[derive(Clone, Default)]
pub struct TreeSnapshot {
    pub epoch: u64,
    pub revision: u64,
    pub nodes: BTreeMap<String, tree::Node>,
    pub branches: BTreeMap<Option<String>, TreeBranch>,
    pub expanded: BTreeSet<String>,
    pub icons: BTreeMap<String, Arc<[u8]>>,
}
struct Pending {
    request: tree::Request,
    deadline: Instant,
}
struct State {
    snapshot: TreeSnapshot,
    next: u64,
    pending: BTreeMap<u64, Pending>,
}
/// Runtime-issued lifetime authority prevents stale native entities from reviving retired providers.
pub struct TreeResource {
    handle: ResourceHandle,
    authority: EditorAuthority,
    revoked: AtomicBool,
    state: Mutex<State>,
}
impl TreeResource {
    /// Only the admitted instance allocates a tree; callers never supply a replacement owner.
    pub(crate) fn new(handle: ResourceHandle, authority: EditorAuthority) -> Self {
        Self {
            handle,
            authority,
            revoked: AtomicBool::new(false),
            state: Mutex::new(State {
                snapshot: TreeSnapshot {
                    epoch: 1,
                    revision: 1,
                    branches: [(None, TreeBranch::default())].into(),
                    ..Default::default()
                },
                next: 1,
                pending: BTreeMap::new(),
            }),
        }
    }
    pub fn handle(&self) -> &ResourceHandle {
        &self.handle
    }
    /// Task completion does not revoke a tree; owner or delegated caller retirement does.
    pub fn is_live(&self) -> bool {
        self.authority.is_live() && !self.revoked.load(Ordering::Acquire)
    }
    /// Close and owner cutover seal every retained native clone.
    pub fn revoke(&self) {
        self.revoked.store(true, Ordering::Release);
        self.suspend();
    }
    /// A retained readonly scene loses pending work without discarding its last visible metadata.
    pub fn suspend(&self) {
        let mut state = self.state.lock().unwrap();
        state.snapshot.epoch = state.snapshot.epoch.saturating_add(1);
        state.snapshot.revision = state.snapshot.revision.saturating_add(1);
        state.pending.clear();
        for branch in state.snapshot.branches.values_mut() {
            branch.loading = false;
        }
    }
    /// Expiration is observed by the bounded native refresh pulse and by subsequent operations.
    pub fn snapshot(&self) -> TreeSnapshot {
        let mut state = self.state.lock().unwrap();
        expire(&mut state);
        state.snapshot.clone()
    }
    /// Root loading, first expansion, continuation and retry all use this same bounded request.
    pub fn request(
        &self,
        parent: Option<String>,
        continuation: bool,
    ) -> Result<Option<tree::Request>, Failure> {
        self.live()?;
        let mut state = self.state.lock().unwrap();
        expire(&mut state);
        if let Some(parent) = &parent {
            if state
                .snapshot
                .nodes
                .get(parent)
                .is_none_or(|node| !node.branch || node.disabled)
            {
                return Err(stale());
            }
            if state.snapshot.expanded.insert(parent.clone()) {
                // Cached children still need a fresh native disclosure projection after re-expansion.
                state.snapshot.revision += 1;
            }
        }
        let branch = state.snapshot.branches.get(&parent).ok_or_else(stale)?;
        if branch.loading
            || (branch.loaded && branch.error.is_none() && !continuation)
            || (continuation && branch.next.is_none())
        {
            return Ok(None);
        }
        if state.pending.len() >= 8 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Tree pending-page quota exceeded",
            ));
        }
        let request = tree::Request {
            tree: self.handle.clone(),
            epoch: state.snapshot.epoch,
            sequence: state.next,
            parent: parent.clone(),
            cursor: if continuation {
                branch.next.clone()
            } else {
                None
            },
            max_nodes: tree::MAX_PAGE as u32,
        };
        state.next = state
            .next
            .checked_add(1)
            .ok_or_else(|| Failure::new(ErrorCode::LimitExceeded, "Tree request IDs exhausted"))?;
        let branch = state.snapshot.branches.get_mut(&parent).unwrap();
        branch.loading = true;
        branch.error = None;
        state.pending.insert(
            request.sequence,
            Pending {
                request: request.clone(),
                deadline: Instant::now() + Duration::from_secs(15),
            },
        );
        state.snapshot.revision += 1;
        Ok(Some(request))
    }
    /// A scene rejection never reached the provider. Release only its exact pending request so the
    /// current native owner can ask again. Keep cached pages, sibling requests, cursors and epoch.
    pub(crate) fn release_undelivered(&self, request: &tree::Request) {
        if request.tree != self.handle || !self.is_live() {
            return;
        }
        let mut state = self.state.lock().unwrap();
        if state
            .pending
            .get(&request.sequence)
            .is_none_or(|pending| pending.request != *request)
        {
            return;
        }
        state.pending.remove(&request.sequence);
        if let Some(branch) = state.snapshot.branches.get_mut(&request.parent) {
            branch.loading = false;
        }
        state.snapshot.revision += 1;
    }
    /// Native disclosure and keyboard collapse cancel every pending descendant before a late reply.
    pub fn collapse(&self, node: &str) {
        let mut state = self.state.lock().unwrap();
        let descendants = subtree(&state.snapshot, node);
        // Remove the entire disclosure path, otherwise reconciliation would restart hidden descendant work.
        state
            .snapshot
            .expanded
            .retain(|id| !descendants.contains(id));
        state.pending.retain(|_, pending| {
            !pending
                .request
                .parent
                .as_ref()
                .is_some_and(|id| descendants.contains(id))
        });
        for id in &descendants {
            if let Some(branch) = state.snapshot.branches.get_mut(&Some(id.clone())) {
                branch.loading = false;
            }
        }
        state.snapshot.revision += 1;
    }
    /// Refresh replaces provider pages atomically and seals old request identities.
    pub(crate) fn refresh(&self) -> Result<(), Failure> {
        self.live()?;
        let mut state = self.state.lock().unwrap();
        state.snapshot = TreeSnapshot {
            epoch: state.snapshot.epoch + 1,
            revision: state.snapshot.revision + 1,
            branches: [(None, TreeBranch::default())].into(),
            expanded: state.snapshot.expanded.clone(),
            ..Default::default()
        };
        state.pending.clear();
        Ok(())
    }
    /// Resolve complete typed arguments from the current owned node's exact activation or menu slot.
    /// Refresh, removed/disabled nodes and missing actions reject captured callbacks as stale.
    pub fn command(
        &self,
        epoch: u64,
        node: &str,
        action: &tree::NodeAction,
    ) -> Result<tree::Command, Failure> {
        self.live()?;
        let state = self.state.lock().unwrap();
        if state.snapshot.epoch != epoch {
            return Err(stale());
        }
        let node = state
            .snapshot
            .nodes
            .get(node)
            .filter(|node| !node.disabled)
            .ok_or_else(stale)?;
        // Menu ordinal identity remains immutable within an epoch; command names may legitimately repeat.
        match action {
            tree::NodeAction::Activation => node.activation.as_ref(),
            tree::NodeAction::Menu { index } => usize::try_from(*index)
                .ok()
                .and_then(|index| node.menu.get(index)),
        }
        .cloned()
        .ok_or_else(stale)
    }
    /// Requests are host issued, but must also still be pending when the worker accepts their UI event.
    pub(crate) fn validate_event(&self, event: &tree::Event) -> Result<(), Failure> {
        self.live()?;
        let mut state = self.state.lock().unwrap();
        expire(&mut state);
        match event {
            tree::Event::Request(request) => {
                if request.tree != self.handle
                    || state
                        .pending
                        .get(&request.sequence)
                        .is_none_or(|pending| pending.request != *request)
                {
                    return Err(stale());
                }
            }
            tree::Event::Selected { epoch, node, .. } => {
                if *epoch != state.snapshot.epoch
                    || state
                        .snapshot
                        .nodes
                        .get(node)
                        .is_none_or(|node| node.disabled)
                {
                    return Err(stale());
                }
            }
            tree::Event::Invoke { .. } => {}
        }
        Ok(())
    }
    /// Runtime validates commands and package icons before this one atomic publication point.
    pub(crate) fn reply(
        &self,
        request: &tree::Request,
        result: Result<tree::Page, Failure>,
        icons: BTreeMap<String, Arc<[u8]>>,
    ) -> Result<(), Failure> {
        self.live()?;
        let mut state = self.state.lock().unwrap();
        expire(&mut state);
        if request.tree != self.handle
            || state
                .pending
                .get(&request.sequence)
                .is_none_or(|pending| pending.request != *request)
        {
            return Err(stale());
        }
        if let Ok(page) = &result {
            validate_page(&state.snapshot, request, page)?;
            let mut metadata = state
                .snapshot
                .nodes
                .values()
                .map(|node| serde_json::to_vec(node).map_or(usize::MAX, |bytes| bytes.len()))
                .sum::<usize>();
            metadata = metadata
                .saturating_add(serde_json::to_vec(page).map_or(usize::MAX, |bytes| bytes.len()));
            let artwork = state
                .snapshot
                .icons
                .iter()
                .filter(|(path, _)| !icons.contains_key(*path))
                .map(|(_, bytes)| bytes.len())
                .sum::<usize>()
                + icons.values().map(|bytes| bytes.len()).sum::<usize>();
            if metadata > 2 * 1024 * 1024 || artwork > 2 * 1024 * 1024 {
                return Err(Failure::new(
                    ErrorCode::LimitExceeded,
                    "Tree retained metadata or icon budget exceeded",
                ));
            }
        }
        state.pending.remove(&request.sequence);
        let branch = state
            .snapshot
            .branches
            .get_mut(&request.parent)
            .ok_or_else(stale)?;
        branch.loading = false;
        match result {
            Err(error) => {
                branch.error = Some(error.message.chars().take(512).collect());
            }
            Ok(page) => {
                if request.cursor.is_none() {
                    branch.children.clear();
                }
                branch
                    .children
                    .extend(page.nodes.iter().map(|node| node.id.clone()));
                branch.loaded = true;
                branch.next = page.next;
                branch.error = None;
                for node in page.nodes {
                    if node.branch {
                        state
                            .snapshot
                            .branches
                            .entry(Some(node.id.clone()))
                            .or_default();
                    }
                    state.snapshot.nodes.insert(node.id.clone(), node);
                }
                state.snapshot.icons.extend(icons);
            }
        }
        state.snapshot.revision += 1;
        Ok(())
    }
    fn live(&self) -> Result<(), Failure> {
        if self.is_live() {
            Ok(())
        } else {
            Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Tree owner was revoked",
            ))
        }
    }
}
/// Count the current subtree only; no event, text or undo history is retained.
fn subtree(snapshot: &TreeSnapshot, node: &str) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    let mut queue = vec![node.to_string()];
    while let Some(id) = queue.pop() {
        if result.insert(id.clone()) {
            if let Some(branch) = snapshot.branches.get(&Some(id)) {
                queue.extend(branch.children.clone());
            }
        }
    }
    result
}
fn stale() -> Failure {
    Failure::new(ErrorCode::StaleRevision, "Tree request or node is obsolete")
}
fn expire(state: &mut State) {
    let now = Instant::now();
    let expired = state
        .pending
        .iter()
        .filter(|(_, pending)| now >= pending.deadline)
        .map(|(id, pending)| (*id, pending.request.parent.clone()))
        .collect::<Vec<_>>();
    for (id, parent) in expired {
        state.pending.remove(&id);
        if let Some(branch) = state.snapshot.branches.get_mut(&parent) {
            branch.loading = false;
            branch.error = Some("timeout".into());
        }
        state.snapshot.revision += 1;
    }
}
/// Duplicate IDs, cycles, oversized pages and excessive depth fail before changing any visible page.
fn validate_page(
    snapshot: &TreeSnapshot,
    request: &tree::Request,
    page: &tree::Page,
) -> Result<(), Failure> {
    if page.nodes.len() > tree::MAX_PAGE
        || snapshot.nodes.len() + page.nodes.len() > tree::MAX_NODES
        || page
            .next
            .as_ref()
            .is_some_and(|cursor| cursor.is_empty() || cursor.len() > 128)
        || serde_json::to_vec(page).map_or(true, |bytes| bytes.len() > 512 * 1024)
    {
        return Err(Failure::new(
            ErrorCode::LimitExceeded,
            "Tree page quota exceeded",
        ));
    }
    let mut seen = BTreeSet::new();
    for node in &page.nodes {
        node.validate()?;
        if snapshot.nodes.contains_key(&node.id) || !seen.insert(&node.id) {
            return Err(Failure::new(
                ErrorCode::Conflict,
                "Tree node IDs must be unique in this generation",
            ));
        }
    }
    let mut parent = request.parent.as_ref();
    let mut depth = 0;
    while let Some(id) = parent {
        depth += 1;
        if depth >= 64 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Tree depth exceeded",
            ));
        }
        parent = snapshot
            .branches
            .iter()
            .find(|(_, branch)| branch.children.contains(id))
            .and_then(|(parent, _)| parent.as_ref());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A native page reply must never resurrect a branch folded while its provider was working.
    #[test]
    fn lazy_pages_preserve_continuation_and_refuse_folded_and_refreshed_replies() {
        let resource = TreeResource::new(
            ResourceHandle {
                instance: "owner".into(),
                scope: "scope".into(),
                resource: 1,
            },
            EditorAuthority::new(Arc::new(AtomicBool::new(true)), None),
        );
        let root = resource.request(None, false).unwrap().unwrap();
        resource
            .reply(
                &root,
                Ok(tree::Page {
                    nodes: vec![tree::Node::new("table", "表格", true)],
                    next: Some("next-root".into()),
                }),
                Default::default(),
            )
            .unwrap();
        let child = resource
            .request(Some("table".into()), false)
            .unwrap()
            .unwrap();
        resource.collapse("table");
        let late = resource
            .reply(
                &child,
                Ok(tree::Page {
                    nodes: vec![tree::Node::new("late", "late", false)],
                    next: None,
                }),
                Default::default(),
            )
            .unwrap_err();
        assert_eq!(late.code, ErrorCode::StaleRevision);
        assert!(!resource.snapshot().nodes.contains_key("late"));
        let next = resource.request(None, true).unwrap().unwrap();
        assert_eq!(next.cursor.as_deref(), Some("next-root"));
        resource.refresh().unwrap();
        assert_eq!(
            resource
                .reply(
                    &next,
                    Err(Failure::new(ErrorCode::OperationFailed, "old failure")),
                    Default::default()
                )
                .unwrap_err()
                .code,
            ErrorCode::StaleRevision
        );
        assert!(resource.snapshot().nodes.is_empty());
    }
}
