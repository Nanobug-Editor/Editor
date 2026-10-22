//! Workspace refactors retain immutable native preimages and enter ordinary document-plan admission.
use super::*;
use lsp_types::TextEdit;
use plugin_runtime::plugin_protocol::{
    api::{ErrorCode, Failure},
    language::SourceSnapshot,
    workspace::{Change, Plan},
};
use std::collections::{BTreeMap, BTreeSet};
mod files;

/// A single analysis round retains at most 64 exact native sources and 8 MiB of immutable text.
#[derive(Clone)]
pub(crate) struct Sources {
    pub workspace: PathBuf,
    pub documents: Vec<SourceSnapshot>,
    /// Host-issued monotonic graph/configuration and original-source conditions.
    pub conditions: Vec<Arc<std::sync::atomic::AtomicBool>>,
}

/// Current-source edits retain ordinary immediate rename; related edits require native preview.
pub(crate) enum Rename {
    Document(Vec<TextEdit>),
    Workspace(Plan),
    /// This response supplies discovery metadata only; edits must be recomputed after native admission.
    Sources(Vec<Uri>),
}

/// Every result retains the exact transport incarnation that computed it, including current-source edits.
pub(crate) struct Reply {
    pub proposal: Rename,
    pub origin: plugin_runtime::LanguageFixOrigin,
}

impl LanguageServer {
    /// A declaration cannot grant behavior absent from the original negotiated service incarnation.
    pub(crate) fn supports_workspace_edits(&self) -> bool {
        self.provides_editing()
            && self.service.provider.workspace_edits
            && self
                .service
                .capability("language.editing")
                .is_some_and(|v| v.major == 1 && v.minor >= 2)
    }

