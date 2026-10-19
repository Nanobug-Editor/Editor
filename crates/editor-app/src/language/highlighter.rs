//! Plugin-only Tree-sitter parsing and capture resolution through Base's parser-independent seam.
//! The registry stores immutable grammar resources; document text and history remain in EditorState.
mod registry;
mod source;
mod syntax;
mod theme;
pub(crate) use registry::{GrammarConfig, LanguageParserFactory, LanguageRegistry};
pub(crate) use syntax::SyntaxHighlighter;
pub(crate) use theme::HighlightTheme;
