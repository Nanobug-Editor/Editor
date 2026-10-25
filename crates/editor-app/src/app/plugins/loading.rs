//! Plugin resources and external engines project independent live activity from the existing readiness sources.
use super::*;
use crate::app::language_servers::ServiceLoadState;

impl EditorApp {
    /// Group package restoration and contributed grammar loading by plugin; native startup is separate.
    pub(super) fn plugin_loading_groups(&self, cx: &App) -> BTreeMap<String, PluginLoadingSummary> {
        let panel = self.extensions.read(cx);
        let mut groups = BTreeMap::<String, PluginLoadingSummary>::new();
        let mut add = |plugin: &str, fallback: &str, detail: String| {
            let name = panel
                .entries
                .iter()
                .find(|entry| entry.manifest.id == plugin)
                .map_or_else(|| fallback.to_owned(), |entry| entry.manifest.name.clone());
            groups
                .entry(plugin.to_owned())
                .or_insert_with(|| PluginLoadingSummary {
                    plugin: Some(plugin.to_owned()),
                    name,
                    details: Vec::new(),
                })
                .details
                .push(detail);
        };
        for (id, name) in &panel.startup {
            add(id, name, t!("plugins.status_starting").to_string());
        }
        for (provider, state) in &self.dynamic_languages.entries {
            if matches!(state, Ok(false)) {
                add(
                    &provider.owner,
                    &provider.owner,
                    t!(
                        "plugins.status_highlight_loading",
                        language = &provider.declaration.language
                    )
                    .to_string(),
                );
            }
        }
        groups
    }

    /// List each declared external engine once while any of its current language/context startups is pending.
    /// Ownership comes from a bound plan or selected declaration, never from translated messages or special IDs.
    pub(super) fn external_service_loading_groups(
        &self,
        cx: &App,
    ) -> BTreeMap<String, PluginLoadingSummary> {
        let panel = self.extensions.read(cx);
        let available = panel.language_services();
        let selected = self.providers.language_servers();
        let mut groups = BTreeMap::<String, PluginLoadingSummary>::new();
        for (language, state) in &self.language_service_states {
            if *state != ServiceLoadState::Loading {
                continue;
            }
            let context = self.language_context_status_owner(language);
            let contextual = context.is_some();
            let owned = context.or_else(|| {
                self.language_servers
                    .get(language)
                    .and_then(|server| {
                        available.iter().find_map(|(key, plan)| {
                            plan.as_ref()
                                .ok()
                                .filter(|plan| server.uses_service(plan))
                                .map(|plan| (plan.owner.clone(), key.clone()))
                        })
                    })
                    .or_else(|| {
                        selected
                            .get(language)
                            .and_then(|key| key.as_ref())
                            .and_then(|key| {
                                panel
                                    .entries
                                    .iter()
                                    .find(|entry| {
                                        entry.manifest.language_servers.iter().any(|provider| {
                                            format!("{}/{}", entry.manifest.id, provider.id) == *key
                                        })
                                    })
                                    .map(|entry| (entry.manifest.id.clone(), key.clone()))
                            })
                    })
            });
            let (plugin, service) = owned.map_or((None, language.clone()), |(plugin, service)| {
                (Some(plugin), service)
            });
            let group = groups.entry(service.clone()).or_insert_with(|| {
                let details = plugin
                    .as_ref()
                    .map(|id| {
                        let name = panel
                            .entries
                            .iter()
                            .find(|entry| entry.manifest.id == *id)
                            .map_or(id.as_str(), |entry| entry.manifest.name.as_str());
                        t!("services.status_owner", plugin = name).to_string()
                    })
                    .into_iter()
                    .collect();
                PluginLoadingSummary {
                    plugin,
                    name: service.clone(),
                    details,
                }
            });
            let detail = if contextual {
                t!("services.status_context_starting").to_string()
            } else {
                t!(
                    "plugins.status_service_loading",
                    language = language,
                    service = service
                )
                .to_string()
            };
            if !group.details.contains(&detail) {
                group.details.push(detail);
            }
        }
        groups
    }
}
