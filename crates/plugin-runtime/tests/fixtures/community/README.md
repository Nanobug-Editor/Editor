# Community SDK consumers

These are test-only independent SDK consumers retained from community delivery
`901c0907f9a0734d30506b3557f4603bdf4cab24`. They exercise documents, commands,
workspace transactions, diagnostics, language services, trees, decorations and
cloud lifecycle through the public Package/Manager boundary. They are not
application plugins, installer payloads, exported SDK files or registry listings.
Official business sources remain in their independent repositories.

Each project uses the host-managed public SDK cache, without a path dependency or
SDK source copy. From the repository root, build a candidate directly with:

```powershell
cargo build -p editor-app
# Replace the project name with the consumer under test.
& ./target/debug/editor-app.exe --plugin-package ./crates/plugin-runtime/tests/fixtures/community/cloud-translation --output ./target/community-current --release
```

The adjacent [fixed Package ledger](../packages/inputs.json) selects exact archive
names and SHA-256 values; tests use `tests/support/packages.rs`. Older delivered
ZIPs retain their original bytes, independently of the merged SDK and these
relocated sources. The per-package build evidence remains in
[community verification](../../../../../docs/plugins/verification/plugin-api-community-foundation/README.md).
Do not describe an old ZIP as rebuilt with the current SDK. Replacing a current
fixture requires an actual public SDK build, Package validation and affected
behavior tests before updating its ledger.

The retired example is retained only as a fixed Package archive; source is not
restored. The capability consumer remains here solely for new SDK negotiation,
menu, async request and cleanup regressions. Native key/value server fixtures
are built separately with Cargo when their explicit ignored tests require them.
