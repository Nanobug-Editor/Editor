//! Registered project references seal opaque provider choices to native scope and owner lifetimes.
use crate::EditorAuthority;
mod discovery;
use discovery::Discovery;
use plugin_protocol::{
    api::{ErrorCode, Failure},
    language_context::{Descriptor, Reference},
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// Immutable registration guards can narrow a plan's lifetime without owning mutable source text.
#[derive(Clone)]
pub(crate) struct RegisteredContext {
    pub reference: Reference,
    pub root: PathBuf,
    workspace: PathBuf,
    owner: String,
    scope: String,
    authority: EditorAuthority,
    seal: Arc<AtomicBool>,
}
impl RegisteredContext {
    pub(crate) fn is_live(&self) -> bool {
        self.authority.is_live() && self.seal.load(Ordering::Acquire)
    }
    pub(crate) fn guard(&self) -> Arc<AtomicBool> {
        self.seal.clone()
    }
}

/// The registry contains metadata only; actual tools still use the ordinary approved language plan.
#[derive(Clone, Default)]
pub struct LanguageContexts(Arc<Mutex<State>>);
#[derive(Default)]
struct State {
    /// Instance IDs already identify a single scope. Epoch guards survive completed service calls.
    discovery: BTreeMap<String, Discovery>,
    references: BTreeMap<String, RegisteredContext>,
    selected: BTreeMap<(String, String, String, String), SourceSelection>,
    /// Native navigation lends one current source context to the next exact target preparation.
    navigation: BTreeMap<(String, String, String), (String, Instant)>,
}

/// A source choice seals transient waits and binding handoffs without owning its shared process.
struct SourceSelection {
    token: String,
    alive: Arc<AtomicBool>,
}
impl std::fmt::Debug for LanguageContexts {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        output
            .debug_struct("LanguageContexts")
            .finish_non_exhaustive()
    }
}

