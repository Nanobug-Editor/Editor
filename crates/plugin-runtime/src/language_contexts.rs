//! Registered project references seal opaque provider choices to native scope and owner lifetimes.
use crate::EditorAuthority;
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
    references: BTreeMap<String, RegisteredContext>,
    selected: BTreeMap<(String, String, String, String), String>,
    /// Native navigation lends one current source context to the next exact target preparation.
    navigation: BTreeMap<(String, String, String), (String, Instant)>,
}
impl std::fmt::Debug for LanguageContexts {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        output
            .debug_struct("LanguageContexts")
            .finish_non_exhaustive()
    }
}

impl LanguageContexts {
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
        state.selected.insert(key, token.into());
        Ok(context)
    }

    /// A runtime-issued refresh retains its exact registration instead of trusting guest identity strings.
    pub(crate) fn select_registered(
        &self,
        context: &RegisteredContext,
        path: &str,
    ) -> Result<(), Failure> {
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
        state.selected.insert(key, context.reference.token.clone());
        state.navigation.insert(
            hint,
            (
                context.reference.token,
                Instant::now() + Duration::from_secs(30),
            ),
        );
        Ok(())
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

    /// The next native prepare consumes the hint; it never writes a durable user preference.
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
            .retain(|(_, candidate, _, source), _| candidate != scope || source != path);
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
        state.references.get(token).cloned()
    }

    /// Settings/cutover revoke registered generations before callbacks can issue another native request.
    pub(crate) fn revoke_owner(&self, owner: &str) {
        let mut state = self.0.lock().unwrap();
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
    fn sweep(&mut self) {
        self.references.retain(|_, context| context.is_live());
        self.selected
            .retain(|_, token| self.references.contains_key(token));
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