    /// Synchronize captured native preimages before semantic rename; every returned target must
    /// match one of them. The native UI checks exact revisions again before presentation/effects.
    pub(crate) fn rename_workspace_for(
        &self,
        document: DocumentLease,
        position: Position,
        new_name: String,
        sources: Sources,
        title: String,
    ) -> anyhow::Result<Reply> {
        ensure!(
            self.supports_workspace_edits(),
            "Workspace refactors were not negotiated"
        );
        ensure!(
            !sources.documents.is_empty()
                && sources.documents.len() <= 64
                && sources
                    .documents
                    .iter()
                    .map(|source| source.text.len())
                    .sum::<usize>()
                    <= 8 * 1024 * 1024,
            Failure::new(
                ErrorCode::LimitExceeded,
                "Workspace refactor source budget exceeded"
            )
        );
        let mut snapshots = BTreeMap::new();
        for snapshot in &sources.documents {
            ensure!(
                snapshot.valid(),
                Failure::new(ErrorCode::InvalidRequest, "Invalid native refactor source")
            );
            let uri = file_uri(&sources.workspace.join(&snapshot.document.path))
                .context("Invalid native source URI")?;
            let uri = canonical_uri(&uri, &sources.workspace)?;
            let lease = self
                .document(&uri)
                .filter(DocumentLease::is_active)
                .context("Native refactor source has no live language lease")?;
            ensure!(
                snapshots.insert(uri, (lease, snapshot.clone())).is_none(),
                "Duplicate refactor source"
            );
        }
        let origin_uri = canonical_uri(&document.uri, &sources.workspace)?;
        let origin = snapshots
            .get(&origin_uri)
            .context("Refactor source is missing")?
            .1
            .document
            .clone();
        ensure!(
            snapshots.len() == 1 || self.service.can_read_workspace(),
            Failure::new(
                ErrorCode::PermissionDenied,
                "Related refactor sources require workspace.read"
            )
        );
        let owner = self.service.clone();
        // Capture physical preimages before semantic analysis, never from a later disk baseline.
        let mut captured = files::FileSources::capture(&owner, &snapshots)?;
        self.edit_request(document, "rename", move |connection, uri| {
            let mut conditions = sources.conditions;
            conditions.push(connection.incarnation_alive.clone());
            let authority = plugin_runtime::LanguageFixOrigin::GuardedLsp {
                service: owner.clone(),
                guards: conditions,
                // All file proposals share this original root. Its proofs follow the queued origin
                // until the Manager has issued its own request, rather than ending at UI dispatch.
                files: captured.proofs.clone(),
            };
            ensure!(
                connection.rename_provider,
                "Language service does not support rename"
            );
            for (lease, snapshot) in snapshots.values() {
                connection.bind_document(lease)?;
                connection.sync_document(lease.uri.clone(), snapshot.text.clone())?;
            }
            let wire = connection.wire_uri(&uri);
            let response = connection.request(
                "textDocument/rename",
                json!({"textDocument":{"uri":wire},"position":position,"newName":new_name}),
            )?;
            let parts = partition(response)?;
            ensure!(
                parts.renames.is_empty() || owner.can_edit_workspace_files(),
                Failure::new(
                    ErrorCode::PermissionDenied,
                    "File refactors were not negotiated"
                )
            );
            let renames = parts
                .renames
                .into_iter()
                .map(|(old, new)| (connection.logical_uri(&old), connection.logical_uri(&new)))
                .collect::<Vec<_>>();
            let single = parts.documents.len() == 1 && renames.is_empty();
            let mut changes = Vec::new();
            let mut seen = BTreeSet::new();
            let mut admitted = Vec::new();
            let mut missing = Vec::new();
            for (target, part) in parts.documents {
                let logical = connection.logical_uri(&target);
                let canonical = canonical_uri(&logical, &sources.workspace)?;
                ensure!(
                    seen.insert(canonical.clone()),
                    Failure::new(ErrorCode::InvalidRequest, "Duplicate workspace edit target")
                );
                if !snapshots.contains_key(&canonical) {
                    // A closed target has no native wire version. Reject versioned/annotated
                    // proposals before opening; the discovery response itself never grants an edit.
                    editing::document_edits(part.clone(), &canonical, -1, |target| {
                        canonical_uri(&connection.logical_uri(target), &sources.workspace)
                            .unwrap_or_else(|_| target.clone())
                    })?;
                    missing.push(canonical.clone());
                }
                admitted.push((canonical, part));
            }
            for (old, _) in &renames {
                let old = canonical_uri(old, &sources.workspace)?;
                if !snapshots.contains_key(&old) && !missing.contains(&old) {
                    missing.push(old);
                }
            }
            if !missing.is_empty() {
                return Ok(Reply {
                    proposal: Rename::Sources(missing),
                    origin: authority,
                });
            }
            for (canonical, part) in admitted {
                let (lease, snapshot) = &snapshots[&canonical];
                ensure!(lease.is_active(), "Refactor target closed");
                let version = connection
                    .diagnostics
                    .document_version(lease.uri.as_str())
                    .context("Refactor target is not synchronized")?;
                let edits = editing::document_edits(part, &canonical, version, |target| {
                    canonical_uri(&connection.logical_uri(target), &sources.workspace)
                        .unwrap_or_else(|_| target.clone())
                })?;
                if edits.is_empty() {
                    continue;
                }
                if single && canonical == origin_uri {
                    return Ok(Reply {
                        proposal: Rename::Document(edits),
                        origin: authority,
                    });
                }
                let (edits, selection) = features::native_edits(&snapshot.text, edits)?;
                changes.push(Change::EditDocument {
                    document: snapshot.document.clone(),
                    edits,
                    selection,
                });
            }
            changes.extend(captured.prepare(&owner, &renames, &sources.workspace)?);
            if changes.is_empty() {
                return Ok(Reply {
                    proposal: Rename::Document(Vec::new()),
                    origin: authority,
                });
            }
            let plan = Plan {
                title,
                origin: Some(origin),
                conditions: sources
                    .documents
                    .into_iter()
                    .map(|source| source.document)
                    .collect(),
                changes,
            };
            plan.validate()?;
            Ok(Reply {
                proposal: Rename::Workspace(plan),
                origin: authority,
            })
        })
    }
}