impl LanguageContexts {
    /// Metadata liveness is independent from filesystem observation; paths are checked again on use.
    pub(crate) fn check(&self, token: &str, owner: &str, scope: &str) -> Result<(), Failure> {
        let mut state = self.0.lock().unwrap();
        state.sweep();
        state
            .references
            .get(token)
            .filter(|entry| entry.owner == owner && entry.scope == scope)
            .map(|_| ())
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidHandle,
                    "Unknown, foreign or retired project context",
                )
            })
    }
    /// Fully validate a batch before replacing any generation. A malformed tail publishes no prefix.
    pub(crate) fn register(
        &self,
        owner: &str,
        scope: &str,
        workspace: &Path,
        descriptors: Vec<Descriptor>,
        authority: EditorAuthority,
    ) -> Result<Vec<Reference>, Failure> {
        if descriptors.is_empty() || descriptors.len() > 128 || !authority.is_live() {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Invalid context registration batch",
            ));
        }
        if serde_json::to_vec(&descriptors).map_or(true, |bytes| bytes.len() > 64 * 1024) {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Context registration exceeds 64 KiB",
            ));
        }
        let workspace = workspace.canonicalize().map_err(path_error)?;
        let mut keys = std::collections::BTreeSet::new();
        let mut prepared = Vec::new();
        for descriptor in descriptors {
            if !descriptor.valid()
                || !keys.insert((descriptor.provider.clone(), descriptor.key.clone()))
            {
                return Err(Failure::new(
                    ErrorCode::InvalidRequest,
                    "Invalid or duplicate project context",
                ));
            }
            let root = workspace
                .join(&descriptor.project_root)
                .canonicalize()
                .map_err(path_error)?;
            if !root.starts_with(&workspace) || !root.is_dir() {
                return Err(Failure::new(
                    ErrorCode::InvalidPath,
                    "Project context escapes workspace",
                ));
            }
            prepared.push((descriptor, root));
        }
        let mut state = self.0.lock().unwrap();
        state.sweep();
        let existing = state
            .references
            .values()
            .filter(|entry| entry.owner == owner && entry.scope == scope)
            .count();
        let added = prepared
            .iter()
            .filter(|(descriptor, _)| {
                !state.references.values().any(|entry| {
                    entry.owner == owner
                        && entry.scope == scope
                        && entry.reference.descriptor.provider == descriptor.provider
                        && entry.reference.descriptor.key == descriptor.key
                })
            })
            .count();
        if existing + added > 128 || state.references.len() + added > 1024 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Project context reference quota exceeded",
            ));
        }
        // Reject regressing generations before mutating even one existing registration.
        for (descriptor, _) in &prepared {
            // A caller-scoped logical key cannot silently borrow another live chain's lifetime.
            // Consumers namespace their keys; retired chains are removed by sweep before reuse.
            if state.references.values().any(|entry| {
                entry.owner == owner
                    && entry.scope == scope
                    && entry.reference.descriptor.provider == descriptor.provider
                    && entry.reference.descriptor.key == descriptor.key
                    && !entry.authority.same_owner(&authority)
            }) {
                return Err(Failure::new(
                    ErrorCode::InvalidHandle,
                    "Project context key belongs to another live caller chain",
                ));
            }
            if state.references.values().any(|entry| {
                entry.owner == owner
                    && entry.scope == scope
                    && entry.reference.descriptor.provider == descriptor.provider
                    && entry.reference.descriptor.key == descriptor.key
                    && entry.reference.descriptor.generation > descriptor.generation
            }) {
                return Err(Failure::new(
                    ErrorCode::InvalidHandle,
                    "Project context generation regressed",
                ));
            }
        }
        let mut result = Vec::new();
        for (descriptor, root) in prepared {
            if let Some(entry) = state.references.values().find(|entry| {
                entry.owner == owner
                    && entry.scope == scope
                    && entry.reference.descriptor == descriptor
                    && entry.root == root
            }) {
                result.push(entry.reference.clone());
                continue;
            }
            for entry in state.references.values().filter(|entry| {
                entry.owner == owner
                    && entry.scope == scope
                    && entry.reference.descriptor.provider == descriptor.provider
                    && entry.reference.descriptor.key == descriptor.key
            }) {
                entry.seal.store(false, Ordering::Release);
            }
            let reference = Reference {
                token: uuid::Uuid::new_v4().to_string(),
                descriptor,
            };
            state.references.insert(
                reference.token.clone(),
                RegisteredContext {
                    reference: reference.clone(),
                    workspace: workspace.clone(),
                    root,
                    owner: owner.into(),
                    scope: scope.into(),
                    authority: authority.clone(),
                    seal: Arc::new(AtomicBool::new(true)),
                },
            );
            result.push(reference);
        }
        state.sweep();
        Ok(result)
    }

    /// Plain strings and a matching path never authorize another incarnation or logical workspace.
    pub(crate) fn resolve(
        &self,
        token: &str,
        owner: &str,
        scope: &str,
    ) -> Result<RegisteredContext, Failure> {
        let mut state = self.0.lock().unwrap();
        state.sweep();
        let context = state
            .references
            .get(token)
            .filter(|entry| entry.owner == owner && entry.scope == scope)
            .cloned()
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidHandle,
                    "Unknown, foreign or retired project context",
                )
            })?;
        let current = context
            .workspace
            .join(&context.reference.descriptor.project_root)
            .canonicalize()
            .map_err(path_error)?;
        if current != context.root || !current.starts_with(&context.workspace) {
            context.seal.store(false, Ordering::Release);
            return Err(Failure::new(
                ErrorCode::InvalidPath,
                "Registered context root changed",
            ));
        }
        Ok(context)
    }

    /// An owner chooses one display context per source. Other source paths and contexts remain live.
    pub(crate) fn select(
        &self,
        token: &str,
        owner: &str,
        scope: &str,
        path: &str,
    ) -> Result<RegisteredContext, Failure> {
        let context = self.resolve(token, owner, scope)?;
        validate_source(&context.workspace, path)?;
        let key = (
            owner.into(),
            scope.into(),
            context.reference.descriptor.provider.clone(),
            path.into(),
        );
        let mut state = self.0.lock().unwrap();
        state.sweep();
        if !state.selected.contains_key(&key)
            && (state.selected.len() >= 4096
                || state
                    .selected
                    .keys()
                    .filter(|(candidate, _, _, _)| candidate == owner)
                    .count()
                    >= 1024)
        {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Selected source quota exceeded",
            ));
        }
        state.select_source(key, token.into());
        Ok(context)
    }

    /// Capture the exact source choice after admission. Replacing it revokes this bit permanently,
    /// including A→B→A; independent documents and consumers keep their original process authority.
    pub(crate) fn selection_guard(
        &self,
        context: &RegisteredContext,
        path: &str,
    ) -> Result<Arc<AtomicBool>, Failure> {
        let state = self.0.lock().unwrap();
        let key = (
            context.owner.clone(),
            context.scope.clone(),
            context.reference.descriptor.provider.clone(),
            path.into(),
        );
        state
            .selected
            .get(&key)
            .filter(|selection| {
                selection.token == context.reference.token
                    && selection.alive.load(Ordering::Acquire)
                    && context.is_live()
            })
            .map(|selection| selection.alive.clone())
            .ok_or_else(|| Failure::new(ErrorCode::InvalidHandle, "Source context choice changed"))
    }

    /// A runtime-issued refresh retains its exact registration instead of trusting guest identity strings.
    pub(crate) fn select_registered(
        &self,
        context: &RegisteredContext,
        path: &str,
        expected: Option<&Arc<AtomicBool>>,
    ) -> Result<Arc<AtomicBool>, Failure> {
        let context = self.resolve(&context.reference.token, &context.owner, &context.scope)?;
        validate_source(&context.workspace, path)?;
        let key = (
            context.owner.clone(),
            context.scope.clone(),
            context.reference.descriptor.provider.clone(),
            path.into(),
        );
        let hint = (context.owner.clone(), context.scope.clone(), path.into());
        let mut state = self.0.lock().unwrap();
        state.sweep();
        let current = state.selected.get(&key);
        let unchanged = match (expected, current) {
            (None, None) => true,
            (Some(expected), Some(current)) => {
                Arc::ptr_eq(expected, &current.alive) && current.alive.load(Ordering::Acquire)
            }
            _ => false,
        };
        if !unchanged {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Source choice changed before refresh",
            ));
        }
        // Selection and its one-use preparation hint publish together after the native source check.
        // A prior durable preference must not override this explicit choice during alias refresh.
        if (!state.selected.contains_key(&key)
            && (state.selected.len() >= 4096
                || state
                    .selected
                    .keys()
                    .filter(|(owner, _, _, _)| owner == &context.owner)
                    .count()
                    >= 1024))
            || (!state.navigation.contains_key(&hint) && state.navigation.len() >= 1024)
        {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Context refresh quota exceeded",
            ));
        }
        state.select_source(key.clone(), context.reference.token.clone());
        state.navigation.insert(
            hint,
            (
                context.reference.token,
                Instant::now() + Duration::from_secs(30),
            ),
        );
        Ok(state.selected[&key].alive.clone())
    }

    /// Refresh captures the prior choice, including absence; commit checks that epoch under its lock.
    pub(crate) fn refresh_guard(
        &self,
        context: &RegisteredContext,
        path: &str,
    ) -> Option<Arc<AtomicBool>> {
        let state = self.0.lock().unwrap();
        state
            .selected
            .get(&(
                context.owner.clone(),
                context.scope.clone(),
                context.reference.descriptor.provider.clone(),
                path.into(),
            ))
            .map(|choice| choice.alive.clone())
    }

    /// Initial ordinary native plans may acquire provenance only when one registered identity matches.
    pub(crate) fn for_plan(
        &self,
        owner: &str,
        scope: &str,
        provider: &str,
        root: &Path,
    ) -> Option<RegisteredContext> {
        let mut state = self.0.lock().unwrap();
        state.sweep();
        let mut candidates = state.references.values().filter(|context| {
            context.owner == owner
                && context.scope == scope
                && context.reference.descriptor.provider == provider
                && context.root == root
        });
        let context = candidates.next()?.clone();
        candidates.next().is_none().then_some(context)
    }

    /// Only a live native source plan can lend this one-use hint. It grants no filesystem authority.
    pub(crate) fn navigation_hint(
        &self,
        context: &RegisteredContext,
        path: &str,
    ) -> Result<(), Failure> {
        let context = self.resolve(&context.reference.token, &context.owner, &context.scope)?;
        validate_source(&context.workspace, path)?;
        let mut state = self.0.lock().unwrap();
        state.sweep();
        let key = (context.owner.clone(), context.scope.clone(), path.into());
        if !state.navigation.contains_key(&key) && state.navigation.len() >= 1024 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Navigation context quota exceeded",
            ));
        }
        state.navigation.insert(
            key,
            (
                context.reference.token,
                Instant::now() + Duration::from_secs(30),
            ),
        );
        Ok(())
    }

    /// Cache admission may inspect a live navigation hint without spending its one-use handoff.
    pub(crate) fn navigation_context(
        &self,
        owner: &str,
        scope: &str,
        path: &str,
    ) -> Option<RegisteredContext> {
        let mut state = self.0.lock().unwrap();
        state.sweep();
        let (token, _) = state
            .navigation
            .get(&(owner.into(), scope.into(), path.into()))?;
        state.references.get(token).cloned()
    }

    /// Consume the hint only when an actual new delegated preparation begins.
    pub(crate) fn take_navigation_hint(
        &self,
        owner: &str,
        scope: &str,
        path: &str,
    ) -> Option<String> {
        let mut state = self.0.lock().unwrap();
        state.sweep();
        state
            .navigation
            .remove(&(owner.into(), scope.into(), path.into()))
            .map(|(token, _)| token)
    }

    /// A closed native incarnation cannot leave either a one-use hint or source selection behind.
    pub(crate) fn source_closed(&self, scope: &str, path: &str) {
        let mut state = self.0.lock().unwrap();
        state
            .selected
            .retain(|(_, candidate, _, source), selection| {
                let keep = candidate != scope || source != path;
                if !keep {
                    selection.alive.store(false, Ordering::Release);
                }
                keep
            });
        state
            .navigation
            .retain(|(_, candidate, source), _| candidate != scope || source != path);
    }

    pub(crate) fn selected(
        &self,
        owner: &str,
        scope: &str,
        provider: &str,
        path: &str,
    ) -> Option<RegisteredContext> {
        let mut state = self.0.lock().unwrap();
        state.sweep();
        let token =
            state
                .selected
                .get(&(owner.into(), scope.into(), provider.into(), path.into()))?;
        state.references.get(&token.token).cloned()
    }

    /// Settings/cutover revoke registered generations before callbacks can issue another native request.
    pub(crate) fn revoke_owner(&self, owner: &str) {
        let mut state = self.0.lock().unwrap();
        if let Some(epoch) = state.discovery.remove(owner)
            && let Some(alive) = epoch.alive
        {
            alive.store(false, Ordering::Release);
        }
        for entry in state
            .references
            .values()
            .filter(|entry| entry.owner == owner)
        {
            entry.seal.store(false, Ordering::Release);
        }
        state.sweep();
    }
}
impl State {
    /// Repeating one choice is idempotent; any replacement creates a new monotonic lifetime bit.
    fn select_source(&mut self, key: (String, String, String, String), token: String) {
        if self
            .selected
            .get(&key)
            .is_some_and(|selected| selected.token == token)
        {
            return;
        }
        if let Some(previous) = self.selected.insert(
            key,
            SourceSelection {
                token,
                alive: Arc::new(AtomicBool::new(true)),
            },
        ) {
            previous.alive.store(false, Ordering::Release);
        }
    }

