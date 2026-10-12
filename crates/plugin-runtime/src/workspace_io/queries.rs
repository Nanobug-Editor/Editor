//! Bounded readonly disk metadata and stateless listing/discovery cursors share the sealed file roots.
use super::{
    io, limit,
    snapshot::{Budget, Snapshot},
};
use crate::AuthorizedFile;
use plugin_protocol::{
    api::{ErrorCode, Failure, FileQuery},
    workspace::{Cursor, Metadata, Page},
};
use std::time::{Duration, Instant};

/// Read metadata or bounded disk bytes after exact root validation; the host overlays authorized buffers.
pub fn read(file: &AuthorizedFile, contents: bool) -> Result<(Metadata, Vec<u8>), Failure> {
    let _guard = file.guard()?;
    if !file.can_read() || file.is_save_intent() {
        return Err(Failure::new(
            ErrorCode::PermissionDenied,
            "Target has no read authority",
        ));
    }
    let snapshot = Snapshot::capture(file.path(), &mut Budget::default(), 0, &|| !file.is_live())?;
    let bytes = if contents {
        snapshot.bytes()?.to_vec()
    } else {
        Vec::new()
    };
    if bytes.len() > 1024 * 1024 {
        return Err(limit("File read exceeds 1 MiB"));
    }
    Ok((
        Metadata {
            target: file.target().clone(),
            stamp: snapshot.stamp,
            document: None,
        },
        bytes,
    ))
}

/// Paged discovery retains no cursor state and never treats a glob as extra filesystem authority.
pub fn discover(
    file: &AuthorizedFile,
    query: Option<&FileQuery>,
    page_size: u16,
    cursor: Option<&Cursor>,
) -> Result<Page, Failure> {
    let _guard = file.guard()?;
    if !file.can_read() || file.is_save_intent() || !file.path().is_dir() {
        return Err(Failure::new(
            ErrorCode::PermissionDenied,
            "Directory read authority is required",
        ));
    }
    let (paths, skipped, ignore_version) = if let Some(query) = query {
        let (matches, version) = crate::instance::file_discovery::find_versioned(
            file.path(),
            query,
            Instant::now() + Duration::from_secs(10),
        )?;
        (matches.paths, matches.skipped, version)
    } else {
        let mut paths = Vec::new();
        for entry in std::fs::read_dir(file.path()).map_err(io)? {
            let entry = entry.map_err(io)?;
            if paths.len() >= 4096 {
                return Err(limit("Directory listing exceeds 4096 entries"));
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| Failure::new(ErrorCode::InvalidPath, "Non-UTF-8 name"))?;
            paths.push(name);
        }
        (paths, Vec::new(), String::new())
    };
    let mut entries = Vec::new();
    let mut budget = Budget::default();
    let mut digest = sha2::Sha256::new();
    use sha2::Digest;
    digest.update(ignore_version);
    digest.update(serde_json::to_vec(&query).expect("query serializes"));
    for relative in paths {
        let path = file.path().join(&relative);
        let mut target = file.target().clone();
        target.path = if target.path.is_empty() {
            relative
        } else {
            format!("{}/{}", target.path, relative)
        };
        target.validate()?;
        let snapshot = Snapshot::capture(&path, &mut budget, 0, &|| !file.is_live())?;
        digest.update(&target.path);
        digest.update(format!("{:?}", snapshot.stamp));
        entries.push(Metadata {
            target,
            stamp: snapshot.stamp,
            document: None,
        });
    }
    entries.sort_by(|left, right| left.target.path.cmp(&right.target.path));
    let version = format!("{:x}", digest.finalize());
    if let Some(cursor) = cursor {
        if cursor.version != version {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Discovery changed; restart from the first page",
            ));
        }
    }
    let after = cursor.map(|cursor| cursor.after.as_str()).unwrap_or("");
    let mut remaining = entries
        .into_iter()
        .filter(|entry| entry.target.path.as_str() > after);
    let page: Vec<_> = remaining.by_ref().take(page_size as usize).collect();
    let next = remaining.next().map(|_| Cursor {
        version,
        after: page.last().expect("positive page size").target.path.clone(),
    });
    Ok(Page {
        entries: page,
        next,
        skipped,
    })
}
