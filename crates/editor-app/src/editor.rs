//! Document lifecycle and editor-area presentation.

pub(crate) mod comparison;
pub(crate) mod decorations;
#[cfg(test)]
mod definition_tests;
pub(crate) mod diagnostics;
mod documents;
#[cfg(test)]
mod empty_canvas_tests;
pub(crate) mod file_watch;
#[cfg(test)]
mod file_watch_tests;
#[cfg(test)]
mod hover_hit_test;
pub(crate) mod language_edits;
pub(crate) mod language_features;
pub(crate) mod linked_input;
pub(crate) mod native_text;
mod pointer_hover;
mod resources;
mod source;
pub(crate) mod tabs;
mod text_session;
pub(crate) mod text_transactions;
pub(crate) use popovers::{CompletionPopupState, DefinitionPopupFocus};
mod popovers;
pub(crate) use text_drag::TextDragState;
pub(crate) use text_drag::caret_offset_at;
mod text_drag;
mod view;
pub(crate) mod viewport;

pub(crate) use documents::{attach_language_server, detach_language_server};

/// Legacy single-project fixture calls resolve through the same project owner as production actions.
#[cfg(test)]
pub(crate) fn language_for_path(path: &std::path::Path) -> String {
    crate::language::providers::legacy()
        .language_for_path(path)
        .unwrap_or_else(|| "text".into())
}
