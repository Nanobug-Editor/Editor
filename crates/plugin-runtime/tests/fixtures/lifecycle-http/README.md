# HTTP lifecycle SDK consumer

Test-only independent WASM package. Build with the host `--plugin-package` entry.
The `query` command exercises HttpTask against the controlled test endpoint on
127.0.0.1:18927; `close` explicitly releases it, and `trap` verifies host retirement.
This package is never distributed, registered in the marketplace or exported as SDK input.
