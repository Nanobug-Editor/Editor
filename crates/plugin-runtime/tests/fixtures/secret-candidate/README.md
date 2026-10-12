# Unpublished candidate fixture

This independent SDK guest attempts a synthetic secret mutation from Snapshot when
an optional package policy requests it. It reports only error categories or presence,
never secret bytes. Tests exercise Snapshot failure and the existing public install
cancellation barrier, plus normal enable, recovery, settings replacement and reopen.

```powershell
& <host-exe> --plugin-package crates/plugin-runtime/tests/fixtures/secret-candidate --output target/cloud-api-test
cargo test -p plugin-runtime --test cloud t27_t30_unpublished -- --ignored --nocapture --test-threads=1
```
