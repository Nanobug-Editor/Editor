# editor-ui

Nanobug's local native controls. GPUI Kit supplies core primitives and static assets;
GPUI Base owns behavior, input, focus and retained state. The styled component
runtime is excluded from both the selected dependency graph and Cargo.lock.

Each control has one entry point at `src/<control>/mod.rs`. Its drawing, state and
tests stay inside the same directory; child modules with supporting files also use
`mod.rs`. `src/lib.rs` declares the controls and exports the intended caller interface.

Theme resolution, typography, local icons and the shared bilingual translation
catalog live here. The application composes plugin assets, chooses contributed
themes and handles workspace/plugin lifecycle operations in `editor-app`.

The `test-support` feature exposes bounded native input helpers using ordinary
GPUI window dispatch. Test dependencies enable the already-locked Base and GPUI
test foundations directly; Kit's `test-support` feature remains disabled.

Run `cargo test -p editor-ui` from the workspace root. Cross-feature tests involving
the editor application remain in `editor-app`.
