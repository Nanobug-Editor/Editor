//! File identity changes retain pre-analysis disk stamps and ordinary root-scoped IO authority.
use super::*;
use plugin_runtime::{AuthorizedFile, LanguageService, plugin_protocol::workspace::FileStamp};

/// Bounded proofs contain no editable source or private recovery history.
pub(super) struct FileSources {
    stamps: BTreeMap<Uri, FileStamp>,
    pub(super) proofs: Vec<AuthorizedFile>,
}
impl FileSources {
    /// Every admitted native source gets its physical preimage before the semantic request starts.
    pub(super) fn capture(
        service: &LanguageService,
        sources: &BTreeMap<Uri, (DocumentLease, SourceSnapshot)>,
    ) -> anyhow::Result<Self> {
        let mut result = Self {
            stamps: BTreeMap::new(),
            proofs: Vec::new(),
        };
        if service.can_edit_workspace_files() {
            for (uri, (_, snapshot)) in sources {
                let file = service.refactor_file(&snapshot.document.path)?;
                let (metadata, _) = plugin_runtime::workspace_io::read(&file, false)?;
                ensure!(
                    matches!(metadata.stamp, FileStamp::File { .. }),
                    "Refactor source is not a physical file"
                );
                result.stamps.insert(uri.clone(), metadata.stamp);
                result.proofs.push(file);
            }
        }
        Ok(result)
    }

    /// Independent RenameFile operations enter the existing ordered plan after native text edits.
    /// Rename chains and collisions fail whole; no path guess or overwriting option is accepted.
    pub(super) fn prepare(
        &mut self,
        service: &LanguageService,
        renames: &[(Uri, Uri)],
        workspace: &Path,
    ) -> anyhow::Result<Vec<Change>> {
        let mut result = Vec::new();
        let mut seen = BTreeSet::new();
        let root = workspace.canonicalize()?;
        for (old, new) in renames {
            let old = canonical_uri(old, workspace)?;
            let expected_source = self
                .stamps
                .get(&old)
                .context("Missing original file preimage")?
                .clone();
            let old_path = url::Url::parse(old.as_str())?
                .to_file_path()
                .map_err(|_| anyhow!("Invalid source URI"))?;
            let new_path = url::Url::parse(new.as_str())?
                .to_file_path()
                .map_err(|_| anyhow!("Invalid target URI"))?;
            let old_path = old_path.canonicalize()?;
            let new_path = new_path
                .parent()
                .context("Missing target parent")?
                .canonicalize()?
                .join(new_path.file_name().context("Missing target name")?);
            ensure!(
                seen.insert(old_path.clone()) && seen.insert(new_path.clone()),
                Failure::new(
                    ErrorCode::UnsupportedOperation,
                    "Overlapping resource renames"
                )
            );
            let relative = |path: &Path| -> anyhow::Result<String> {
                Ok(path
                    .strip_prefix(&root)?
                    .to_str()
                    .context("Non-UTF-8 refactor target")?
                    .replace('\\', "/"))
            };
            let source = service.refactor_file(&relative(&old_path)?)?;
            let target = service.refactor_file(&relative(&new_path)?)?;
            let (metadata, _) = plugin_runtime::workspace_io::read(&target, false)?;
            ensure!(
                metadata.stamp == FileStamp::Missing,
                Failure::new(ErrorCode::StaleRevision, "Rename target already exists")
            );
            result.push(Change::Rename {
                source: source.target().clone(),
                target: target.target().clone(),
                expected_source,
            });
            self.proofs.extend([source, target]);
        }
        Ok(result)
    }
}
