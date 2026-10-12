# Temporary owner lifecycle fixture

This independent guest uses the public SDK and production Manager. Its logical
snapshot selects a rejected Prepare, Validate or Apply phase; the prior
instance's temporary handles must continue to read and write their files.
An optional immutable `snapshot-policy.json` attempts temporary access after
candidate activation and staged-file flush, before durable publication.
Receipts expose only operation results, never native paths or secret bytes.

Build from the repository using the host's `--plugin-package` entry point, then
explicitly run the ignored `temporary_ownership` module in the `cloud` test:

```powershell
& <host-exe> --plugin-package crates/plugin-runtime/tests/fixtures/temporary-candidate crates/plugin-runtime/tests/fixtures/community/cloud-translation crates/plugin-runtime/tests/fixtures/community/code-host-query --output target/cloud-api-test
cargo test -p plugin-runtime --test cloud temporary_ownership -- --ignored --nocapture --test-threads=1
```

Tests consume the exact version and SHA-256 in `../packages/inputs.json`. A rebuilt
output does not replace that input automatically; verify the affected public SDK
and lifecycle behavior before updating its archive and ledger.

The same public probes verify reserved Data aliases, ordinary Cache/Workspace
names and editor-navigation consent before native path lookup. No host-only
fixture API or repository-path SDK dependency is used.
