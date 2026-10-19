//! Each native project reads and mutates only its own effective provider selection.
use super::*;
impl Providers {
    /// A refresh replaces all contributions together; disabled and failed packages are absent upstream.
    pub(crate) fn refresh(
        &self,
        root: &Path,
        entries: Vec<(String, PathBuf, Vec<LanguageDefinition>, Vec<Highlighter>)>,
        services: Vec<(
            String,
            Vec<plugin_runtime::plugin_protocol::language::Provider>,
        )>,
    ) {
        self.refresh_with_structures(root, entries, services, Vec::new());
    }

    /// Publish independent language roles as one snapshot; callers without structure use `refresh`.
    pub(crate) fn refresh_with_structures(
        &self,
        root: &Path,
        entries: Vec<(String, PathBuf, Vec<LanguageDefinition>, Vec<Highlighter>)>,
        services: Vec<(
            String,
            Vec<plugin_runtime::plugin_protocol::language::Provider>,
        )>,
        structures: Vec<(
            String,
            Vec<plugin_runtime::plugin_protocol::structure::Provider>,
        )>,
    ) {
        self.refresh_with_language_features(root, entries, services, structures, Vec::new());
    }

    /// Publish every language role atomically so registration order cannot override a valid choice.
    pub(crate) fn refresh_with_language_features(
        &self,
        root: &Path,
        entries: Vec<(String, PathBuf, Vec<LanguageDefinition>, Vec<Highlighter>)>,
        services: Vec<(
            String,
            Vec<plugin_runtime::plugin_protocol::language::Provider>,
        )>,
        structures: Vec<(
            String,
            Vec<plugin_runtime::plugin_protocol::structure::Provider>,
        )>,
        direct: Vec<(
            String,
            Vec<plugin_runtime::plugin_protocol::direct::Provider>,
        )>,
    ) {
        let mut next = Registry::default();
        for (owner, providers) in direct {
            for provider in providers {
                for feature in provider.features {
                    next.language_features
                        .entry(format!("{}:{}", feature.role(), provider.language))
                        .or_default()
                        .push(format!("{owner}/{}", provider.id));
                }
            }
        }
        for (owner, providers) in services {
            for provider in providers {
                let key = format!("{owner}/{}", provider.id);
                for language in std::iter::once(&provider.language).chain(&provider.languages) {
                    if provider.primary {
                        next.language_servers
                            .entry(language.clone())
                            .or_default()
                            .push(key.clone());
                    }
                    if provider.formatting {
                        next.formatters
                            .entry(language.clone())
                            .or_default()
                            .push(key.clone());
                    }
                }
            }
        }
        for (owner, providers) in structures {
            for provider in providers {
                next.structure_providers
                    .entry(provider.language)
                    .or_default()
                    .push(format!("{owner}/{}", provider.id));
            }
        }
        for (owner, root, definitions, highlighters) in entries {
            for definition in definitions {
                for key in definition
                    .extensions
                    .iter()
                    .map(|s| format!("ext:{}", s.to_lowercase()))
                    .chain(
                        definition
                            .filenames
                            .iter()
                            .map(|s| format!("file:{}", s.to_lowercase())),
                    )
                {
                    next.recognizers
                        .entry(key)
                        .or_default()
                        .push((owner.clone(), definition.clone()));
                }
            }
            for declaration in highlighters {
                next.highlighters
                    .entry(declaration.language.clone())
                    .or_default()
                    .push(GrammarProvider {
                        owner: owner.clone(),
                        root: root.clone(),
                        declaration,
                    });
            }
        }
        let mut registry = self.write();
        if !same_root(&registry.root, root) {
            *registry = configured(root, "");
        }
        next.root = registry.root.clone();
        next.workspace = registry.workspace.clone();
        next.saved = registry.saved.clone();
        next.error = registry.error.clone();
        next.resolve();
        // The shared projection guard persists sole-provider adoption once before releasing its authority lock.
        // Alias declarations affect code-block selection just as much as the selected grammar does.
        // Equal refreshes preserve reusable work; every observable change invalidates old jobs.
        next.highlight_epoch = if registry.root == next.root
            && registry.workspace == next.workspace
            && registry.recognizers == next.recognizers
            && registry.highlighters == next.highlighters
            && registry.language_features == next.language_features
            && registry.selected == next.selected
        {
            registry.highlight_epoch
        } else {
            next_highlight_epoch()
        };
        let epoch = next.highlight_epoch;
        *registry = next;
        drop(registry);
        crate::language::code_highlighting::invalidate_prepared(self.scope(), epoch);
    }

