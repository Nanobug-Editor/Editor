//! Public package admission must negotiate diagnostics independently from language servers.

use plugin_runtime::Package;
use std::collections::BTreeMap;

/// Independent checkers can declare stable diagnostics without a native process or LSP grant.
#[test]
fn independent_checkers_negotiate_diagnostics_without_lsp() {
    for id in ["independent-spelling", "independent-configuration"] {
        let manifest = serde_json::json!({
            "id": id, "name": id, "version": "0.1.0", "protocol": 7,
            "api": {"base": "^1", "required": {"editor.diagnostics": "^1"}},
            "component": "checker.wasm",
            "contributions": "plugin.toml", "permissions": ["editor.read", "editor.diagnostics"],
            "storage_limit": 1024
        });
        let resources = BTreeMap::from([
            ("manifest.json".into(), serde_json::to_vec(&manifest).unwrap()),
            // Admission only inspects the component envelope; execution fixtures use real guests.
            ("checker.wasm".into(), b"\0asm\x0d\0\x01\0".to_vec()),
            ("plugin.toml".into(), format!("[plugin]\nid = \"{id}\"\nname = \"{id}\"\nversion = \"0.1.0\"\nhost_version = \"^0.1\"\n").into_bytes()),
        ]);
        let package = Package::from_files(resources).unwrap();
        assert!(package.manifest.language_servers.is_empty());
        assert!(package.manifest.services.is_empty());
    }
}
