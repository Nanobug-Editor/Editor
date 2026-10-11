//! One selected role cache slot is prepared on a worker and published before its waiter becomes ready.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Native,
    Direct,
    Structure,
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    provider: String,
    kind: Kind,
}
impl Key {
    /// LSP/formatter and individual direct features share their declared provider's existing executor.
    fn from_reason(reason: &DemandReason) -> Option<Self> {
        let DemandReason::Language { provider, role } = reason else {
            return None;
        };
        let kind = match role {
            LanguageRole::Lsp | LanguageRole::Formatter => Kind::Native,
            LanguageRole::Direct(_) => Kind::Direct,
            LanguageRole::Structure => Kind::Structure,
        };
        Some(Self {
            provider: provider.clone(),
            kind,
        })
    }
}
pub(super) enum Ready {
    Native(Arc<crate::LanguageService>),
    Direct(Arc<crate::DirectProvider>),
    Structure(Arc<crate::StructureProvider>),
}
pub(super) struct PreparedRole {
    key: Key,
    fingerprint: language::Fingerprint,
    value: Result<Ready, api::Failure>,
}
impl DemandPreparation {
    /// Successful selected roles and actual command/view/service/background intent may activate business.
    /// A cancelled successful role cannot lend activation authority to another role that failed.
    pub(super) fn business_is_demanded(&self, roles: &[PreparedRole]) -> bool {
        self.waiters
            .lock()
            .unwrap()
            .iter()
            .filter(|waiter| !waiter.completion.status().is_terminal())
            .any(|waiter| {
                Key::from_reason(&waiter.reason).map_or(true, |key| {
                    roles
                        .iter()
                        .any(|role| role.key == key && role.value.is_ok())
                })
            })
    }

    /// A discarded cold candidate has no published Discovered values or authority. Its errors belong
    /// to the original saved/default input; successful cancelled roles cannot publish its dead owner.
    pub(super) fn retain_unactivated_failures(&self, roles: &mut Vec<PreparedRole>) {
        let fingerprint = language::Fingerprint::new(
            &self.entry,
            &self.environment.workspace,
            &Ok(self.configuration.clone()),
            &self.host_resources.sdk,
        );
        roles.retain_mut(|role| {
            if role.value.is_ok() {
                return false;
            }
            role.fingerprint = fingerprint.clone();
            true
        });
    }

