//! Minimal language-service consumer for host SDK completion, hover and definition regression tests.
use plugin_protocol::ui::Action;

/// Keep one real protocol reference; tests submit unsaved variants without changing this source file.
pub fn retain_action(action: Action) -> Action {
    action
}
