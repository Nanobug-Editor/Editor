//! Interactive language preparation admits its fixed private-preference profile only.
use plugin_protocol::{language_control, service};
use serde_json::json;

/// The native preparation profile must pass the same declaration validation as a packaged guest.
/// Ordinary delegated methods retain no right to borrow a provider's private preference namespace.
#[test]
fn only_the_canonical_interactive_prepare_profile_declares_private_preferences() {
    let canonical = language_control::binding_prepare_method_for(true);
    let declarations = |method: &service::Method| {
        serde_json::from_value::<service::Declarations>(json!({
            "provides": {"generic.language": {"version":"1.0.0", "methods":{"prepare":method}}}
        }))
        .unwrap()
    };
    assert!(
        declarations(&canonical).validate().is_ok(),
        "the public interactive profile must be independently packageable"
    );

    let mut different = canonical.clone();
    different.permissions.insert("editor.write".into());
    assert!(declarations(&different).validate().is_err());
    different = canonical.clone();
    different.result = service::Schema::Boolean;
    assert!(declarations(&different).validate().is_err());

    // The original noninteractive preparation profile still needs no private-data permission.
    assert!(
        declarations(&language_control::binding_prepare_method())
            .validate()
            .is_ok()
    );
}
