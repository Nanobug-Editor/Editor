//! `language.content 1.0` opens provider-owned library definitions as readonly native resources.
//! No URI is fetched by the host filesystem or network. The declared live LSP connection returns
//! `{ "text": "..." }` for `{ "uri": "..." }`; lifecycle and editing methods remain host-owned.
use serde::{Deserialize, Serialize};

/// A provider explicitly opts into content requests for bounded non-file URI schemes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    /// Lowercase URI schemes, excluding file and the host's opaque virtual-resource scheme.
    pub schemes: Vec<String>,
    /// `workspace/textDocumentContent` or a custom non-lifecycle, non-editing request method.
    pub method: String,
    /// Recognized language used only for highlighting the readonly native document.
    pub language: String,
}

impl Provider {
    /// Validate declaration shape before installation; this grants no process or IO authority.
    pub fn valid(&self) -> bool {
        !self.schemes.is_empty()
            && self.schemes.len() <= 16
            && self.schemes.iter().all(|scheme| {
                !scheme.is_empty()
                    && scheme.len() <= 32
                    && scheme.as_bytes()[0].is_ascii_lowercase()
                    && scheme.bytes().all(|b| {
                        b.is_ascii_lowercase() || b.is_ascii_digit() || b"+.-".contains(&b)
                    })
                    && !matches!(scheme.as_str(), "file" | "nanobug-virtual")
            })
            && self
                .schemes
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == self.schemes.len()
            && (self.method == "workspace/textDocumentContent"
                || crate::language::valid_control_method(&self.method))
            && !self.language.is_empty()
            && self.language.len() <= 100
            && self
                .language
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
    }

    /// Match an exact declared scheme; URI text remains opaque provider data and grants no IO.
    pub fn accepts(&self, uri: &str) -> bool {
        self.valid()
            && uri.len() <= 8192
            && !uri.chars().any(char::is_control)
            && uri.split_once(':').is_some_and(|(scheme, rest)| {
                !rest.is_empty() && self.schemes.iter().any(|s| s == scheme)
            })
    }
}

/// Complete UTF-8 content. The native consumer rejects replies exceeding 1 MiB before opening.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Content {
    /// Complete provider text; it never becomes a writable workspace file.
    pub text: String,
}

/// Original definition identity and strict coordinates, independent of a native tab's opaque URI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    /// Exact provider identity retained independently of the native virtual-resource URI.
    pub uri: String,
    /// Original strict target coordinates, or no initial selection when the provider omits one.
    pub selection: Option<crate::api::DocumentRange>,
}
