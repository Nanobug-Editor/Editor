//! Read local ZIP packages into bounded memory before any installation side effect.
use plugin_protocol::Manifest;
use plugin_schema::{FileIconConfig, PluginManifest, ThemeFile};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Cursor, Read},
    path::Path,
};
mod atomic_file;
pub(crate) mod icons;
pub(crate) use atomic_file::atomic_write;

/// Parse only the actual installed contribution bytes for static discovery and package admission.
/// The caller validates the package-relative path/owner; this gate checks identity, version and
/// the author's host range without reading grammar assets or starting a business instance.
pub(crate) fn compatible_contributions(
    manifest: &Manifest,
    bytes: &[u8],
) -> anyhow::Result<PluginManifest> {
    let contributions = PluginManifest::parse(std::str::from_utf8(bytes)?)?;
    let host = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;
    let supported = semver::VersionReq::parse(&contributions.plugin.host_version)?;
    anyhow::ensure!(
        supported.matches(&host),
        "Unsupported host version: requires {supported}, host {host}"
    );
    anyhow::ensure!(
        contributions.plugin.id == manifest.id,
        "Contribution manifest identity differs from package identity"
    );
    anyhow::ensure!(
        contributions.plugin.version == manifest.version,
        "Contribution manifest version differs from package version"
    );
    Ok(contributions)
}