    pub(crate) fn rows(&self) -> Vec<ProviderRow> {
        let registry = self.read();
        let mut rows = registry.candidates();
        for key in registry
            .saved
            .automatic
            .get(&registry.workspace)
            .into_iter()
            .flatten()
            .map(|(key, _)| key)
        {
            rows.entry(key.clone()).or_default();
        }
        rows.into_iter()
            .map(|(key, candidates)| {
                let selected = registry.selected.get(&key).cloned();
                let configuration_error = registry.formatter_preference_errors.get(&key).cloned();
                let source = if let Some(error) = &configuration_error {
                    error.source
                } else if registry
                    .saved
                    .projects
                    .get(&registry.workspace)
                    .and_then(|m| m.get(&key))
                    .is_some_and(|id| Some(id) == selected.as_ref())
                {
                    "project"
                } else if registry
                    .saved
                    .user
                    .get(&key)
                    .is_some_and(|id| Some(id) == selected.as_ref())
                {
                    "user"
                } else {
                    "automatic"
                };
                ProviderRow {
                    key,
                    candidates,
                    selected,
                    source,
                    configuration_error,
                }
            })
            .collect()
    }

    pub(crate) fn error(&self) -> Option<String> {
        self.read().error.clone()
    }

    /// Explicit native UI selection persists atomically before changing the active provider snapshot.
    pub(crate) fn choose(
        &self,
        scope: Scope,
        key: &str,
        provider: Option<&str>,
    ) -> anyhow::Result<()> {
        let mut registry = self.write();
        anyhow::ensure!(
            registry.error.is_none(),
            "Provider preferences could not be loaded or saved"
        );
        let candidates = registry.candidates();
        anyhow::ensure!(candidates.contains_key(key), "Unknown provider role");
        if let Some(provider) = provider {
            anyhow::ensure!(
                candidates[key].iter().any(|id| id == provider),
                "Provider is unavailable"
            );
        }
        let mut saved = registry.saved.clone();
        let layer = match scope {
            Scope::User => &mut saved.user,
            Scope::Project => {
                anyhow::ensure!(!registry.workspace.is_empty(), "No active workspace");
                saved
                    .projects
                    .entry(registry.workspace.clone())
                    .or_default()
            }
        };
        if let Some(provider) = provider {
            layer.insert(key.into(), provider.into());
        } else {
            layer.remove(key);
        }
        // This UI entry has just checked live candidates. Persist its exact value with the same
        // atomic snapshot; resetting a layer also removes its proof rather than retaining history.
        saved
            .formatter_choices
            .record(scope, &registry.workspace, key, provider);
        // A reset deliberately relinquishes remembered choice in this workspace before fallback resolution.
        saved
            .automatic
            .entry(registry.workspace.clone())
            .or_default()
            .remove(key);
        let mut next = (*registry).clone();
        next.saved = saved;
        next.resolve();
        if next.selected != registry.selected {
            next.highlight_epoch = next_highlight_epoch();
        }
        registry.commit(next)?;
        let epoch = registry.highlight_epoch;
        drop(registry);
        crate::language::code_highlighting::invalidate_prepared(self.scope(), epoch);
        Ok(())
    }

    /// Recognition is available independently of whether the grammar exists or loaded successfully.
    pub(crate) fn language_for_path(&self, path: &Path) -> Option<String> {
        let registry = self.read();
        let filename = path.file_name()?.to_str()?.to_lowercase();
        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        // An explicit association wins while its language exists; a missing provider stays plain.
        if let Some(language) = registry.saved.associations.get(&extension) {
            return registry.code_language(language);
        }
        let key = if registry
            .recognizers
            .contains_key(&format!("file:{filename}"))
        {
            format!("file:{filename}")
        } else {
            format!("ext:{extension}")
        };
        let selected = registry.selected.get(&format!("recognition:{key}"))?;
        registry
            .recognizers
            .get(&key)?
            .iter()
            .find(|(owner, definition)| format!("{owner}/{}", definition.id) == *selected)
            .map(|(_, definition)| definition.id.clone())
    }

