//! `language.lsp 1.6` registers provider-owned project choices without granting path authority.
//!
//! Registration requires `editor.read` and `workspace.read` in an activated workspace. The host
//! canonicalizes roots, seals each reference to its owner/scope/configuration, and revokes old
//! generations. Tokens must not be persisted: save a plugin's logical key in private preferences
//! and validate it against fresh candidates after reopening. `Control` uses only an owned declared
//! provider and its approved startup plan; source text continues through the native document stream.
use crate::{api, language_control};
use serde::{Deserialize, Serialize};

/// Informational plugin-owned configuration identity; roots remain within the authorized workspace.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Descriptor {
    /// Package-local declared provider that owns the approved native analysis plan.
    pub provider: String,
    /// Stable logical project identity. It is not a path or a transferable resource capability.
    pub key: String,
    /// Positive monotonic configuration generation; replacing it revokes the older registration.
    pub generation: u64,
    /// Empty denotes the workspace root. This path is never itself a resource handle.
    pub project_root: String,
    /// At most 32 KiB under language.lsp 1.9 (4 KiB before it), interpreted by the readonly hook.
    #[serde(default)]
    pub data: serde_json::Value,
}

impl Descriptor {
    /// Validate portable shape before the native host checks ownership and canonical containment.
    pub fn valid(&self) -> bool {
        crate::language::ProjectContext {
            key: self.key.clone(),
            generation: self.generation,
        }
        .valid()
            && self.generation > 0
            && !self.provider.is_empty()
            && self.provider.len() <= 100
            && self.provider.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
            })
            && (self.project_root.is_empty()
                || crate::api::ResourceIdentity::Local {
                    path: self.project_root.clone(),
                }
                .validate()
                .is_ok())
            && serde_json::to_vec(&self.data).is_ok_and(|bytes| bytes.len() <= 32768)
    }
}

/// A live opaque reference cannot be manufactured from its descriptive root, key or generation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    /// Host-issued live owner/scope token. Never manufacture it or persist it as a preference.
    pub token: String,
    /// Readonly metadata for presenting and matching freshly discovered project choices.
    pub descriptor: Descriptor,
}

/// Registration is atomic and bounded to 128 owned contexts; control uses the existing request gate.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Register {
        contexts: Vec<Descriptor>,
    },
    /// language.lsp 1.8: retain the sealed original caller chain as well as the provider lifetime.
    /// Completing the service call preserves the context; retiring any participant revokes it.
    RegisterBorrowed {
        contexts: Vec<Descriptor>,
    },
    /// language.lsp 1.9: retire this instance's language discovery inputs and borrowed contexts.
    /// Subsequent discovery uses a new epoch; completing an older call cannot revive old plans.
    Invalidate,
    /// language.lsp 1.9: check an owned metadata token's lifetime without reading files or text.
    Check {
        context: String,
    },
    Describe {
        context: String,
    },
    Control {
        context: String,
        request: language_control::Request,
        timeout_ms: u32,
    },
    /// Refresh only the exact original source after a native user selected a current owned context.
    Refresh {
        context: String,
        provider: String,
        document: api::DocumentVersion,
    },
}

#[cfg(feature = "guest")]
/// Register freshly validated domain candidates; no process is started by this synchronous call.
pub fn register(contexts: Vec<Descriptor>) -> Result<Vec<Reference>, api::Failure> {
    match api::guest::request(api::Operation::LanguageContexts {
        operation: Operation::Register { contexts },
    })? {
        api::Value::LanguageContexts(references) => Ok(references),
        _ => Err(api::Failure::new(
            api::ErrorCode::InvalidState,
            "Unexpected context registration result",
        )),
    }
}

#[cfg(feature = "guest")]
/// Register caller-scoped project metadata under language.lsp 1.8 and the existing read grants.
/// The host supplies the lifetime chain; descriptors cannot name or revive another caller.
/// Returns owned references, or a typed capability, permission, quota or stale-generation failure.
pub fn register_borrowed(contexts: Vec<Descriptor>) -> Result<Vec<Reference>, api::Failure> {
    match api::guest::request(api::Operation::LanguageContexts {
        operation: Operation::RegisterBorrowed { contexts },
    })? {
        api::Value::LanguageContexts(references) => Ok(references),
        _ => Err(api::Failure::new(
            api::ErrorCode::InvalidState,
            "Unexpected borrowed context registration result",
        )),
    }
}

#[cfg(feature = "guest")]
/// Invalidate language discovery after domain inputs change, without changing user settings.
/// Requires language.lsp 1.9 and the existing workspace read grants; returns a typed gate failure.
pub fn invalidate() -> Result<(), api::Failure> {
    match api::guest::request(api::Operation::LanguageContexts {
        operation: Operation::Invalidate,
    })? {
        api::Value::Unit => Ok(()),
        _ => Err(api::Failure::new(
            api::ErrorCode::InvalidState,
            "Unexpected language invalidation result",
        )),
    }
}

#[cfg(feature = "guest")]
/// Check only ownership, scope and metadata lifetime. This grants no file or editor authority and
/// can be used by a process-only service before consuming metadata already held in its own memory.
/// Requires language.lsp 1.9; foreign, replaced or retired tokens return InvalidHandle.
pub fn check(context: impl Into<String>) -> Result<(), api::Failure> {
    match api::guest::request(api::Operation::LanguageContexts {
        operation: Operation::Check {
            context: context.into(),
        },
    })? {
        api::Value::Unit => Ok(()),
        _ => Err(api::Failure::new(
            api::ErrorCode::InvalidState,
            "Unexpected context lifetime result",
        )),
    }
}

#[cfg(feature = "guest")]
/// Describe only this instance's current reference. Foreign, expired or replaced tokens fail.
pub fn describe(context: impl Into<String>) -> Result<Reference, api::Failure> {
    match api::guest::request(api::Operation::LanguageContexts {
        operation: Operation::Describe {
            context: context.into(),
        },
    })? {
        api::Value::LanguageContexts(mut references) if references.len() == 1 => {
            Ok(references.remove(0))
        }
        _ => Err(api::Failure::new(
            api::ErrorCode::InvalidState,
            "Unexpected context description result",
        )),
    }
}

#[cfg(feature = "guest")]
/// Analyze or publish a binding for the original source through the explicitly selected context.
pub fn control(
    context: impl Into<String>,
    request: language_control::Request,
    timeout_ms: u32,
) -> Result<api::guest::EditorTask, api::Failure> {
    match api::guest::request(api::Operation::LanguageContexts {
        operation: Operation::Control {
            context: context.into(),
            request,
            timeout_ms,
        },
    })? {
        api::Value::Accepted(handle) => Ok(api::guest::EditorTask::from_accepted(handle)),
        _ => Err(api::Failure::new(
            api::ErrorCode::InvalidState,
            "Unexpected context control acceptance",
        )),
    }
}
