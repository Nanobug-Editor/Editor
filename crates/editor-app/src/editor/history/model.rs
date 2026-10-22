//! Bounded timeline presentation and immutable source guards for native recovery.

use super::*;
use editor_history::{Cursor, Lease, LegacyCursor, LegacyEntry, Location, Version};
use gpui_base::input::InputState;

/// Category ownership is explicit; a standalone source never acquires the current workspace.
#[derive(Clone, Debug)]
pub(crate) enum Category {
    File(PathBuf),
    Workspace,
    Deleted,
    Standalone,
    Legacy,
    All,
}

impl Category {
    /// Stable localized category labels, independent of the selected row's event source.
    pub(super) fn label(&self) -> String {
        t!(match self {
            Self::File(_) => "history.file",
            Self::Workspace => "history.workspace",
            Self::Deleted => "history.deleted",
            Self::Standalone => "history.standalone",
            Self::Legacy => "history.legacy",
            Self::All => "history.all",
        })
        .to_string()
    }
}

/// Timeline rows contain metadata only; rendering never loads a compressed content object.
#[derive(Clone, Debug)]
pub(crate) enum Item {
    Version(Version),
    Legacy(LegacyEntry),
}

impl Item {
    /// Source path or the limited filename actually available in the retired format.
    pub(super) fn title(&self) -> String {
        match self {
            Self::Version(row) => editor_core::display_path(&row.path).to_string(),
            Self::Legacy(row) => row.filename.clone(),
        }
    }
    /// Keep the filename visible in narrow rows; the full historical path remains in its tooltip.
    pub(super) fn path_label(&self) -> String {
        match self {
            Self::Version(row) => {
                let path = row
                    .workspace
                    .as_ref()
                    .and_then(|root| row.path.strip_prefix(root).ok())
                    .unwrap_or_else(|| row.path.file_name().map(Path::new).unwrap_or(&row.path));
                editor_core::display_path(path).to_string()
            }
            Self::Legacy(_) => self.title(),
        }
    }
    /// Timestamp uses local presentation while persistence remains exact UTC milliseconds.
    pub(super) fn time(&self) -> String {
        let at = match self {
            Self::Version(row) => Some(row.at_ms),
            Self::Legacy(row) => row.at_ms,
        };
        at.and_then(chrono::DateTime::from_timestamp_millis)
            .map(|time| {
                time.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M:%S%.3f")
                    .to_string()
            })
            .unwrap_or_else(|| t!("history.unknown_time").to_string())
    }
    /// Event labels distinguish protected drafts, actual saves, observed disk and structural changes.
    pub(super) fn source(&self) -> String {
        match self {
            Self::Version(row) => t!(format!("history.source.{}", row.reason.as_str())).to_string(),
            Self::Legacy(_) => t!("history.legacy").to_string(),
        }
    }
    /// Only rows with retained content offer native text preview/recovery.
    pub(super) fn has_content(&self) -> bool {
        match self {
            Self::Version(row) => row.size.is_some(),
            Self::Legacy(_) => true,
        }
    }
}

/// Page cursors use the source's stable indexed order, including duplicate timestamps.
#[derive(Clone)]
pub(super) enum Page {
    Version(Cursor),
    Legacy(LegacyCursor),
}

/// Native target state at the time a selected historical preview became visible.
#[derive(Clone)]
pub(super) struct Target {
    /// A reopened tab at the same path is a different recovery target.
    pub id: u64,
    /// Session and capability revisions also reject silent reload/provider changes.
    pub revision: u64,
    pub capability: u64,
    /// Includes notify-only IME preedit rather than relying on a Change event alone.
    pub identity: super::super::native_text::NativeTextIdentity,
}

/// One selected source and the disk/native preconditions against which it was previewed.
pub(super) struct Selection {
    /// Exact indexed source; unsupported legacy data still permits raw export.
    pub item: Item,
    /// Changing the selected native tab after preview revokes this proposed action.
    pub active_document: Option<u64>,
    /// Active compressed source remains readable through preview and restoration.
    pub _lease: Option<Lease>,
    /// Current/last source location, never an obsolete rename path from the selected version.
    pub location: Option<Location>,
    /// Complete disk reads happen in the background; final publication compares this native stamp.
    pub disk: Option<platform_windows::history::FileStamp>,
    /// None means the target was not open when this preview was established.
    pub target: Option<Target>,
}

/// A confirmed cleanup preview is invalidated by changes to its durable metadata generation.
pub(super) struct ClearConfirmation {
    /// Human-readable scope displayed before the one destructive confirmation.
    pub label: String,
    /// Exact library selection and generation-aware result.
    pub scope: editor_history::ClearScope,
    pub preview: editor_history::Cleanup,
}

/// Metadata retained for a restored deleted document; its text lives only in EditorState.
pub(super) struct PendingCreate {
    /// After the first successful disk write, later saves use ordinary replacement authority.
    pub needs_create: bool,
    /// Existing workspace/root under which missing parents may be recreated during Save.
    pub root: PathBuf,
    /// Original lineage only when restoring the source's actual deleted path.
    pub file: Option<editor_history::FileId>,
    /// Independent copies retain a source link without merging logical identities.
    pub origin: editor_history::VersionId,
}

/// The history view retains one page, one readonly content source, and derived comparison geometry.
pub(crate) struct Panel {
    /// Current category and user-entered literal path/filename filter.
    pub(super) category: Category,
    pub(super) filter: Entity<InputState>,
    /// Input events reset pagination and cancel old asynchronous list/preview results.
    pub(super) _filter_subscription: Subscription,
    /// At most fifty rows from the direct metadata index.
    pub(crate) rows: Vec<Item>,
    /// The selected row position, kept separate from the asynchronous preview resource.
    pub(super) selected: Option<usize>,
    pub(crate) preview: Option<Entity<EditorState>>,
    pub(super) selection: Option<Selection>,
    /// The first page has no cursor; previous pages retain metadata cursors rather than rows/content.
    pub(super) pages: Vec<Option<Page>>,
    /// A visible pending state and cancellable generation cover listing, preview and recovery.
    pub(super) busy: bool,
    pub(super) error: Option<String>,
    pub(super) notice: Option<String>,
    /// Native Base focus returns to its actual prior owner on cancellation/close.
    pub(super) return_focus: Option<FocusHandle>,
    /// A document-origin view returns to the current document, including after tab changes/closure.
    pub(super) return_source: bool,
    pub(super) scroll: ScrollHandle,
    pub(super) clear: Option<ClearConfirmation>,
    pub(super) comparison: Option<super::comparison::Comparison>,
}