/// LSP declarations carry exactly the native service authority that the host will exercise.
fn validate_language_services(manifest: &Manifest) -> anyhow::Result<()> {
    use plugin_protocol::{api::InstanceScope, settings::SettingType};
    let mut ids = std::collections::BTreeSet::new();
    anyhow::ensure!(
        manifest.language_servers.len() <= 64,
        "Too many LSP providers"
    );
    for provider in &manifest.language_servers {
        anyhow::ensure!(
            manifest.protocol == 7
                && manifest.scope == InstanceScope::Workspace
                && manifest
                    .api
                    .as_ref()
                    .is_some_and(|api| api.required.contains_key("language.lsp")
                        && (provider.delegate.is_some() || api.required.contains_key("process")))
                && provider.valid()
                && ids.insert(&provider.id),
            "Invalid LSP provider declaration"
        );
        anyhow::ensure!(
            !provider.hook || manifest.component.is_some(),
            "LSP hooks require WASM"
        );
        if !provider.languages.is_empty()
            || provider.delegate.is_some()
            || provider.related_documents.is_some()
        {
            anyhow::ensure!(
                manifest
                    .api
                    .as_ref()
                    .and_then(|api| api.required.get("language.lsp"))
                    .is_some_and(|requirement| (requirement
                        .matches(&semver::Version::new(1, 5, 0))
                        || requirement.matches(&semver::Version::new(1, 6, 0)))
                        && !requirement.matches(&semver::Version::new(1, 4, 0))),
                "Document contexts, aliases and delegation require language.lsp >=1.5"
            );
        }
        if !provider.control_methods.is_empty() {
            anyhow::ensure!(
                manifest
                    .api
                    .as_ref()
                    .is_some_and(|api| api.required.contains_key("language.control")),
                "Native control declarations require language.control"
            );
        }
        if let Some(delegate) = &provider.delegate {
            let method = manifest
                .plugin_services
                .provides
                .get(&delegate.contract)
                .and_then(|contract| contract.methods.get(&delegate.method));
            anyhow::ensure!(
                manifest.component.is_some()
                    && method
                        == Some(
                            &plugin_protocol::language_control::binding_prepare_method_for(
                                delegate.interactive
                            )
                        ),
                "Delegated language providers require the canonical public prepare method"
            );
            anyhow::ensure!(
                !provider.hook
                    && provider.alternatives.is_empty()
                    && provider.executable_setting.is_none()
                    && provider.control_methods.is_empty()
                    && !provider.optional_installation,
                "A delegated language provider cannot invent a native startup plan"
            );
        }
        if provider.internal
            || provider
                .delegate
                .as_ref()
                .is_some_and(|delegate| delegate.interactive)
        {
            anyhow::ensure!(
                manifest
                    .api
                    .as_ref()
                    .and_then(|api| api.required.get("language.lsp"))
                    .is_some_and(|range| range.matches(&semver::Version::new(1, 6, 0))
                        && !range.matches(&semver::Version::new(1, 5, 0))),
                "Internal and interactive providers require language.lsp >=1.6"
            );
        }
        // Independent roles negotiate before installation, so an older host cannot silently
        // treat a formatting-only contribution as the document's primary analysis service.
        for (enabled, capability) in [
            (
                !provider.primary || provider.formatting,
                "language.formatting",
            ),
            (provider.editing, "language.editing"),
        ] {
            anyhow::ensure!(
                !enabled
                    || manifest
                        .api
                        .as_ref()
                        .is_some_and(|api| api.required.contains_key(capability)),
                "Language editing role requires {capability}"
            );
        }
        anyhow::ensure!(
            provider.primary || provider.formatting,
            "A language service must provide analysis or formatting"
        );
        if provider.optional_installation {
            // Capability negotiation already checks the current host version. Reject the retired
            // 1.0 contract without requiring stricter 1.2 declarations to also include exactly 1.1.
            anyhow::ensure!(
                manifest
                    .api
                    .as_ref()
                    .and_then(|api| api.required.get("dependencies"))
                    .is_some_and(|requirement| {
                        !requirement.matches(&semver::Version::new(1, 0, 0))
                    }),
                "Optional service installation requires dependencies >=1.1"
            );
        }
        if provider.diagnostic_snapshots {
            anyhow::ensure!(
                manifest
                    .api
                    .as_ref()
                    .and_then(|api| api.required.get("language.lsp"))
                    .is_some_and(|requirement| {
                        requirement.matches(&semver::Version::new(1, 5, 0))
                            && !requirement.matches(&semver::Version::new(1, 2, 0))
                    }),
                "Immutable diagnostic snapshots require language.lsp >=1.3"
            );
        }
        anyhow::ensure!(
            !provider.completion_hook
                || (manifest.component.is_some()
                    && manifest.permissions.contains("editor.read")
                    && manifest
                        .api
                        .as_ref()
                        .is_some_and(|api| api.required.contains_key("language.completion"))),
            "Snapshot completion requires WASM, language.completion and editor.read"
        );
        for service in std::iter::once(&provider.service).chain(&provider.alternatives) {
            if provider.delegate.is_some() {
                continue;
            }
            anyhow::ensure!(
                manifest.services.contains_key(service)
                    && manifest
                        .permissions
                        .contains(&format!("process.service.{service}")),
                "LSP service must be declared and granted"
            );
        }
        if let Some(key) = &provider.executable_setting {
            anyhow::ensure!(
                manifest
                    .settings
                    .get(key)
                    .is_some_and(|definition| matches!(
                        definition.value_type,
                        SettingType::String { .. }
                    )),
                "LSP executable setting must declare a string"
            );
        }
    }
    Ok(())
}
#[derive(Clone)]
pub struct Package {
    pub manifest: Manifest,
    pub files: BTreeMap<String, Vec<u8>>,
    pub digest: String,
    /// Display the exact local package source in the consent UI; it grants no trust by itself.
    pub source: Option<String>,
}
impl Package {
    /// A package is a ZIP with a manifest and either executable WASM or declarative assets.
    pub fn read(path: &Path) -> anyhow::Result<Self> {
        anyhow::ensure!(
            path.metadata()?.len() <= 64 * 1024 * 1024,
            "Package exceeds 64 MiB"
        );
        let mut package = Self::from_bytes(&std::fs::read(path)?)?;
        package.source = Some(path.display().to_string());
        Ok(package)
    }
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        anyhow::ensure!(bytes.len() <= 64 * 1024 * 1024, "Package too large");
        let digest = format!("{:x}", Sha256::digest(bytes));
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes))?;
        anyhow::ensure!(zip.len() <= 512, "Too many package entries");
        let mut files = BTreeMap::new();
        let mut total = 0;
        let mut actual_total = 0usize;
        for index in 0..zip.len() {
            let mut entry = zip.by_index(index)?;
            if entry.is_dir() {
                continue;
            }
            let name = entry.name().to_owned();
            validate_relative(&name)?;
            anyhow::ensure!(!entry.is_symlink(), "Package symlinks are forbidden");
            total += entry.size();
            anyhow::ensure!(total <= 128 * 1024 * 1024, "Expanded package exceeds quota");
            let mut data = Vec::new();
            let remaining = 128 * 1024 * 1024 - actual_total;
            entry
                .by_ref()
                .take(remaining as u64 + 1)
                .read_to_end(&mut data)?;
            anyhow::ensure!(
                data.len() <= remaining,
                "Actual expanded package exceeds quota"
            );
            actual_total += data.len();
            anyhow::ensure!(files.insert(name, data).is_none(), "Duplicate package path");
        }
        Self::validated_files(files, digest)
    }

    /// Inspect a development directory's immutable bytes without creating an intermediate ZIP.
    /// Paths, quotas, permissions, protocol and resource references use the installation validator.
    /// Returns a content-addressed package, or an error before any registry/data mutation.
    pub fn from_files(files: BTreeMap<String, Vec<u8>>) -> anyhow::Result<Self> {
        anyhow::ensure!(files.len() <= 512, "Too many package entries");
        let mut hash = Sha256::new();
        let mut total = 0usize;
        for (name, bytes) in &files {
            validate_relative(name)?;
            total = total
                .checked_add(bytes.len())
                .ok_or_else(|| anyhow::anyhow!("Package quota overflow"))?;
            anyhow::ensure!(total <= 128 * 1024 * 1024, "Expanded package exceeds quota");
            // Length prefixes keep different path/content boundaries from sharing a digest.
            hash.update((name.len() as u64).to_le_bytes());
            hash.update(name.as_bytes());
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
        Self::validated_files(files, format!("{:x}", hash.finalize()))
    }

    /// Archive and development inputs converge here; neither can bypass admission checks.
    fn validated_files(files: BTreeMap<String, Vec<u8>>, digest: String) -> anyhow::Result<Self> {
        let manifest: Manifest = serde_json::from_slice(
            files
                .get("manifest.json")
                .ok_or_else(|| anyhow::anyhow!("Missing manifest.json"))?,
        )
        .map_err(|error| {
            // CLI and UI share technical details; the native caller translates its operation label.
            anyhow::anyhow!("Incompatible or invalid plugin manifest; update the plugin with the current SDK: {error}")
        })?;
        // New archives keep their declared identity; historical aliases belong exclusively to data import.
        anyhow::ensure!(
            manifest.id.len() <= 100
                && !manifest.id.is_empty()
                && manifest.id.bytes().all(|b| b.is_ascii_lowercase()
                    || b.is_ascii_digit()
                    || b == b'.'
                    || b == b'-')
                && !manifest.id.starts_with('.'),
            "Invalid plugin identity"
        );
        semver::Version::parse(&manifest.version)?;
        super::capabilities::require_current(&manifest)?;
        validate_language_services(&manifest)?;
        crate::native_modules::validate_exports(&manifest, &files)?;
        crate::structure::validate_manifest(&manifest)?;
        crate::direct::validate_manifest(&manifest)?;
        manifest
            .plugin_services
            .validate()
            .map_err(anyhow::Error::msg)?;
        if !manifest.plugin_services.provides.is_empty()
            || !manifest.plugin_services.requires.is_empty()
        {
            anyhow::ensure!(
                manifest.protocol == 7
                    && manifest.component.is_some()
                    && manifest
                        .api
                        .as_ref()
                        .is_some_and(|api| api.required.contains_key("plugin.services")),
                "Plugin service declarations require a component and plugin.services capability"
            );
            anyhow::ensure!(
                manifest.plugin_services.requires.is_empty()
                    || manifest.permissions.contains("services.call"),
                "Service consumers require services.call permission"
            );
            for method in manifest
                .plugin_services
                .provides
                .values()
                .flat_map(|c| c.methods.values())
                .chain(
                    manifest
                        .plugin_services
                        .requires
                        .values()
                        .flat_map(|c| c.methods.values()),
                )
            {
                anyhow::ensure!(
                    method.permissions.is_subset(&manifest.permissions),
                    "Service method authority must be explicitly requested in the package"
                );
            }
        }
        if manifest.permissions.contains("dependencies.install") {
            anyhow::ensure!(
                manifest.permissions.contains("dependencies.prepare"),
                "Installer permission requires dependency preparation"
            );
        }
        if manifest.permissions.contains("dependencies.prepare") {
            anyhow::ensure!(
                manifest.protocol == 7
                    && manifest
                        .api
                        .as_ref()
                        .is_some_and(|api| api.required.contains_key("dependencies")),
                "Dependency preparation requires dependencies capability"
            );
        }
        for service in manifest.services.values() {
            if let Some(plan) = &service.installation {
                anyhow::ensure!(
                    manifest.permissions.contains("dependencies.prepare"),
                    "Dependency preparation permission required"
                );
                crate::dependencies::validate(plan)?;
                for artifact in &plan.artifacts {
                    anyhow::ensure!(
                        artifact.installer.is_none()
                            || manifest.permissions.contains("dependencies.install"),
                        "Installer permission required"
                    );
                    if let plugin_protocol::dependencies::Source::Package { path } =
                        &artifact.source
                    {
                        package_bytes(&files, path)?;
                    }
                }
            }
        }
        // Service keys are authority scopes, and definitions never accept dynamic command templates.
        anyhow::ensure!(manifest.services.len() <= 32, "Too many native services");
        for (id, service) in &manifest.services {
            anyhow::ensure!(
                manifest.protocol == 7
                    && !id.is_empty()
                    && id.len() <= 100
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                    && service.valid(),
                "Invalid native service declaration"
            );
        }
        // Additive preparation, contribution gates and finite replacement require an aware host.
        let deferred_metadata = !manifest.replaces.is_empty()
            || manifest
                .services
                .values()
                .any(|service| service.on_demand || !service.platforms.is_empty())
            || manifest.plugin_services.provides.values().any(|contract| {
                !contract.preparation.is_empty()
                    || contract.enabled_setting.is_some()
                    || !contract.platforms.is_empty()
            });
        if deferred_metadata {
            let api = crate::capabilities::negotiate(&manifest)?;
            anyhow::ensure!(
                api.capabilities
                    .get("dependencies")
                    .is_some_and(|version| *version >= semver::Version::new(1, 2, 0)),
                "Deferred metadata requires dependencies 1.2"
            );
        }
        for service in manifest
            .services
            .values()
            .filter(|service| service.on_demand)
        {
            anyhow::ensure!(
                service.installation.is_some(),
                "Deferred service requires an installation plan"
            );
        }
        // Reject malformed forms before installation, not when a settings window tries to render them.
        for contract in manifest.plugin_services.provides.values() {
            if !contract.preparation.is_empty() {
                let api = crate::capabilities::negotiate(&manifest)?;
                anyhow::ensure!(
                    api.capabilities
                        .get("dependencies")
                        .is_some_and(|v| *v >= semver::Version::new(1, 2, 0)),
                    "Method preparation requires dependencies 1.2"
                );
            }
            anyhow::ensure!(
                contract.preparation.len() <= 32
                    && contract
                        .preparation
                        .iter()
                        .all(|(method, services)| contract.methods.contains_key(method)
                            && services.len() <= 8
                            && services.iter().all(|id| manifest
                                .services
                                .get(id)
                                .is_some_and(|service| service.installation.is_some())
                                && manifest
                                    .permissions
                                    .contains(&format!("process.service.{id}")))),
                "Invalid native method preparation"
            );
            if let Some(key) = &contract.enabled_setting {
                anyhow::ensure!(
                    manifest
                        .settings
                        .get(key)
                        .is_some_and(|definition| definition.default.is_boolean()),
                    "Service gate requires a declared boolean setting"
                );
            }
        }
        anyhow::ensure!(
            manifest.replaces.len() <= 8
                && (manifest.replaces.is_empty() || manifest.component.is_some()),
            "Package replacements require a component and at most eight sources"
        );
        let mut replaced = std::collections::BTreeSet::new();
        for replacement in &manifest.replaces {
            validate_relative(&replacement.id)?;
            anyhow::ensure!(
                replacement.id != manifest.id
                    && !replacement.id.contains('/')
                    && replaced.insert(&replacement.id)
                    && !replacement.contracts.is_empty()
                    && replacement.contracts.len() <= 32
                    && replacement.contracts.iter().all(|id| manifest
                        .plugin_services
                        .provides
                        .get(id)
                        .is_some_and(|contract| contract.enabled_setting.as_ref()
                            == Some(&replacement.enabled_setting))),
                "Invalid package replacement"
            );
        }
        // Reject malformed forms before installation, not when a settings window tries to render them.
        anyhow::ensure!(manifest.settings.len() <= 64, "Too many settings");
        anyhow::ensure!(
            manifest
                .settings
                .iter()
                .all(|(key, definition)| !key.is_empty()
                    && key.len() <= 64
                    && key
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
                    && definition.valid()),
            "Invalid settings declaration"
        );
        if !manifest.settings.is_empty() || manifest.settings_hook {
            anyhow::ensure!(
                manifest.protocol == 7,
                "Settings require the capability protocol"
            );
            anyhow::ensure!(
                !manifest.settings_hook || manifest.component.is_some(),
                "Settings hooks require WASM"
            );
            if manifest.component.is_some() {
                anyhow::ensure!(
                    manifest
                        .api
                        .as_ref()
                        .is_some_and(|api| api.required.contains_key("configuration")),
                    "Settings require configuration capability"
                );
            }
        }
        anyhow::ensure!(
            (1024..=32 * 1024 * 1024).contains(&manifest.storage_limit),
            "Invalid storage quota"
        );
        if let Some(format) = &manifest.data_format {
            anyhow::ensure!(
                manifest.protocol == 7
                    && manifest.component.is_some()
                    && format.version > 0
                    && manifest.permissions.contains("storage")
                    && manifest.api.as_ref().is_some_and(|api| api
                        .required
                        .contains_key("storage.private")
                        && (!format.migration_hook
                            || api.required.contains_key("storage.migration"))),
                "Data formats require a private-storage component and negotiated migration hooks"
            );
        }
        anyhow::ensure!(
            manifest.component.is_some()
                || manifest.contributions.is_some()
                || !manifest.module_exports.is_empty(),
            "Package declares neither a component nor contributions"
        );
        if let Some(path) = &manifest.component {
            validate_relative(path)?;
            let wasm = files
                .get(path)
                .ok_or_else(|| anyhow::anyhow!("Missing component"))?;
            anyhow::ensure!(
                wasm.starts_with(b"\0asm\x0d\0\x01\0"),
                "Expected a WebAssembly component, not a core module"
            );
        } else {
            // Declarative native language consumers have their own sealed runtime lease.
            // Their original document/navigation grants need no empty guest component;
            // executable UI, commands and unrestricted process calls still require one.
            anyhow::ensure!(
                manifest.panels.is_empty()
                    && manifest.commands.is_empty()
                    && manifest.permissions.iter().all(|permission| {
                        if permission == "native.modules" && !manifest.module_exports.is_empty() {
                            return true;
                        }
                        if matches!(
                            permission.as_str(),
                            "dependencies.prepare" | "dependencies.install"
                        ) {
                            return true;
                        }
                        if !manifest.language_servers.is_empty()
                            && matches!(
                                permission.as_str(),
                                "editor.read" | "editor.write" | "workspace.read"
                            )
                        {
                            return true;
                        }
                        permission
                            .strip_prefix("process.service.")
                            .is_some_and(|service| {
                                manifest.language_servers.iter().any(|provider| {
                                    provider.service == service
                                        || provider.alternatives.iter().any(|id| id == service)
                                })
                            })
                    }),
                "Declarative packages cannot request executable capabilities"
            );
        }
        if let Some(path) = &manifest.contributions {
            // Validate every declarative reference before installation can mutate the registry.
            validate_relative(path)?;
            let source = package_text(&files, path)?;
            // Local ZIP/development admission enforces the author's same host range as remote
            // discovery, before extracting files or mutating any installed registry/data.
            let contributions = compatible_contributions(&manifest, source.as_bytes())?;
            if !contributions.language_definitions.is_empty()
                || !contributions.highlighters.is_empty()
            {
                anyhow::ensure!(
                    manifest.protocol == 7,
                    "Dynamic languages require capability protocol"
                );
            }
            for provider in &contributions.highlighters {
                let grammar = package_bytes(&files, &provider.grammar.to_string_lossy())?;
                anyhow::ensure!(grammar.starts_with(b"\0asm"), "Invalid grammar module");
                package_text(&files, &provider.highlights.to_string_lossy())?;
                if let Some(path) = &provider.injections {
                    package_text(&files, &path.to_string_lossy())?;
                }
            }
            if let Some(theme) = &contributions.theme {
                ThemeFile::parse(package_text(&files, &theme.file.to_string_lossy())?)?;
                if let Some(path) = &theme.file_icons {
                    validate_file_icons(&files, path)?;
                }
            }
            if let Some(path) = &contributions.file_icons {
                validate_file_icons(&files, path)?;
            }
        }
        for permission in &manifest.permissions {
            anyhow::ensure!(
                [
                    "workspace.read",
                    "workspace.write",
                    "network.images",
                    "network.http",
                    "secrets",
                    "navigation.external",
                    "storage",
                    "clipboard",
                    "assets.read",
                    "editor.read",
                    "editor.diagnostics",
                    "editor.write",
                    "ui.panels",
                    "ui.interaction",
                    "files.select",
                    "commands.call",
                    "services.call",
                    "native.modules",
                    "process.exec",
                    "dependencies.prepare",
                    "dependencies.install"
                ]
                .contains(&permission.as_str())
                    || permission
                        .strip_prefix("network.http:")
                        .is_some_and(|target| crate::http::origin(target)
                            .is_ok_and(|origin| origin == target))
                    || permission
                        .strip_prefix("process.service.")
                        .is_some_and(|id| manifest.services.contains_key(id)),
                "Unsupported capability {permission}"
            );
        }
        anyhow::ensure!(
            manifest.panels.len() <= 8 && manifest.commands.len() <= 64,
            "Contribution quota exceeded"
        );
        let mut ids = std::collections::BTreeSet::new();
        for panel in &manifest.panels {
            anyhow::ensure!(
                !panel.id.is_empty()
                    && panel.id.len() <= 100
                    && !panel.id.contains('/')
                    && ids.insert(panel.id.clone()),
                "Invalid or duplicate panel ID"
            );
            anyhow::ensure!(
                ["left", "right", "bottom", "editor", "window"].contains(&panel.position.as_str()),
                "Unsupported dock position"
            );
            if panel.position == "window" {
                anyhow::ensure!(
                    manifest.scope == plugin_protocol::api::InstanceScope::Workspace
                        && manifest
                            .api
                            .as_ref()
                            .is_some_and(|api| api.required.contains_key("ui.windows"))
                        && manifest.permissions.contains("ui.panels"),
                    "Native work windows require workspace scope, ui.windows and ui.panels"
                );
            }
            anyhow::ensure!(
                !panel.auxiliary || panel.position == "editor",
                "Auxiliary file tools require an editor-local contribution"
            );
            // Preview ingress is workspace-owned and explicitly requires document read authority.
            anyhow::ensure!(
                if panel.position == "editor" {
                    manifest.permissions.contains("editor.read")
                        && manifest.scope == plugin_protocol::api::InstanceScope::Workspace
                        && manifest.api.as_ref().is_some_and(|api| {
                            api.required.contains_key("editor.documents")
                                || api.required.contains_key("editor.files")
                        })
                        && (!panel.file_extensions.is_empty()
                            || !panel.readonly_file_extensions.is_empty())
                        && panel.file_extensions.len() + panel.readonly_file_extensions.len() <= 32
                        && (panel.readonly_file_extensions.is_empty()
                            || manifest
                                .api
                                .as_ref()
                                .is_some_and(|api| api.required.contains_key("editor.files")))
                        && panel
                            .file_extensions
                            .iter()
                            .chain(&panel.readonly_file_extensions)
                            .all(|extension| {
                                !extension.is_empty()
                                    && extension.len() <= 32
                                    && extension.bytes().all(|byte| {
                                        byte.is_ascii_alphanumeric()
                                            || matches!(byte, b'_' | b'-' | b'+')
                                    })
                            })
                } else {
                    panel.file_extensions.is_empty() && panel.readonly_file_extensions.is_empty()
                },
                "Editor previews require document read authority and valid file extensions"
            );
            // Panel artwork must come from this package and remain small enough for native UI.
            for icon in [&panel.icon_light, &panel.icon_dark].into_iter().flatten() {
                validate_relative(icon)?;
                let svg = files
                    .get(icon)
                    .ok_or_else(|| anyhow::anyhow!("Missing panel icon: {icon}"))?;
                anyhow::ensure!(
                    svg.len() <= 64 * 1024
                        && std::str::from_utf8(svg)?.trim_start().starts_with("<svg"),
                    "Panel icon must be an SVG under 64 KiB"
                );
                let source = std::str::from_utf8(svg)?.to_ascii_lowercase();
                anyhow::ensure!(
                    !source.contains("<!doctype")
                        && !source.contains("<script")
                        && !source.contains("href="),
                    "Panel icon cannot load external content or scripts"
                );
            }
        }
        ids.clear();
        for command in &manifest.commands {
            command.validate().map_err(anyhow::Error::msg)?;
            anyhow::ensure!(
                !command.id.is_empty()
                    && command.id.len() <= 128
                    && !command.id.contains('/')
                    && ids.insert(command.id.clone()),
                "Invalid or duplicate command ID"
            );
            if let Some(signature) = &command.signature {
                anyhow::ensure!(
                    manifest.component.is_some()
                        && manifest
                            .api
                            .as_ref()
                            .is_some_and(|api| api.required.contains_key("plugin.commands")),
                    "Typed command declarations require a component and plugin.commands"
                );
                anyhow::ensure!(
                    signature.permissions.is_subset(&manifest.permissions),
                    "Command signature cannot grant undeclared permissions"
                );
            }
            anyhow::ensure!(
                command.menus.is_empty()
                    || (manifest.component.is_some()
                        && manifest
                            .api
                            .as_ref()
                            .is_some_and(|api| api.required.contains_key("plugin.commands"))),
                "Native menu contributions require a component and plugin.commands"
            );
        }
        anyhow::ensure!(
            !manifest
                .plugin_services
                .provides
                .keys()
                .chain(manifest.plugin_services.requires.keys())
                .any(|id| id.starts_with(plugin_protocol::commands::CONTRACT_PREFIX)),
            "Command transport contracts are reserved"
        );
        Ok(Self {
            manifest,
            files,
            digest,
            source: None,
        })
    }
    /// Executable packages expose their component; declarative packages have no guest.
    pub fn component(&self) -> Option<&[u8]> {
        self.manifest
            .component
            .as_ref()
            .and_then(|path| self.files.get(path).map(Vec::as_slice))
    }
    /// Materialize only validated files under a fresh, content-addressed version directory.
    pub(crate) fn extract(&self, path: &Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(path)?;
        for (name, bytes) in &self.files {
            let target = path.join(name);
            // A content-addressed version is immutable, including during a failed update.
            if target.exists() {
                anyhow::ensure!(
                    std::fs::read(&target)? == *bytes,
                    "Installed version content differs from package"
                );
                continue;
            }
            std::fs::create_dir_all(target.parent().unwrap())?;
            atomic_write(&target, bytes)?;
        }
        Ok(())
    }
}

