//! Package admission seals an adapter's complete relative file closure before installation approval.
use plugin_protocol::{Manifest, language_control::ModuleExport};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Validate declarations and exact bytes; a resource export cannot borrow ordinary asset authority.
pub(crate) fn validate_exports(
    manifest: &Manifest,
    files: &BTreeMap<String, Vec<u8>>,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        manifest.module_exports.len() <= 16,
        "Too many native module exports"
    );
    if manifest.module_exports.is_empty() {
        return Ok(());
    }
    anyhow::ensure!(
        manifest.permissions.contains("native.modules")
            && manifest
                .api
                .as_ref()
                .is_some_and(|api| api.required.contains_key("language.modules")),
        "Module exports require language.modules and native.modules approval"
    );
    let mut roots = Vec::<String>::new();
    for (id, export) in &manifest.module_exports {
        anyhow::ensure!(
            plugin_protocol::language_control::valid_export_id(id),
            "Invalid module export name"
        );
        crate::package::validate_relative(&export.root)?;
        crate::package::validate_relative(&export.entry)?;
        plugin_protocol::api::ResourceIdentity::Local {
            path: export.root.clone(),
        }
        .validate()?;
        plugin_protocol::api::ResourceIdentity::Local {
            path: export.entry.clone(),
        }
        .validate()?;
        anyhow::ensure!(
            export.root.len() <= 4096
                && export.entry.len() <= 4096
                && !export.abi.is_empty()
                && export.abi.len() <= 128
                && !export.abi.chars().any(char::is_control),
            "Invalid native module metadata"
        );
        anyhow::ensure!(
            export.engine.len() <= 16
                && export
                    .engine
                    .keys()
                    .all(|id| plugin_protocol::language_control::valid_export_id(id)),
            "Invalid engine constraints"
        );
        let prefix = format!("{}/", export.root);
        anyhow::ensure!(
            roots
                .iter()
                .all(|old| !prefix.starts_with(old) && !old.starts_with(&prefix)),
            "Module export roots must not overlap"
        );
        roots.push(prefix.clone());
        anyhow::ensure!(
            !export.files.is_empty()
                && export.files.len() <= 512
                && hash_valid(&export.sha256)
                && export.files.get(&export.entry) == Some(&export.sha256),
            "Unsealed module entry"
        );
        for (relative, hash) in &export.files {
            crate::package::validate_relative(relative)?;
            plugin_protocol::api::ResourceIdentity::Local {
                path: relative.clone(),
            }
            .validate()?;
            anyhow::ensure!(hash_valid(hash), "Invalid module file digest");
            let bytes = files
                .get(&format!("{prefix}{relative}"))
                .ok_or_else(|| anyhow::anyhow!("Missing module file"))?;
            anyhow::ensure!(
                digest(bytes) == *hash,
                "Module file digest mismatch: {relative}"
            );
        }
        anyhow::ensure!(
            files
                .keys()
                .filter_map(|path| path.strip_prefix(&prefix))
                .all(|relative| export.files.contains_key(relative)),
            "Unlisted module runtime neighbor"
        );
    }
    Ok(())
}

pub(super) fn hash_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub(super) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Enumerate the native closure again at use; installation metadata cannot authorize added files.
pub(super) fn verify_directory(
    root: &std::path::Path,
    export: &ModuleExport,
) -> anyhow::Result<Vec<std::fs::File>> {
    let mut pending = vec![root.to_owned()];
    let mut actual = BTreeMap::new();
    let mut total = 0u64;
    let mut visited = 0;
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            visited += 1;
            anyhow::ensure!(visited <= 1024, "Module directory quota exceeded");
            let kind = entry.file_type()?;
            anyhow::ensure!(
                !crate::native_path::is_reparse(&std::fs::symlink_metadata(entry.path())?),
                "Module links are forbidden"
            );
            if kind.is_dir() {
                pending.push(entry.path());
                continue;
            }
            anyhow::ensure!(kind.is_file(), "Module resource is not a regular file");
            let relative = entry
                .path()
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            let expected = export
                .files
                .get(&relative)
                .ok_or_else(|| anyhow::anyhow!("Unlisted module neighbor"))?;
            let mut options = std::fs::File::options();
            options.read(true);
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                // Keep readers possible while denying replacement, writes and deletion of executable bytes.
                options.share_mode(1);
            }
            let mut file = options.open(entry.path())?;
            total = total
                .checked_add(file.metadata()?.len())
                .ok_or_else(|| anyhow::anyhow!("Module size overflow"))?;
            anyhow::ensure!(total <= 128 * 1024 * 1024, "Module closure exceeds quota");
            let mut hash = Sha256::new();
            use std::io::Read;
            let mut buffer = [0; 8192];
            loop {
                let count = file.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                hash.update(&buffer[..count]);
            }
            anyhow::ensure!(
                format!("{:x}", hash.finalize()) == *expected,
                "Installed module bytes changed"
            );
            actual.insert(relative, file);
        }
    }
    anyhow::ensure!(
        actual.len() == export.files.len(),
        "Module closure is incomplete"
    );
    Ok(actual.into_values().collect())
}