/// Normalize physical local targets once and reject foreign, missing and non-file targets before use.
fn canonical_uri(uri: &Uri, workspace: &Path) -> anyhow::Result<Uri> {
    let path = url::Url::parse(uri.as_str())?
        .to_file_path()
        .map_err(|_| {
            Failure::new(
                ErrorCode::UnsupportedOperation,
                "Workspace refactors require local source targets",
            )
        })?
        .canonicalize()?;
    ensure!(
        path.is_file() && path.starts_with(workspace.canonicalize()?),
        Failure::new(
            ErrorCode::PermissionDenied,
            "Refactor target escapes workspace"
        )
    );
    file_uri(&path).context("Invalid refactor target URI")
}

/// Standard text and resource representations remain distinct until exact native/disk admission.
/// Existing per-document validation remains authoritative for ranges, versions and unsupported fields.
struct Parts {
    documents: Vec<(Uri, Value)>,
    renames: Vec<(Uri, Uri)>,
}

/// Preserve resource operations separately so they cannot silently disappear from text proposals.
fn partition(response: Value) -> anyhow::Result<Parts> {
    let response = editing::without_empty_representations(response);
    if response.is_null() {
        return Ok(Parts {
            documents: Vec::new(),
            renames: Vec::new(),
        });
    }
    ensure!(
        response.is_object(),
        "Workspace rename requires a workspace edit"
    );
    // Commands are a separate effect and cannot be silently discarded while admitting text edits.
    ensure!(
        response.get("command").is_none(),
        Failure::new(
            ErrorCode::UnsupportedOperation,
            "Workspace edit commands are not supported"
        )
    );
    ensure!(
        response["changes"].is_null() || response["documentChanges"].is_null(),
        Failure::new(
            ErrorCode::InvalidRequest,
            "Workspace edit mixes representations"
        )
    );
    ensure!(
        response["changeAnnotations"].is_null()
            || response["changeAnnotations"]
                .as_object()
                .is_some_and(|a| a.is_empty()),
        Failure::new(
            ErrorCode::UnsupportedOperation,
            "Workspace edit annotations are not supported"
        )
    );
    let mut parts = Vec::new();
    let mut renames = Vec::new();
    if !response["changes"].is_null() {
        let changes = response["changes"]
            .as_object()
            .context("Invalid workspace changes")?;
        ensure!(
            changes.len() <= 64,
            Failure::new(ErrorCode::LimitExceeded, "Too many refactor targets")
        );
        for (uri, edits) in changes {
            let part = json!({"changes":{uri:edits}});
            parts.push((Uri::from_str(uri)?, part));
        }
    }
    if !response["documentChanges"].is_null() {
        let documents = response["documentChanges"]
            .as_array()
            .context("Invalid documentChanges")?;
        ensure!(
            documents.len() <= 64,
            Failure::new(ErrorCode::LimitExceeded, "Too many refactor targets")
        );
        for change in documents {
            if change.get("kind").is_some() {
                ensure!(
                    change["kind"] == "rename"
                        && change.get("annotationId").is_none()
                        && change["options"]
                            .get("overwrite")
                            .is_none_or(|v| v == false)
                        && change["options"]
                            .get("ignoreIfExists")
                            .is_none_or(|v| v == false),
                    Failure::new(
                        ErrorCode::UnsupportedOperation,
                        "Unsupported workspace resource operation or options"
                    )
                );
                let operation: lsp_types::RenameFile = serde_json::from_value(change.clone())?;
                renames.push((operation.old_uri, operation.new_uri));
                continue;
            }
            let uri = change["textDocument"]["uri"]
                .as_str()
                .context("Missing workspace edit URI")?;
            let part = json!({"documentChanges":[change]});
            parts.push((Uri::from_str(uri)?, part));
        }
    }
    Ok(Parts {
        documents: parts,
        renames,
    })
}
