//! Preserve the complete standard text edit set while rejecting unsupported effects before admission.
use super::*;
use std::collections::BTreeSet;

/// Logical target identities never grant file access; native workspace admission owns that decision.
pub(crate) struct WorkspaceTextEdits {
    pub uri: Uri,
    pub edits: Vec<TextEdit>,
}

/// Split only after validating the whole wire shape. Every constituent retains annotations and its
/// original wire version for the existing strict single-target decoder, avoiding serde field loss.
pub(super) fn decode(
    connection: &LanguageServerConnection,
    response: Value,
) -> anyhow::Result<Vec<WorkspaceTextEdits>> {
    ensure!(response.is_object(), "Workspace edit must be an object");
    ensure!(
        response.get("command").is_none(),
        "Workspace edit commands are unsupported"
    );
    ensure!(
        response.get("changes").is_none() || response.get("documentChanges").is_none(),
        "Workspace edit mixes representations"
    );
    ensure!(
        response.get("changeAnnotations").is_none_or(
            |value| value.is_null() || value.as_object().is_some_and(|map| map.is_empty())
        ),
        "Workspace edit annotations are unsupported"
    );
    let mut targets = Vec::new();
    if let Some(changes) = response.get("changes").filter(|value| !value.is_null()) {
        for (uri, edits) in changes.as_object().context("Invalid workspace changes")? {
            targets.push((uri.parse::<Uri>()?, json!({"changes":{uri:edits}})));
        }
    }
    if let Some(changes) = response
        .get("documentChanges")
        .filter(|value| !value.is_null())
    {
        for change in changes.as_array().context("Invalid documentChanges")? {
            ensure!(
                change.get("kind").is_none(),
                "Workspace resource operations are unsupported"
            );
            let uri = change["textDocument"]["uri"]
                .as_str()
                .context("Missing edit target")?
                .parse::<Uri>()?;
            targets.push((uri, json!({"documentChanges":[change]})));
        }
    }
    ensure!(targets.len() <= 128, "Too many workspace edit targets");
    let mut seen = BTreeSet::new();
    targets
        .into_iter()
        .map(|(wire, edit)| {
            let uri = connection.logical_uri(&wire);
            ensure!(
                seen.insert(uri.as_str().to_owned()),
                "Repeated workspace edit target"
            );
            // Unknown null-version targets can be discovered, then opened and recomputed by the UI.
            // A numeric version without a synchronized document is never accepted as a baseline.
            let version = connection.diagnostics.document_version(uri.as_str());
            if version.is_none() {
                ensure!(
                    edit["documentChanges"][0]["textDocument"]["version"].is_null(),
                    "Unsynchronized versioned edit target"
                );
            }
            let edits = document_edits(edit, &uri, version.unwrap_or(0), |target| {
                connection.logical_uri(target)
            })?;
            Ok(WorkspaceTextEdits { uri, edits })
        })
        .collect()
}
