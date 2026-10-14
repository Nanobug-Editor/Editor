# Nanobug

<p align="center">
  <img src="crates/editor-app/assets/branding/nanobug.png" alt="Nanobug logo" width="128" height="128" />
</p>

**English** · [简体中文](README.zh-CN.md)

Nanobug is a native desktop code editor built with Rust and GPUI, with Windows as its primary platform.

The editor, reviewed marketplace and official plugins are maintained in the [Nanobug organization](https://github.com/Nanobug-Editor). The editor repository is named `Editor`; plugin repositories use their language or feature names.

The built-in terminal uses upstream Alacritty and shares one bottom panel with build, run and debug sessions; no terminal plugin installation is required. Independent plugins provide language support, Markdown and image previews, build rules and debuggers. Official and third-party plugins use the same public interfaces. Fresh installations contain no plugins or automatic recommendations: plain-text editing, the native terminal and the default theme are available immediately.

The XML plugin provides schema assistance, completion, formatting, paired tag editing, and a document outline. HTML and JavaScript provide independent native language services. New workspaces start with the outline hidden; open it from the bottom window controls.

The native plugin manager reads the [reviewed marketplace](https://github.com/Nanobug-Editor/Plugin-Marketplace). Browse historical releases and changelogs, choose a compatible version for first installation, and explicitly install higher compatible versions. A red dot on the plugin button indicates available upgrades. Every download must match the reviewed ZIP SHA-256; updates that request new permissions require consent. Catalog failures leave cached browsing available and disable market installation until a successful refresh. Download counts are advisory and retain their measurement time during outages.

[Getting started](#developing-nanobug) · [Plugin SDK sources](crates/plugin-protocol/src/lib.rs) · [Maintainer documents](docs/README.md) · [Report an issue](https://github.com/Nanobug-Editor/Editor/issues)

## Installation

Build the current source to use the organization's marketplace endpoint. Binaries built before the organization migration still use the retired personal Pages address; installed plugins, settings and private data remain on your machine.

Nanobug is under active development. On Windows, run `Nanobug-Setup-<version>-x64.exe` to install for your user account. The installer creates a Start menu entry, offers an optional desktop shortcut, and registers an uninstaller. A separate option, checked on first installation, adds **Open with Nanobug** to folder and folder background menus; upgrades retain your choice. On Windows 11, these entries may appear under **Show more options**. It installs the program, icon and host licenses under `%LOCALAPPDATA%\Programs\Nanobug`; Rust and Cargo are not required to use them.

Exit Nanobug before installing an update or uninstalling. Settings, history, and installed plugin data in the existing `MeEditor` directories are preserved. Windows installers include English and Simplified Chinese.

With the development prerequisites below and [Inno Setup](https://jrsoftware.org/isdl.php) prepared, build the host directly:

```powershell
# Build the native Release host; direct development launches also remain available.
cargo build -p editor-app --release
cargo run --release
```

Prepare the executable, icon and host licenses as described in [native packaging](installer/README.md). Then compile the prepared Windows payload directly from the repository root:

```powershell
# Read the actual host version and invoke the native installer compiler directly.
$version = ((cargo metadata --no-deps --format-version 1 | ConvertFrom-Json).packages |
    Where-Object name -eq 'editor-app').version
$payload = (Resolve-Path .\dist\editor).Path
$output = Join-Path $PWD 'dist/installers'
ISCC.exe "/DPayloadDir=$payload" "/DInstallerDir=$output" "/DAppVersion=$version" .\installer\windows\nanobug.iss
```

Installers are written to `dist/installers/`; prepared runtime files remain in `dist/editor/`. Specify the compiler's full path if ISCC is not on PATH. Keep the adjacent `licenses/` directory for a portable copy. Plugin ZIPs and first-use catalogs do not belong in the payload; plugins are installed into the existing private user store.

The repository contains no helper-script directory. Packaging must call Cargo, the host's public CLI, archive tools, and native installer tools directly; it must not invoke archived Codex helper scripts, including through wrappers.

The earlier installer phase validated Windows installation, in-place repair, running-app protection, and uninstall. macOS application bundles, DMG/PKG, and Linux DEB/RPM use direct native tools; preparation and commands are documented in [native packaging](installer/README.md). Those platforms are **not built or installation-tested in this phase**. Signing/notarization and platform-specific plugin dependencies require separate release work.

## Developing Nanobug

On Windows, development requires the Rust MSVC toolchain (Rust 1.95 or newer), C++ build tools, and the Windows SDK. Building WASM plugins separately requires the `wasm32-wasip2` target. See [Cargo.toml](Cargo.toml), [rust-toolchain.toml](rust-toolchain.toml), and [Cargo.lock](Cargo.lock) for version and dependency constraints.

Run from the repository root:

```powershell
# Start the Debug development build.
cargo run

# Start an optimized Release build.
cargo run --release

# Open a workspace; a file path is also accepted.
cargo run --release -- C:\path\to\project
```

Debug builds are unoptimized; window movement and editing may be slower. Use Release for everyday evaluation.

Starting without a path opens the project home. Choose **Open folder** or a recent project; a folder needs no Git repository or project file. A new project asks for trust or restricted editing, while existing local choices are retained. **Close project** returns the same window to the home after saving or discarding changes and confirming any running programs. Cancelling or a save failure keeps the project open. Reopening restores saved tabs and layout without restarting programs.

Opening another project keeps the current project running in its own window. Reopening the same directory activates its existing window, including requests from File Explorer or another ordinary launch. An argument-free second launch reuses or opens a home. **Quit application** gathers every window's decisions; any cancellation keeps all windows available. Portable runs accept the same directory/file arguments without registering Explorer menus. Use `Nanobug.exe --profile "<separate profile directory>" -- "<project or file>"` for an isolated GUI profile; plugin tooling and explicit development instances retain their independent startup behavior.

The six workspace crates cover the application, document core, platform integration, plugin schema, public protocol, and plugin runtime. Official plugin sources live in the independent [plugin repositories in the Nanobug organization](https://github.com/Nanobug-Editor/Plugin-Marketplace/tree/main/registry); fixed test-only Package inputs live in `crates/plugin-runtime/tests/fixtures/packages/`; native packaging configuration and instructions live in `installer/`. See the [maintainer documentation index](docs/README.md) for details.

The Cargo package remains `editor-app`. Existing `MeEditor` data directories, `ME_EDITOR_*` environment variables, and persisted command identifiers are retained to preserve settings, history, and installed plugins.

## Plugin development

Each plugin project uses a versioned `nanobug-plugin.json` to declare its manifest, optional WASM/native builds and explicit assets. GUI and CLI share this policy. ZIPs default to **each plugin project root**; a local configuration or `--output` can override the directory. A successful build atomically replaces `<id>-<version>.zip`; failure retains the previous ZIP. Packaging does not bump the manifest version.

In **Edit configurations → Add → Nanobug**, choose **Plugin Packaging** or **Plugin Development** to create a draft. Packaging supports several projects and an output-directory picker. Development opens an isolated editor with the development version; a new profile contains no other plugins, without creating a ZIP. Its logs offer manual reload; automatic reload is optional. Build only prepares artifacts, and WASM source breakpoints are unavailable. Settings, installed plugins, private data and history are isolated and retained per configuration; resetting the environment starts afresh.

```powershell
# Build a host that provides the current SDK.
cargo build -p editor-app

# Build and ZIP a plugin, using its project root as output.
.\target\debug\editor-app.exe --plugin-package ../Rust

# Batch packaging; relative --output is resolved from the current directory.
.\target\debug\editor-app.exe --plugin-package ../TOML ../HTML --output target/plugin-build

# Prepare a directory candidate without ZIP or launching a development window.
.\target\debug\editor-app.exe --plugin-build ../Rust

# Prepare the development instance; add explicit --grant arguments for the manifest's declared permissions.
.\target\debug\editor-app.exe --plugin-dev ../Rust --watch --grant workspace.read --grant process.exec --grant ui.panels --grant process.service.analysis --grant process.service.adapter --grant dependencies.prepare

# Low-level independent Cargo builds still use the host's embedded SDK.
.\target\debug\editor-app.exe --plugin-cargo ../Rust/Cargo.toml check --target wasm32-wasip2

# Export SDK source and protocol resources (no Markdown) for an external toolchain.
.\target\debug\editor-app.exe --export-plugin-sdk .\target\sdk-export
```

For CLI development, send `reload` or `stop` on standard input. `--workspace` selects a test workspace (the plugin project by default); `--profile` selects isolated data. Repeat `--grant <permission>` for the permissions declared in the manifest. New permissions require stopping and authorizing another run; changing the plugin ID also requires restarting the configuration. Missing compilers/SDK targets are reported and never installed automatically. Native build steps declare their exact OS/architecture; foreign native artifacts are rejected. Windows is verified; macOS/Linux use portable paths and process interfaces but remain unverified.

Install or update generated ZIPs in the plugin manager and approve their declared permissions. Interface usage is documented in [SDK source comments](crates/plugin-protocol/src/lib.rs); debugger setup is described in the [Rust plugin README](https://github.com/Nanobug-Editor/Rust). Maintainer decisions and verification are indexed in [docs/plugins/README.md](docs/plugins/README.md).

## Contributing

Use [GitHub Issues](https://github.com/Nanobug-Editor/Editor/issues) to report bugs or propose changes. Before editing, find the relevant specification and acceptance requirements in the [maintainer documentation index](docs/README.md).

Basic checks for Rust changes:

```powershell
# Check Rust formatting.
cargo fmt --check

# Run workspace tests excluding the UI application.
cargo test --workspace --exclude editor-app

# Check compilation of the entire workspace.
cargo check --workspace
```

Changes to `editor-app` also require relevant UI tests and native interaction checks. Tests marked ignored that use actual WASM packages require their fixtures to be built before explicitly running them. Documentation-only changes require link and path checks plus `git diff --check`.

Basic run, build, and packaging commands are maintained in this README. Plugin interface usage is documented beside the public types in [the protocol sources](crates/plugin-protocol/src/lib.rs). The independent SDK exports only Rust sources, Cargo manifests, WIT, and JSON protocol resources; it does not bundle Markdown. Specifications, tickets, and verification records live in `docs/`. Keep this English README and [its Chinese translation](README.zh-CN.md) in sync.

## Licensing

Workspace packages declare the MIT license in [Cargo.toml](Cargo.toml). Third-party dependencies and plugin grammars, icons, and other assets retain their respective licenses; see the independent plugin repositories and their reviewed Release packages for attribution and license texts.