    /// Prepare only the roles actually waiting at this seam, coalescing features of the same provider.
    /// Late roles retain their original completion and join a subsequent bounded preparation.
    pub(super) fn prepare_roles(
        &self,
        code: Option<language_preparation::ComponentCode<'_>>,
        mut owner: Option<&mut Instance>,
        authority: Option<crate::EditorAuthority>,
    ) -> Vec<PreparedRole> {
        let requested = self
            .waiters
            .lock()
            .unwrap()
            .iter()
            .filter(|waiter| !waiter.completion.status().is_terminal())
            .filter_map(|waiter| Key::from_reason(&waiter.reason))
            .collect::<BTreeSet<_>>();
        let settings = owner.as_ref().map_or_else(
            || self.configuration.clone(),
            |instance| instance.configuration.clone(),
        );
        let fingerprint = language::Fingerprint::new(
            &self.entry,
            &self.environment.workspace,
            &Ok(settings.clone()),
            &self.host_resources.sdk,
        );
        requested
            .into_iter()
            .map(|key| {
                let result = (|| -> anyhow::Result<Ready> {
                    self.check()?;
                    let id = key
                        .provider
                        .strip_prefix(&format!("{}/", self.entry.manifest.id))
                        .ok_or_else(|| {
                            anyhow::anyhow!("Provider does not belong to demanded package")
                        })?;
                    let ready = match key.kind {
                        Kind::Native => {
                            let declaration = self
                                .entry
                                .manifest
                                .language_servers
                                .iter()
                                .find(|item| item.id == id)
                                .unwrap()
                                .clone();
                            Ready::Native(Arc::new(self.prepare_language_service(
                                code,
                                owner.as_deref_mut(),
                                authority.clone(),
                                &self.entry,
                                declaration,
                                settings.clone(),
                            )?))
                        }
                        Kind::Direct => {
                            let declaration = self
                                .entry
                                .manifest
                                .direct_language_providers
                                .iter()
                                .find(|item| item.id == id)
                                .unwrap()
                                .clone();
                            Ready::Direct(Arc::new(self.prepare_direct(
                                code.ok_or_else(|| {
                                    api::Failure::new(
                                        api::ErrorCode::CapabilityUnavailable,
                                        "Direct executor needs WASM",
                                    )
                                })?,
                                authority.clone().ok_or_else(|| {
                                    anyhow::anyhow!("Direct owner is unavailable")
                                })?,
                                &self.entry,
                                declaration,
                                settings.clone(),
                            )?))
                        }
                        Kind::Structure => {
                            let declaration = self
                                .entry
                                .manifest
                                .structure_providers
                                .iter()
                                .find(|item| item.id == id)
                                .unwrap()
                                .clone();
                            Ready::Structure(Arc::new(self.prepare_structure(
                                code.ok_or_else(|| {
                                    api::Failure::new(
                                        api::ErrorCode::CapabilityUnavailable,
                                        "Structure executor needs WASM",
                                    )
                                })?,
                                &self.entry,
                                declaration,
                                settings.clone(),
                            )?))
                        }
                    };
                    self.check()?;
                    Ok(ready)
                })()
                .map_err(lease::failure);
                PreparedRole {
                    key,
                    fingerprint: fingerprint.clone(),
                    value: result,
                }
            })
            .collect()
    }
}
impl Manager {
    /// Metadata fingerprints prune only stale existing leases; they do not prepare any role.
    pub(in crate::manager) fn role_fingerprints(
        &mut self,
    ) -> BTreeMap<String, language::Fingerprint> {
        let entries = self
            .availabilities()
            .into_keys()
            .filter_map(|id| self.installed.get(&id))
            .collect::<Vec<_>>();
        let mut result = BTreeMap::new();
        for entry in entries {
            let values = self
                .effective_settings(&entry.manifest.id)
                .map_err(|error| format!("{error:#}"));
            let fingerprint = self.language_fingerprint(entry, &values);
            for id in entry
                .manifest
                .language_servers
                .iter()
                .map(|item| &item.id)
                .chain(
                    entry
                        .manifest
                        .direct_language_providers
                        .iter()
                        .map(|item| &item.id),
                )
                .chain(
                    entry
                        .manifest
                        .structure_providers
                        .iter()
                        .map(|item| &item.id),
                )
            {
                result.insert(format!("{}/{}", entry.manifest.id, id), fingerprint.clone());
            }
        }
        result
    }
    pub(super) fn demanded_role_result(
        &self,
        reason: &DemandReason,
    ) -> Option<Result<(), api::Failure>> {
        let key = Key::from_reason(reason)?;
        let fingerprint = self
            .installed
            .get(key.provider.split('/').next().unwrap())
            .map(|entry| {
                self.language_fingerprint(
                    entry,
                    &self
                        .effective_settings(&entry.manifest.id)
                        .map_err(|error| format!("{error:#}")),
                )
            })?;
        let result = match key.kind {
            Kind::Native => self
                .language_services
                .get(&key.provider)
                .filter(|item| item.fingerprint == fingerprint)
                .map(|item| {
                    item.service
                        .as_ref()
                        .map(|_| ())
                        .map_err(|error| error.clone())
                }),
            Kind::Direct => self
                .direct_language_providers
                .get(&key.provider)
                .filter(|item| item.fingerprint == fingerprint)
                .map(|item| {
                    item.provider
                        .as_ref()
                        .map(|_| ())
                        .map_err(|error| error.clone())
                }),
            Kind::Structure => self
                .structure_providers
                .get(&key.provider)
                .filter(|item| item.fingerprint == fingerprint)
                .map(|item| {
                    item.provider
                        .as_ref()
                        .map(|_| ())
                        .map_err(|error| error.clone())
                }),
        };
        result.map(|result| {
            result.map_err(|error| api::Failure::new(api::ErrorCode::OperationFailed, error))
        })
    }
    pub(super) fn publish_demanded_roles(&mut self, prepared: Vec<PreparedRole>) {
        for role in prepared {
            let error = role.value.as_ref().err().map(ToString::to_string);
            match (role.key.kind, role.value) {
                (Kind::Native, value) => {
                    let service = value
                        .map(|ready| {
                            let Ready::Native(service) = ready else {
                                unreachable!()
                            };
                            service
                        })
                        .map_err(|_| error.unwrap());
                    self.publish_language_plan(role.key.provider, role.fingerprint, service);
                }
                (Kind::Direct, value) => {
                    let provider = value
                        .map(|ready| {
                            let Ready::Direct(provider) = ready else {
                                unreachable!()
                            };
                            provider
                        })
                        .map_err(|_| error.unwrap());
                    self.direct_language_providers.insert(
                        role.key.provider,
                        direct::Prepared {
                            fingerprint: role.fingerprint,
                            provider,
                        },
                    );
                }
                (Kind::Structure, value) => {
                    let provider = value
                        .map(|ready| {
                            let Ready::Structure(provider) = ready else {
                                unreachable!()
                            };
                            provider
                        })
                        .map_err(|_| error.unwrap());
                    self.structure_providers.insert(
                        role.key.provider,
                        structure::Prepared {
                            fingerprint: role.fingerprint,
                            provider,
                        },
                    );
                }
            }
        }
    }
}
