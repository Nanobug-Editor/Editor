# Frozen plugin test inputs

`canvas-flow-1.0.0.zip` is the independent public `ui.canvas 1.7` scene consumer in `../canvas-flow`.
It covers curved edges, rounded nodes, viewport input and selected-node repaint, plus rejected-scene
recovery through the public Manager. It is an acceptance fixture, excluded from distribution and SDK.

`canvas-flow-1.1.0.zip` and `canvas-table-1.0.0.zip` independently consume public `ui.canvas 1.8`.
They exercise original-grapheme edits, guest-owned undo, native target focus/IME, bounded semantic
trees, and the public Manager's shown-frame/acknowledgement/retirement gates. The table declares six
visible cells and 100000 logical rows. Native acceptance loads these exact ledger entries and drives
real Windows Chinese IME plus UI Automation; candidate overrides are optional development inputs.
The historical flow 1.0 archive and its ledger entry remain first and unchanged. Both newer packages
were produced with the recorded committed host/SDK; later native geometry fixes retain that same
SDK and guest source. They are excluded from distribution and SDK and create no marketplace listing.

`markdown-0.18.2-readme-pr.zip` is the isolated source-PR README presentation candidate: paragraph alignment,
accessible image alt semantics and Primer metrics through public `ui.styles`. The 0.18.0/0.18.1
local candidates and the first `markdown-0.18.2.zip` remain preserved. `sdk-styles-fixture-1.0.0.zip` is an independent minimal current-SDK
consumer for reusable style negotiation and demand-owned view activation; its source is in ../sdk-styles.
Each explicit ledger entry
records its actual public SDK digest and leaves revisions that were uncommitted at build time null. Native
Markdown behavior regressions select this archive explicitly; bare `markdown` and marketplace
distribution snapshots keep the reviewed 0.17.2 package. Neither the new fixture nor a successful
build changes marketplace admission or any already published ZIP.

The adjacent `sdk-canvas` project independently exercises the current `editor.layout` contract: it
publishes a native source bound to the exact Preview version beside a zoomable SVG canvas. No
placeholder source is published during Prepare; unsaved text comes only from Preview. Build it with
the same public `--plugin-cargo` release/wasm32-wasip2 command as `sdk-launch` and archive only its
manifest and component. The source and ZIP digests are fixed in the ledger; it is never a listing.

These archives are test inputs, excluded from the application, installer and exported SDK. Official business sources live in the seven independent `*-for-Nanobug` repositories. The six retired examples are retained only as immutable Package/SDK test artifacts; their IDs preserve existing lifecycle and migration assertions and do not create marketplace listings.

`inputs.json` records each exact version and SHA-256, its source commit, the host SDK digest and build toolchain. `tests/support/packages.rs` checks the fixed digest and inspects through the public `Package` boundary before extracting resource-only inputs. This does not execute guests or grant installation permissions; the existing Manager/native/ignored integration tests continue to cover those boundaries.

The official catalog and statistics are byte snapshots of the published Pages data, with their own digests and registry revision/measurement time in the ledger. Integration tests replace only the transport URL with a controlled local HTTP server. Historical Rust, Rust Debugger and TOML packages exercise real existing-user migration; they are never marketplace candidates.

The original `configuration-alpha` and `configuration-beta` acceptance variants are also frozen separately. Their components differ from the generic example, so tests retain the original bytes instead of inferring interchangeability. Their original SDK/source/toolchain digests were not recorded; per-entry nulls state that limitation and override the ledger's current build baseline. These variants are controlled historical test inputs and are never marketplace candidates.

Run resource and ordinary host checks with Cargo. For real WASM/native checks, invoke the affected integration binary with `--ignored` after reading its native-tool prerequisite. Build native fixtures directly with `cargo build -p plugin-runtime --examples`. If a Windows test binary must be copied to avoid a link lock, keep the copy inside the same `target/debug/deps` directory so its sibling `examples` lookup remains valid. Image worker lifecycle tests share a process-wide pool and must run with `--test-threads=1`.

Development-directory tests stage the frozen example through public `development::stage` and read it through `development::read_directory`; an explicit `NANOBUG_DEVELOPMENT_CANDIDATE` still permits independent CLI output. The adjacent `sdk-completion` project is a minimal independent public SDK consumer used by Rust Analyzer, with no local SDK source or path dependency. It is excluded from the host workspace and declares its own workspace.

The adjacent `sdk-launch` project supplies a current public `run.configurations` validation provider for literal argv, environment, preparation and discovered-target regressions. Its opaque values serialize the public `Launch` contract; the real configuration actor validates them again before starting any host-owned session. Native configuration-window editing remains covered by the separate Alpha/Beta and Cargo/Shell package tests. This fixture never executes a program itself and is not a marketplace listing. Build it directly with `Nanobug.exe --plugin-cargo <absolute-sdk-launch-Cargo.toml> build --release --target wasm32-wasip2`, using an absolute short `CARGO_TARGET_DIR` on Windows if MSVC exceeds its path limit; archive only `manifest.json` and the resulting component under its declared filename. The ledger fixes source and ZIP digests.

To replace an input, check out the recorded source commit in a separate checkout (the example sources are recoverable at the recorded Nanobug commit). Export/use that host's SDK with its public `--plugin-cargo` / `--plugin-package` commands, then invoke Cargo and archive tools directly. Do not call retired scripts or embed SDK sources. Update the ledger only after actual Package validation and affected consumer tests. SDK, source, manifest, target, lockfile or toolchain changes require rebuilding the affected input; ZIP equality is verified from bytes, not inferred from source equality.