    fn sweep(&mut self) {
        self.references.retain(|_, context| {
            let live = context.is_live();
            // Native requests may already hold only this seal. Removing borrowed metadata must
            // revoke those holders too, even when the retiring participant is a different owner.
            if !live {
                context.seal.store(false, Ordering::Release);
            }
            live
        });
        self.selected.retain(|_, selected| {
            let live = self.references.contains_key(&selected.token);
            if !live {
                selected.alive.store(false, Ordering::Release);
            }
            live
        });
        self.navigation.retain(|_, (token, deadline)| {
            *deadline > Instant::now() && self.references.contains_key(token)
        });
    }
}
fn path_error(error: std::io::Error) -> Failure {
    Failure::new(ErrorCode::InvalidPath, error.to_string())
}

/// New buffers may have no disk file, but their real parent must remain inside the sealed workspace.
fn validate_source(workspace: &Path, path: &str) -> Result<(), Failure> {
    plugin_protocol::api::ResourceIdentity::Local { path: path.into() }.validate()?;
    let target = workspace.join(path);
    let physical = if target.exists() {
        target.canonicalize().map_err(path_error)?
    } else {
        target
            .parent()
            .ok_or_else(|| Failure::new(ErrorCode::InvalidPath, "Source has no parent"))?
            .canonicalize()
            .map_err(path_error)?
            .join(target.file_name().unwrap())
    };
    if !physical.starts_with(workspace) {
        return Err(Failure::new(
            ErrorCode::InvalidPath,
            "Context source escapes workspace",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Source selection affects transient handoffs, independently from registered process metadata.
    use super::*;

    /// Existing native holders observe a supplier's domain retirement through their retained seal.
    #[test]
    fn borrowed_context_revocation_seals_retained_native_guards() {
        let workspace = tempfile::tempdir().unwrap();
        let registry = LanguageContexts::default();
        let authority = registry
            .borrow_discovery(
                EditorAuthority::new(Arc::new(AtomicBool::new(true)), None),
                &["supplier".into(), "engine".into()],
            )
            .unwrap();
        let reference = registry
            .register(
                "engine",
                "scope",
                workspace.path(),
                vec![Descriptor {
                    provider: "analysis".into(),
                    key: "project".into(),
                    generation: 1,
                    project_root: String::new(),
                    data: serde_json::Value::Null,
                }],
                authority,
            )
            .unwrap()
            .pop()
            .unwrap();
        let retained = registry
            .resolve(&reference.token, "engine", "scope")
            .unwrap();
        let guard = retained.guard();
        assert!(guard.load(Ordering::Acquire));
        registry.invalidate("supplier").unwrap();
        assert!(!retained.is_live());
        assert!(
            !guard.load(Ordering::Acquire),
            "Revoking a borrowed context must cancel native holders that already retained its seal"
        );
    }

    /// A→B→A cannot revive an old wait; selecting or closing another document remains independent.
    #[test]
    fn source_choice_revocation_is_monotonic_and_document_scoped() {
        let workspace = tempfile::tempdir().unwrap();
        for name in ["left", "right"] {
            std::fs::create_dir(workspace.path().join(name)).unwrap();
        }
        let registry = LanguageContexts::default();
        let authority = EditorAuthority::new(Arc::new(AtomicBool::new(true)), None);
        let references = registry
            .register(
                "owner",
                "scope",
                workspace.path(),
                ["left", "right"]
                    .map(|key| Descriptor {
                        provider: "engine".into(),
                        key: key.into(),
                        generation: 1,
                        project_root: key.into(),
                        data: serde_json::Value::Null,
                    })
                    .to_vec(),
                authority,
            )
            .unwrap();
        let left_a = registry
            .select(&references[0].token, "owner", "scope", "a.ts")
            .unwrap();
        let old_a = registry.selection_guard(&left_a, "a.ts").unwrap();
        let left_b = registry
            .select(&references[0].token, "owner", "scope", "b.ts")
            .unwrap();
        let old_b = registry.selection_guard(&left_b, "b.ts").unwrap();
        registry
            .select(&references[0].token, "owner", "scope", "a.ts")
            .unwrap();
        assert!(old_a.load(Ordering::Acquire), "same choice is idempotent");

        registry
            .select(&references[1].token, "owner", "scope", "a.ts")
            .unwrap();
        assert!(!old_a.load(Ordering::Acquire));
        assert!(
            old_b.load(Ordering::Acquire),
            "other source waits stay live"
        );
        assert!(
            left_a.is_live(),
            "selection never retires shared configuration metadata"
        );
        let returned_a = registry
            .select(&references[0].token, "owner", "scope", "a.ts")
            .unwrap();
        let new_a = registry.selection_guard(&returned_a, "a.ts").unwrap();
        assert!(new_a.load(Ordering::Acquire));
        assert!(
            !old_a.load(Ordering::Acquire),
            "returning to A cannot revive its old wait"
        );
        assert_eq!(
            registry
                .select_registered(&returned_a, "a.ts", Some(&old_a))
                .unwrap_err()
                .code,
            ErrorCode::InvalidHandle,
            "a queued refresh cannot pass an A→B→A choice change"
        );
        assert!(
            new_a.load(Ordering::Acquire),
            "rejected refresh preserves the current choice"
        );

        registry.navigation_hint(&returned_a, "target.ts").unwrap();
        for _ in 0..2 {
            assert_eq!(
                registry
                    .navigation_context("owner", "scope", "target.ts")
                    .unwrap()
                    .reference
                    .token,
                returned_a.reference.token,
                "cache checks must not consume a navigation handoff"
            );
        }
        assert_eq!(
            registry.take_navigation_hint("owner", "scope", "target.ts"),
            Some(returned_a.reference.token.clone())
        );
        assert!(
            registry
                .navigation_context("owner", "scope", "target.ts")
                .is_none()
        );
        registry.navigation_hint(&left_b, "b.ts").unwrap();
        registry.source_closed("scope", "b.ts");
        assert!(
            registry
                .navigation_context("owner", "scope", "b.ts")
                .is_none()
        );
        assert!(!old_b.load(Ordering::Acquire));
        assert!(new_a.load(Ordering::Acquire));
        registry.revoke_owner("owner");
        assert!(!new_a.load(Ordering::Acquire));
    }
}
