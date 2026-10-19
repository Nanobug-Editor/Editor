//! Local syntax and diagnostic colors implementing Base's semantic capture resolver.
use gpui_base::input::HighlightStyleResolver;
use gpui_kit::{App, FontStyle, FontWeight, HighlightStyle, Hsla, rgb};
use std::sync::Arc;

/// Syntax styles are resolved at paint time, so existing parser snapshots survive a theme change.
#[derive(Clone)]
pub(crate) struct HighlightTheme {
    /// Editor background, gutter and semantic capture presentation.
    pub style: EditorStyle,
}
/// Product-owned editor styling without any component-library theme types.
#[derive(Clone)]
pub(crate) struct EditorStyle {
    /// Background used for source and ghost-text erasure.
    pub editor_background: Option<Hsla>,
    /// Gutter fill, including alpha from the active product theme.
    pub editor_gutter_background: Option<Hsla>,
    /// Invisible-character paint color.
    pub editor_invisible: Option<Hsla>,
    /// Optional active-line wash.
    pub editor_active_line: Option<Hsla>,
    /// Semantic syntax color resolver.
    pub syntax: SyntaxColors,
    /// Stable diagnostic severity palette.
    pub status: StatusColors,
}
/// Capture colors and typography, with dotted names inheriting their nearest known parent.
#[derive(Clone)]
pub(crate) struct SyntaxColors {
    dark: bool,
}
impl SyntaxColors {
    /// Map a query capture name to a local style; unknown names remain unstyled.
    pub(crate) fn style(&self, name: &str) -> Option<HighlightStyle> {
        let root = name.split('.').next()?;
        let color = match root {
            "comment" => {
                if self.dark {
                    0x6a9955
                } else {
                    0x008000
                }
            }
            "keyword" | "tag" | "boolean" | "operator" | "preproc" | "primary" => {
                if self.dark {
                    0x569cd6
                } else {
                    0x0000ff
                }
            }
            "string" | "link_uri" => {
                if self.dark {
                    0xce9178
                } else {
                    0xa31515
                }
            }
            "number" | "constant" => {
                if self.dark {
                    0xb5cea8
                } else {
                    0x098658
                }
            }
            "function" | "constructor" => {
                if self.dark {
                    0xdcdcaa
                } else {
                    0x795e26
                }
            }
            "type" | "enum" | "variant" => {
                if self.dark {
                    0x4ec9b0
                } else {
                    0x267f99
                }
            }
            "attribute" | "property" | "variable" | "label" => {
                if self.dark {
                    0x9cdcfe
                } else {
                    0x001080
                }
            }
            "punctuation" | "embedded" | "title" | "emphasis" | "link_text" => {
                if self.dark {
                    0xd4d4d4
                } else {
                    0x242424
                }
            }
            "hint" | "predictive" => {
                if self.dark {
                    0xa1a1aa
                } else {
                    0x71717a
                }
            }
            "text" if name == "text.code.span" || name == "text.literal" => {
                if self.dark {
                    0xce9178
                } else {
                    0xa31515
                }
            }
            _ => return None,
        };
        Some(HighlightStyle {
            color: Some(rgb(color).into()),
            font_style: (name == "emphasis").then_some(FontStyle::Italic),
            font_weight: (name == "emphasis.strong" || root == "title").then_some(FontWeight::BOLD),
            ..Default::default()
        })
    }
}
/// Diagnostic and decoration colors retain the same meaning in all native editor mounts.
#[derive(Clone)]
pub(crate) struct StatusColors;
impl StatusColors {
    /// Error squiggles and error decorations.
    pub(crate) fn error(&self, _: &App) -> Hsla {
        rgb(0xef4444).into()
    }
    /// Warning squiggles and warning decorations.
    pub(crate) fn warning(&self, _: &App) -> Hsla {
        rgb(0xeab308).into()
    }
    /// Informational diagnostic color.
    pub(crate) fn info(&self, _: &App) -> Hsla {
        rgb(0x3b82f6).into()
    }
    /// Subdued diagnostic hints.
    pub(crate) fn hint(&self, _: &App) -> Hsla {
        rgb(0x71717a).into()
    }
    /// Successful operation markers.
    pub(crate) fn success(&self, _: &App) -> Hsla {
        rgb(0x22c55e).into()
    }
}
impl HighlightTheme {
    /// Resolve the dark baseline before user theme background projection.
    pub(crate) fn default_dark() -> Arc<Self> {
        Self::new(true)
    }
    /// Resolve the light baseline before user theme background projection.
    pub(crate) fn default_light() -> Arc<Self> {
        Self::new(false)
    }
    /// Construct immutable colors with a transparent active-line wash.
    fn new(dark: bool) -> Arc<Self> {
        let background = Some(rgb(if dark { 0x18181b } else { 0xffffff }).into());
        Arc::new(Self {
            style: EditorStyle {
                editor_background: background,
                editor_gutter_background: background,
                editor_invisible: Some(rgb(if dark { 0x52525b } else { 0xd4d4d8 }).into()),
                editor_active_line: Some(
                    Hsla::from(rgb(if dark { 0xffffff } else { 0x000000 })).opacity(0.035),
                ),
                syntax: SyntaxColors { dark },
                status: StatusColors,
            },
        })
    }
}
impl HighlightStyleResolver for HighlightTheme {
    fn style(&self, name: &str) -> Option<HighlightStyle> {
        self.style.syntax.style(name)
    }
}
