//! Rename snapshots become one native preview, including targets first opened for safe recomputation.
use super::*;
use anyhow::Context as _;
use plugin_runtime::plugin_protocol::{
    api,
    diagnostics::Fix,
    workspace::{Change, Plan},
};

/// Immutable text is only a request preimage; EditorState remains the unique mutable text owner.
pub(super) struct Snapshot {
    /// Exact native incarnation, workspace-relative path and revision captured before analysis.
    pub document: DocumentVersion,
    /// Immutable request preimage; changes still go through the existing EditorState.
    pub text: String,
    /// Original connection-owned source lifetime, never a new file or editor permission.
    pub lease: navigation::DocumentLease,
}

impl EditorApp {
    /// Capture only documents already admitted to this shared transport, including explicit targets
    /// from a prior discovery round. Never infer project membership from a filename extension.
    pub(super) fn rename_snapshots(
        &self,
        server: &navigation::LanguageServer,
        cx: &Context<Self>,
    ) -> anyhow::Result<Vec<Snapshot>> {
        let mut snapshots = Vec::new();
        let mut bytes = 0usize;
        for (index, tab) in self.tabs.iter().enumerate() {
            let Some(text) = tab.text.as_ref() else {
                continue;
            };
            let Some(lease) =
                navigation::file_uri(tab.path()).and_then(|uri| server.document(&uri))
            else {
                continue;
            };
            if !tab.path().starts_with(self.workspace.root()) {
                continue;
            }
            let source = text.editor.read(cx).text().to_string();
            bytes = bytes.saturating_add(source.len());
            anyhow::ensure!(
                snapshots.len() < 128 && bytes <= 32 * 1024 * 1024,
                "Rename snapshot quota exceeded"
            );
            snapshots.push(Snapshot {
                document: self.plugin_document_version(index)?,
                text: source,
                lease,
            });
        }
        Ok(snapshots)
    }

    /// Every captured native identity and lease must survive the computation, even for a related
    /// source that contributes types but receives no edit. Undo-to-identical-text still expires it.
    pub(super) fn check_rename_snapshots(
        &self,
        snapshots: &[Snapshot],
        cx: &Context<Self>,
    ) -> anyhow::Result<()> {
        for snapshot in snapshots {
            let index = (0..self.tabs.len())
                .find(|index| {
                    self.plugin_document_version(*index).ok().as_ref() == Some(&snapshot.document)
                })
                .context("Rename dependency changed or closed")?;
            anyhow::ensure!(
                snapshot.lease.is_active()
                    && self.tabs[index]
                        .text
                        .as_ref()
                        .unwrap()
                        .editor
                        .read(cx)
                        .text()
                        .to_string()
                        == snapshot.text,
                "Rename dependency changed or closed"
            );
        }
        Ok(())
    }

    /// Unknown targets are opened read-only for analysis, never edited from the discovery reply.
    /// The next request synchronizes their actual sessions; a second unknown set fails whole.
    pub(super) fn discover_rename_targets(
        &mut self,
        server: &navigation::LanguageServer,
        snapshots: &[Snapshot],
        edits: &[navigation::editing::WorkspaceTextEdits],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<bool> {
        let root = self.workspace.root().canonicalize()?;
        let mut missing = Vec::new();
        for edit in edits {
            let path = url::Url::parse(edit.uri.as_str())?
                .to_file_path()
                .map_err(|_| anyhow::anyhow!("Rename target is not a local file"))?;
            let path = path.canonicalize()?;
            anyhow::ensure!(
                path.starts_with(&root) && path.is_file(),
                "Rename target escapes workspace"
            );
            let relative = path
                .strip_prefix(&root)?
                .to_string_lossy()
                .replace('\\', "/");
            api::ResourceIdentity::Local {
                path: relative.clone(),
            }
            .validate()?;
            // Reject redirected descendants before any ordinary open_file read. Runtime admission
            // repeats its stronger native ancestor-pinning checks before preview and effects.
            let mut ancestor = root.clone();
            for part in Path::new(&relative).components() {
                ancestor.push(part);
                anyhow::ensure!(
                    !std::fs::symlink_metadata(&ancestor)?
                        .file_type()
                        .is_symlink(),
                    "Linked rename targets are unsupported"
                );
            }
            if !snapshots
                .iter()
                .any(|snapshot| snapshot.document.path == relative)
            {
                anyhow::ensure!(
                    server.fix_origin().can_read_workspace(),
                    "Opening rename targets requires workspace.read"
                );
                anyhow::ensure!(
                    !std::fs::metadata(&path)?.permissions().readonly(),
                    "Rename target is readonly"
                );
                missing.push(path);
            }
        }
        if missing.is_empty() {
            return Ok(false);
        }
        let previous = self.active_path.clone();
        for path in missing {
            self.open_file(path.clone(), window, cx);
            let index = self
                .tabs
                .iter()
                .position(|tab| tab.path() == path)
                .context("Could not open rename target for preview")?;
            anyhow::ensure!(
                self.tabs[index].text.is_some(),
                "Rename target is not a text document"
            );
            let language = self.language_for_path(&path);
            server.open_document_for_language(
                navigation::file_uri(&path).context("Invalid rename file URI")?,
                &language,
            );
        }
        if let Some(previous) = previous {
            self.open_file(previous, window, cx);
        }
        Ok(true)
    }

    /// The entire response is converted before submission; no accepted prefix can mutate a session.
    pub(super) fn rename_plan(
        &self,
        document: &DocumentVersion,
        title: String,
        conditions: Vec<DocumentVersion>,
        snapshots: &[Snapshot],
        edits: Vec<navigation::editing::WorkspaceTextEdits>,
    ) -> anyhow::Result<Fix> {
        let mut changes = Vec::new();
        for target in edits {
            let source = snapshots
                .iter()
                .find(|source| source.lease.uri == target.uri)
                .context("Rename target was not captured before computation")?;
            let Some(edit) = navigation::editing::merge_edits(&source.text, &target.edits)? else {
                continue;
            };
            let start = byte_at_position(&source.text, edit.range.start)?;
            let end = byte_at_position(&source.text, edit.range.end)?;
            let caret = start + edit.new_text.len();
            changes.push(Change::EditDocument {
                document: source.document.clone(),
                edits: vec![api::TextEdit {
                    range: api::TextRange { start, end },
                    text: edit.new_text,
                }],
                selection: api::TextRange {
                    start: caret,
                    end: caret,
                },
            });
        }
        let plan = Plan {
            title: title.clone(),
            origin: Some(document.clone()),
            conditions,
            changes,
        };
        plan.validate()?;
        Ok(Fix { title, plan })
    }
}

use crate::language::navigation::editing::byte_at_position;