/// Read only a named package file after checking its path and UTF-8 encoding.
fn package_text<'a>(files: &'a BTreeMap<String, Vec<u8>>, path: &str) -> anyhow::Result<&'a str> {
    std::str::from_utf8(package_bytes(files, path)?).map_err(Into::into)
}

/// Require all referenced resources to be present inside this ZIP archive.
fn package_bytes<'a>(files: &'a BTreeMap<String, Vec<u8>>, path: &str) -> anyhow::Result<&'a [u8]> {
    validate_relative(path)?;
    files
        .get(path)
        .map(Vec::as_slice)
        .ok_or_else(|| anyhow::anyhow!("Missing package asset: {path}"))
}

/// Check both the icon mapping and its referenced SVG files during package inspection.
fn validate_file_icons(files: &BTreeMap<String, Vec<u8>>, path: &Path) -> anyhow::Result<()> {
    let icons = FileIconConfig::parse(package_text(files, &path.to_string_lossy())?)?;
    for icon in icons.icons {
        for path in [&icon.light, &icon.dark] {
            let bytes = package_bytes(files, &path.to_string_lossy())?;
            let svg = std::str::from_utf8(bytes)?;
            let lower = svg.to_ascii_lowercase();
            anyhow::ensure!(
                bytes.len() <= 64 * 1024
                    && svg.trim_start().starts_with("<svg")
                    && !lower.contains("<!doctype")
                    && !lower.contains("<script")
                    && !lower.contains("href="),
                "File icon must be a safe SVG under 64 KiB"
            );
        }
    }
    Ok(())
}
pub(crate) fn validate_relative(name: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !name.is_empty()
            && !name.contains(['\\', ':'])
            && Path::new(name)
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_))),
        "Unsafe package path: {name}"
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Inspection rejects retired wire formats before any component can be installed or activated.
    #[test]
    fn package_requires_the_current_protocol_and_negotiated_api() {
        for (protocol, supported) in [
            (3, false),
            (4, false),
            (5, false),
            (6, false),
            (7, true),
            (8, false),
        ] {
            let mut manifest = serde_json::json!({
                "id": "test", "name": "Test", "version": "0.1.0",
                "protocol": protocol, "component": "test.wasm",
                "permissions": [], "storage_limit": 1024
            });
            if protocol == 7 {
                manifest["api"] = serde_json::json!({"base":"^1","required":{}});
            }
            let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
            zip.start_file("manifest.json", zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(&serde_json::to_vec(&manifest).unwrap())
                .unwrap();
            zip.start_file("test.wasm", zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"\0asm\x0d\0\x01\0").unwrap();
            let bytes = zip.finish().unwrap().into_inner();
            assert_eq!(Package::from_bytes(&bytes).is_ok(), supported);
        }
    }

    /// The host must reject an empty archive instead of installing an inert registry entry.
    #[test]
    fn package_requires_component_or_declarative_contributions() {
        let manifest = serde_json::json!({
            "id": "empty", "name": "Empty", "version": "0.1.0", "protocol": 7,
            "api": {"base":"^1","required":{}},
            "permissions": [], "storage_limit": 1024
        });
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("manifest.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&serde_json::to_vec(&manifest).unwrap())
            .unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        assert!(
            Package::from_bytes(&bytes)
                .err()
                .unwrap()
                .to_string()
                .contains("neither a component nor contributions")
        );
    }
    /// Packages are untrusted even when local: extraction never accepts relative traversal.
    #[test]
    fn rejects_unsafe_zip_paths() {
        for name in ["../escape", "/absolute", "C:/device", "a\\b", "./hidden"] {
            let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"data").unwrap();
            let bytes = zip.finish().unwrap().into_inner();
            assert!(Package::from_bytes(&bytes).is_err(), "accepted {name}");
        }
    }
    /// Failed replacement cannot truncate the last valid state file.
    #[test]
    fn state_replacement_is_complete() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("snapshot.json");
        atomic_write(&path, b"old").unwrap();
        atomic_write(&path, b"new valid data").unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"new valid data");
    }
}
