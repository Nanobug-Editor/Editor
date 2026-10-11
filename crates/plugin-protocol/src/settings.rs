//! Stable `configuration`: plugin-owned values never change host authority or editor preferences.
//!
//! Protocol 7/base 1 and `configuration = ^1` retain boolean, bounded string/integer/enum,
//! user/project provenance and replacement application. New hot application and secret references
//! require `configuration = >=1.1,<2`; a range accepting 1.0 cannot declare these features.
//! A component using hot values opts into `settings_hook` and handles `Phase::Changed` with the
//! entire effective map. Older negotiated consumers receive only Validate/Apply.
//!
//! The host validates schema and isolated guest proposals before saving. A hot-only update keeps
//! the current owner/resources and sends Changed after durable save; a restart value replaces the
//! affected owner through the existing recovery transaction. Cold instances remain cold. A failure
//! applying an already saved hot update is explicit and retires the failed owner; WASM memory or
//! external service side effects cannot be rolled back. Never silently retry a failed apply.
//!
//! SecretReference stores only an own-namespace key, never a token. Resolve it through storage.secrets
//! after separately granting secrets; project overrides cannot grant that permission, select a
//! foreign namespace or overwrite host locale, theme or trust. A blank reference is unconfigured.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Settings default to user scope; a package must explicitly allow a project override.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    #[default]
    User,
    Project,
}

/// Application is declared per setting; mixed changes replace only the affected plugin instance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Apply {
    #[default]
    RestartInstance,
    /// configuration 1.1: deliver Changed after validation/save, preserving the running owner.
    HotApply,
}

/// A small native form vocabulary keeps validation independent of any particular plugin.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SettingType {
    Boolean,
    String {
        #[serde(default = "default_string_limit")]
        max_length: usize,
    },
    Integer {
        min: i64,
        max: i64,
    },
    Enum {
        choices: Vec<String>,
    },
    /// configuration 1.1: own storage.secrets key, 0–128 UTF-8 bytes without controls.
    /// The value is a reference, never credential bytes; an empty value means not configured.
    SecretReference,
}
fn default_string_limit() -> usize {
    4096
}

/// Defaults, scope and application behavior are declarations checked before installation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub title: String,
    pub value_type: SettingType,
    pub default: Value,
    #[serde(default)]
    pub scope: Scope,
    #[serde(default)]
    pub apply: Apply,
}
impl Definition {
    /// Bound schema-driven UI allocations independently of the package archive budget.
    pub fn valid(&self) -> bool {
        !self.title.is_empty()
            && self.title.len() <= 256
            && self.accepts(&self.default)
            && match &self.value_type {
                SettingType::Boolean => true,
                SettingType::SecretReference => true,
                SettingType::String { max_length } => *max_length <= 4096,
                SettingType::Integer { min, max } => min <= max,
                SettingType::Enum { choices } => {
                    !choices.is_empty()
                        && choices.len() <= 32
                        && choices
                            .iter()
                            .all(|choice| !choice.is_empty() && choice.len() <= 128)
                        && choices
                            .iter()
                            .collect::<std::collections::BTreeSet<_>>()
                            .len()
                            == choices.len()
                }
            }
    }
    /// Explicit invalid values must fail, rather than falling through to discovered or default values.
    pub fn accepts(&self, value: &Value) -> bool {
        match &self.value_type {
            SettingType::Boolean => value.is_boolean(),
            SettingType::SecretReference => value
                .as_str()
                .is_some_and(|key| key.len() <= 128 && !key.chars().any(char::is_control)),
            SettingType::String { max_length } => {
                value.as_str().is_some_and(|text| text.len() <= *max_length)
            }
            SettingType::Integer { min, max } => value
                .as_i64()
                .is_some_and(|number| number >= *min && number <= *max),
            SettingType::Enum { choices } => value
                .as_str()
                .is_some_and(|text| choices.iter().any(|choice| choice == text)),
        }
    }

    /// New schema semantics must explicitly exclude configuration 1.0 during package negotiation.
    pub fn requires_configuration_1_1(&self) -> bool {
        self.apply == Apply::HotApply || matches!(self.value_type, SettingType::SecretReference)
    }
}

/// Provenance accompanies effective values so the UI never confuses an override with a default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Default,
    Discovered,
    User,
    Project,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectiveValue {
    pub value: Value,
    pub source: Source,
}
pub type Effective = BTreeMap<String, EffectiveValue>;

/// Each notification carries all resolved values, including provenance; validation is side-effect free.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Validate,
    /// Initial/replacement preparation supplies effective values before business activation.
    Apply,
    /// configuration 1.1: a validated, durably saved hot update for the same running owner.
    Changed,
}

/// A hook may discover only unspecified values and report errors against explicit settings.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    #[serde(default)]
    pub discovered: BTreeMap<String, Value>,
    #[serde(default)]
    pub errors: BTreeMap<String, String>,
}