    pub(crate) fn grammars(&self) -> Vec<GrammarProvider> {
        let registry = self.read();
        registry
            .highlighters
            .iter()
            .filter_map(|(language, list)| {
                let selected = registry.selected.get(&format!("highlight:{language}"))?;
                list.iter()
                    .find(|p| format!("{}/{}", p.owner, p.declaration.id) == *selected)
                    .cloned()
            })
            .collect()
    }

    /// LSP selection shares preference precedence but never depends on the highlighter provider.
    pub(crate) fn language_servers(&self) -> BTreeMap<String, Option<String>> {
        let registry = self.read();
        registry
            .language_servers
            .keys()
            .map(|language| {
                (
                    language.clone(),
                    registry.selected.get(&format!("lsp:{language}")).cloned(),
                )
            })
            .collect()
    }

    /// Structure follows the same stable choice and user/project precedence as every language role.
    pub(crate) fn structure_provider(&self, language: &str) -> Option<String> {
        self.read()
            .selected
            .get(&format!("structure:{language}"))
            .cloned()
    }

    /// A feature's direct provider choice is independent from the single primary LSP and grammar.
    pub(crate) fn language_feature_provider(
        &self,
        language: &str,
        feature: plugin_runtime::plugin_protocol::direct::Feature,
    ) -> Option<String> {
        self.read()
            .selected
            .get(&format!("{}:{language}", feature.role()))
            .cloned()
    }

    /// Presence is distinct from selection: ambiguous direct candidates must not silently fall back.
    pub(crate) fn has_language_feature(
        &self,
        language: &str,
        feature: plugin_runtime::plugin_protocol::direct::Feature,
    ) -> bool {
        self.read()
            .language_features
            .contains_key(&format!("{}:{language}", feature.role()))
    }

    /// A removed or replaced provider cannot register a parser after its task completes.
    pub(crate) fn is_current(&self, provider: &GrammarProvider) -> bool {
        self.is_live() && self.grammars().contains(provider)
    }

    /// Snapshot identity and generation together so a worker cannot observe a torn provider choice.
    pub(in crate::language) fn code_provider(
        &self,
        language: &str,
    ) -> Option<(GrammarProvider, u64)> {
        let registry = self.read();
        let language = registry.code_language(language)?;
        Some((
            registry.grammar(&language)?.clone(),
            registry.highlight_epoch,
        ))
    }

    /// Native owners observe this generation even when no grammar is currently available.
    pub(in crate::language) fn code_epoch(&self) -> u64 {
        self.read().highlight_epoch
    }

    /// Native language requests capture this monotonic selection generation, including A→B→A changes.
    pub(crate) fn feature_generation(&self) -> u64 {
        self.read().highlight_epoch
    }

    /// Equality alone cannot reject results from a provider that was removed then reinstalled.
    pub(in crate::language) fn code_is_current(
        &self,
        provider: &GrammarProvider,
        epoch: u64,
    ) -> bool {
        let registry = self.read();
        self.is_live()
            && registry.highlight_epoch == epoch
            && registry.grammar(&provider.declaration.language) == Some(provider)
    }

    /// Dynamic recognition claims this path even when ambiguity prevents choosing a language.
    pub(crate) fn handles_path(&self, path: &Path) -> bool {
        let registry = self.read();
        let file = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        registry.saved.associations.contains_key(&ext)
            || registry.recognizers.contains_key(&format!("file:{file}"))
            || registry.recognizers.contains_key(&format!("ext:{ext}"))
    }

    pub(crate) fn languages(&self) -> std::collections::BTreeSet<String> {
        let registry = self.read();
        registry
            .recognizers
            .values()
            .flatten()
            .map(|(_, definition)| definition.id.clone())
            .chain(registry.highlighters.keys().cloned())
            .collect()
    }
}
