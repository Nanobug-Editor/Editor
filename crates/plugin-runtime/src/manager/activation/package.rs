//! Saved registry entries are hints; restored execution revalidates actual immutable package assets.
use super::super::*;
use std::io::Read;

/// Reject owner-directory aliases and unsafe relative assets before reading saved metadata or code.
pub(super) fn assets(root: &Path, entry: &Installed) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(
        entry.digest.len() == 64 && entry.digest.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Invalid installed digest"
    );
    anyhow::ensure!(
        !entry.manifest.id.is_empty()
            && !entry.manifest.id.starts_with('.')
            && entry
                .manifest
                .id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || b".-".contains(&byte)),
        "Invalid installed identity"
    );
    let assets = root
        .canonicalize()?
        .join("packages")
        .join(&entry.manifest.id)
        .join(&entry.digest);
    anyhow::ensure!(
        assets.canonicalize()? == assets && assets.is_dir(),
        "Installed package owner was redirected"
    );
    Ok(assets)
}

/// Static discovery checks the actual TOML's host floor without compiling grammar or WASM resources.
pub(super) fn metadata(root: &Path, entry: &Installed) -> anyhow::Result<()> {
    crate::capabilities::require_current(&entry.manifest)?;
    if let Some(contributions) = &entry.manifest.contributions {
        let assets = assets(root, entry)?;
        let path = crate::instance::safe_path(&assets, contributions, true)?;
        let file = std::fs::File::open(path)?;
        anyhow::ensure!(
            file.metadata()?.len() <= 1024 * 1024,
            "Installed contribution metadata exceeds quota"
        );
        let mut bytes = Vec::new();
        file.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
        anyhow::ensure!(
            bytes.len() <= 1024 * 1024,
            "Installed contribution metadata exceeds quota"
        );
        crate::package::compatible_contributions(&entry.manifest, &bytes)?;
    }
    Ok(())
}

/// Background execution uses archive admission again, including actual host-version and resource guards.
/// Traversal is bounded before allocation and rejects symlinks rather than copying foreign package assets.
pub(super) fn read(root: &Path, entry: &Installed) -> anyhow::Result<(PathBuf, Package)> {
    let assets = assets(root, entry)?;
    let mut directories = vec![assets.clone()];
    let mut files = BTreeMap::new();
    let mut total = 0u64;
    let mut visited = 0;
    while let Some(directory) = directories.pop() {
        for item in std::fs::read_dir(directory)? {
            let item = item?;
            visited += 1;
            anyhow::ensure!(visited <= 1024, "Installed package traversal exceeds quota");
            let metadata = item.path().symlink_metadata()?;
            anyhow::ensure!(
                !metadata.file_type().is_symlink(),
                "Installed package contains a symbolic link"
            );
            let path = item.path().canonicalize()?;
            anyhow::ensure!(
                path.starts_with(&assets),
                "Installed resource escaped its owner"
            );
            if metadata.is_dir() {
                directories.push(path);
                continue;
            }
            anyhow::ensure!(
                metadata.is_file() && files.len() < 512,
                "Invalid or oversized installed package"
            );
            total = total
                .checked_add(metadata.len())
                .ok_or_else(|| anyhow::anyhow!("Installed quota overflow"))?;
            anyhow::ensure!(
                total <= 128 * 1024 * 1024,
                "Installed package exceeds expanded quota"
            );
            let relative = path
                .strip_prefix(&assets)?
                .to_string_lossy()
                .replace('\\', "/");
            let mut bytes = Vec::new();
            std::fs::File::open(&path)?
                .take(metadata.len() + 1)
                .read_to_end(&mut bytes)?;
            anyhow::ensure!(
                bytes.len() as u64 == metadata.len(),
                "Installed resource changed while reading"
            );
            files.insert(relative, bytes);
        }
    }
    let package = Package::from_files(files)?;
    // ZIP and directory content digests use different encodings; compare the actual normalized manifest,
    // while the version-directory identity remains the original admitted ZIP digest.
    anyhow::ensure!(
        serde_json::to_value(&package.manifest)? == serde_json::to_value(&entry.manifest)?,
        "Installed manifest differs from its registry entry"
    );
    Ok((assets, package))
}
