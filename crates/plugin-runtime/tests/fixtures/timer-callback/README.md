# Timer callback fixture

This independent SDK guest arms two due timers. A's callback resets B by 300 ms,
and both publish observable receipts through the ordinary native view contract.
The fixture uses no network, credentials, private files or test-only host API.

Build with the current host, then explicitly run the ignored regression:

```powershell
& ../target06/debug/editor-app.exe --plugin-package crates/plugin-runtime/tests/fixtures/timer-callback --output target/cloud-api-test
cargo test -p plugin-runtime --test cloud t29_timer_callback_reset -- --ignored --nocapture --test-threads=1
```
