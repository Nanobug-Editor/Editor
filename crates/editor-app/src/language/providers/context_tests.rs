//! Multiple declared source languages keep independent stable choices while sharing one provider key.
use super::*;

/// Adding an alias-capable provider does not silently replace the user's saved secondary-language choice.
#[test]
fn language_aliases_publish_candidates_and_preserve_explicit_selection() {
    let profile = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let providers = Providers::new(profile.path(), workspace.path());
    let declaration:plugin_runtime::plugin_protocol::language::Provider = serde_json::from_value(serde_json::json!({
        "id":"analysis","language":"novel-primary","languages":["novel-secondary"],"service":"analysis"
    })).unwrap();
    providers.refresh(
        profile.path(),
        Vec::new(),
        vec![("first".into(), vec![declaration.clone()])],
    );
    assert_eq!(
        providers.language_servers()["novel-primary"].as_deref(),
        Some("first/analysis")
    );
    assert_eq!(
        providers.language_servers()["novel-secondary"].as_deref(),
        Some("first/analysis")
    );
    providers
        .choose(Scope::User, "lsp:novel-secondary", Some("first/analysis"))
        .unwrap();
    providers.refresh(
        profile.path(),
        Vec::new(),
        vec![
            ("second".into(), vec![declaration.clone()]),
            ("first".into(), vec![declaration]),
        ],
    );
    assert_eq!(
        providers.language_servers()["novel-secondary"].as_deref(),
        Some("first/analysis")
    );
}
