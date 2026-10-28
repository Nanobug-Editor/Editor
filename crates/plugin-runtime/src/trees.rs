//! Bounded tree resources shared by the serial guest worker and native Base tree projection.
//! Only provider metadata is retained here; text, document selection and tree scrolling stay native.
use crate::EditorAuthority;
mod patch;
mod reveal;
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
#[derive(Clone, Debug, Default, PartialEq)]
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
    /// One pending exact ID path (initial default or explicit reveal); native input or delivery
    /// withdraws it. Explicit requests validate cached ancestry before entering this native seam.
    pub initial_selection: Option<Vec<String>>,
}
struct Pending {
    request: tree::Request,
    deadline: Instant,
}
struct State {
    snapshot: TreeSnapshot,
    next: u64,
    pending: BTreeMap<u64, Pending>,
    /// Defaults cannot be replayed after native input, even if it preceded the provider's first call.
    initialized: bool,
    /// Explicit navigation belongs to one admitted forest; structural replacement withdraws it.
    explicit_reveal: bool,
}
/// Every node and nested native menu shares one admitted artwork set across update and patch.
fn artwork_paths<'a>(nodes: impl Iterator<Item = &'a tree::Node>) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for node in nodes {
        if let Some(icon) = &node.icon {
            paths.extend([icon.light.clone(), icon.dark.clone()]);
        }
        for item in &node.action_menu {
            item.visit(&mut |item| {
                if let Some(icon) = &item.icon {
                    paths.extend([icon.light.clone(), icon.dark.clone()]);
                }
            });
        }
    }
    paths
}
/// Runtime-issued lifetime authority prevents stale native entities from reviving retired providers.
pub struct TreeResource {
    handle: ResourceHandle,
    authority: EditorAuthority,
    /// Creation-time preferences cannot be changed by retained native scene callbacks.
    options: tree::Options,
    revoked: AtomicBool,
    state: Mutex<State>,
}
impl TreeResource {
    /// Only the admitted instance allocates a tree; callers never supply a replacement owner.
    pub(crate) fn new(
        handle: ResourceHandle,
        authority: EditorAuthority,
        options: tree::Options,
    ) -> Self {
        Self {
            handle,
            authority,
            options,
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
                initialized: false,
                explicit_reveal: false,
            }),
        }
    }
    pub fn handle(&self) -> &ResourceHandle {
        &self.handle
    }
    /// Read the admitted immutable pointer preference, independent of provider pages and epochs.
    pub fn selection_gesture(&self) -> tree::SelectionGesture {
        self.options.selection
    }
    /// Directory gestures are fixed at creation and cannot change during a retained native press.
    pub fn single_click_expand(&self) -> bool {
        self.options.single_click_expand
    }

    /// The provider's admitted immutable font preference does not alter tree state or input authority.
    pub fn proportional_labels(&self) -> bool {
        self.options.proportional_labels
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
        if state.explicit_reveal {
            state.snapshot.initial_selection = None;
            state.explicit_reveal = false;
        }
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
    /// Capture cached IDs for metadata admission without expiring or otherwise changing pending pages.
    pub(crate) fn cached_ids(&self) -> BTreeSet<String> {
        self.state
            .lock()
            .unwrap()
            .snapshot
            .nodes
            .keys()
            .cloned()
            .collect()
    }
    /// Admit one bounded default `path` without loading pages or granting activation authority.
    pub(crate) fn initialize(&self, path: Vec<String>) -> Result<(), Failure> {
        self.live()?;
        if path.is_empty()
            || path.len() > 64
            || path.iter().any(|id| !tree::valid_id(id))
            || path.iter().collect::<BTreeSet<_>>().len() != path.len()
        {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Invalid initial tree selection path",
            ));
        }
        let mut state = self.state.lock().unwrap();
        if !state.initialized {
            state.initialized = true;
            state.snapshot.initial_selection = Some(path);
            state.snapshot.revision += 1;
        }
        Ok(())
    }
    /// Native user input and completed/missing defaults permanently finish this resource's seed.
    /// Does not change selection, disclosure, cached pages or pending ordinary requests.
    pub fn dismiss_initial_selection(&self) {
        let mut state = self.state.lock().unwrap();
        state.initialized = true;
        state.explicit_reveal = false;
        if state.snapshot.initial_selection.take().is_some() {
            state.snapshot.revision += 1;
        }
    }
    /// Change only cached branch disclosure. Validate the entire ID list before any mutation;
    /// native reconciliation requests missing child pages under the usual pending-page quota.
    pub(crate) fn set_expanded(&self, nodes: Vec<String>, expanded: bool) -> Result<(), Failure> {
        self.live()?;
        if nodes.len() > tree::MAX_NODES || nodes.iter().any(|id| !tree::valid_id(id)) {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Invalid tree disclosure request",
            ));
        }
        if !expanded {
            for id in nodes {
                self.collapse(&id);
            }
            return Ok(());
        }
        let mut state = self.state.lock().unwrap();
        let mut changed = false;
        for id in nodes {
            if state
                .snapshot
                .nodes
                .get(&id)
                .is_some_and(|node| node.branch && !node.disabled)
            {
                changed |= state.snapshot.expanded.insert(id);
            }
        }
        if changed {
            state.snapshot.revision += 1;
        }
        Ok(())
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
        if state.explicit_reveal {
            state.snapshot.initial_selection = None;
            state.explicit_reveal = false;
        }
        state.snapshot = TreeSnapshot {
            epoch: state.snapshot.epoch + 1,
            revision: state.snapshot.revision + 1,
            branches: [(None, TreeBranch::default())].into(),
            expanded: state.snapshot.expanded.clone(),
            initial_selection: state.snapshot.initial_selection.clone(),
            ..Default::default()
        };
        state.pending.clear();
        Ok(())
    }
    /// Replace only cached display metadata from `nodes` and their already-admitted package `icons`.
    /// Every node is validated before mutation; unknown IDs are ignored and changed structure or
    /// command intent is `Conflict`. Pending pages, epoch and disclosure state remain untouched.
    pub(crate) fn update(
        &self,
        nodes: Vec<tree::Node>,
        icons: BTreeMap<String, Arc<[u8]>>,
    ) -> Result<(), Failure> {
        self.live()?;
        let mut state = self.state.lock().unwrap();
        if nodes.len() > tree::MAX_PAGE
            || serde_json::to_vec(&nodes).map_or(true, |bytes| bytes.len() > 512 * 1024)
        {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Tree update quota exceeded",
            ));
        }
        let mut seen = BTreeSet::new();
        let mut changed = false;
        for node in &nodes {
            node.validate()?;
            if !seen.insert(&node.id) {
                return Err(Failure::new(
                    ErrorCode::Conflict,
                    "Duplicate tree update ID",
                ));
            }
            if let Some(previous) = state.snapshot.nodes.get(&node.id) {
                validate_update_intent(previous, node)?;
                changed |= previous != node;
            }
        }
        if !changed {
            return Ok(());
        }
        // Build a bounded candidate before publishing. Unknown provider IDs never become native
        // rows, and unused incoming artwork cannot consume the resource's retained icon budget.
        let mut candidate = state.snapshot.nodes.clone();
        for node in nodes {
            if let Some(previous) = candidate.get_mut(&node.id) {
                *previous = node;
            }
        }
        let paths = artwork_paths(candidate.values());
        let mut artwork = state.snapshot.icons.clone();
        artwork.extend(icons.into_iter().filter(|(path, _)| paths.contains(path)));
        artwork.retain(|path, _| paths.contains(path));
        let metadata_bytes = candidate
            .values()
            .map(|node| serde_json::to_vec(node).map_or(usize::MAX, |bytes| bytes.len()))
            .fold(0usize, usize::saturating_add);
        let artwork_bytes = artwork.values().map(|bytes| bytes.len()).sum::<usize>();
        if metadata_bytes > 2 * 1024 * 1024 || artwork_bytes > 2 * 1024 * 1024 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Tree retained metadata or icon budget exceeded",
            ));
        }
        state.snapshot.nodes = candidate;
        state.snapshot.icons = artwork;
        state.snapshot.revision = state.snapshot.revision.saturating_add(1);
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
            tree::NodeAction::MenuItem { id } => {
                return plugin_protocol::ui::action_menu_command(&node.action_menu, id)
                    .ok_or_else(stale);
            }
        }
        .filter(|command| !command.disabled)
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
            tree::Event::Checked {
                tree,
                epoch,
                node,
                previous,
                checked,
            } => {
                if tree != &self.handle
                    || *epoch != state.snapshot.epoch
                    || *checked != (*previous != tree::CheckState::Checked)
                    || state
                        .snapshot
                        .nodes
                        .get(node)
                        .is_none_or(|node| node.disabled || node.checked != Some(*previous))
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
/// Cosmetic replacement cannot retarget callbacks captured in the current tree epoch.
fn validate_update_intent(previous: &tree::Node, next: &tree::Node) -> Result<(), Failure> {
    // Presentation and availability may change; stable menu IDs, command arguments and hierarchy
    // cannot be retargeted under callbacks already admitted for this tree epoch.
    fn menu_intent(items: &[plugin_protocol::ui::ActionMenuItem]) -> serde_json::Value {
        use plugin_protocol::ui::ActionMenuKind;
        serde_json::Value::Array(items.iter().map(|item| {
            let kind = match &item.kind {
                ActionMenuKind::Action { command } => serde_json::json!({"command":command.command,"arguments":command.arguments}),
                ActionMenuKind::Group { items, .. } => serde_json::json!({"group":menu_intent(items)}),
                ActionMenuKind::Submenu { items, on_open } => serde_json::json!({"submenu":menu_intent(items),"on_open":on_open.as_ref().map(|command| (&command.command, &command.arguments))}),
            };
            serde_json::json!({"id":item.id,"kind":kind})
        }).collect())
    }
    let same_command = |before: &tree::Command, after: &tree::Command| {
        before.command == after.command
            && before.arguments == after.arguments
            && before.separator_before == after.separator_before
    };
    if previous.branch != next.branch
        || match (&previous.activation, &next.activation) {
            (Some(before), Some(after)) => !same_command(before, after),
            (None, None) => false,
            _ => true,
        }
        || previous.menu.len() != next.menu.len()
        || menu_intent(&previous.action_menu) != menu_intent(&next.action_menu)
        || !previous
            .menu
            .iter()
            .zip(&next.menu)
            .all(|(before, after)| same_command(before, after))
    {
        return Err(Failure::new(
            ErrorCode::Conflict,
            "Tree update cannot change structure or command intent",
        ));
    }
    Ok(())
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
            tree::Options::default(),
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

#[cfg(test)]
#[path = "trees/check_tests.rs"]
mod check_tests;
#[cfg(test)]
#[path = "trees/initial_tests.rs"]
mod initial_tests;
#[cfg(test)]
#[path = "trees/update_tests.rs"]
mod update_tests;
