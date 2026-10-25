//! Terminal sessions and terminal-only split identities, independent from any host application.

use crate::{Engine, Profile};
use serde::{Deserialize, Serialize};

/// Terminal state and caller-owned task metadata share one stable identity, never a process handle.
/// `T` is integration data; parsing, navigation and rendering do not inspect it.
pub struct Session<T = ()> {
    /// Nonzero identity assigned by the host; never reused for a different native child.
    pub id: u64,
    /// User-visible retained name, independent from transient OSC titles.
    pub name: String,
    /// Selected shell program and argument vector.
    pub profile: Profile,
    /// Last known process working directory.
    pub cwd: String,
    /// The only parsed grid, history, selection and Vi/search state for this session.
    pub engine: Engine,
    /// The host has accepted a native process launch for this session.
    pub launched: bool,
    /// The host has observed final process exit.
    pub exited: bool,
    /// Retained history is restored; passive layout/focus must not launch a Shell.
    pub restored: bool,
    /// Optional host task association, never interpreted by terminal algorithms.
    pub task: Option<T>,
}

/// A flat, terminal-only resizable set of session leaves with one shared orientation.
#[derive(Clone, Serialize, Deserialize)]
pub struct PaneLayout {
    /// True stacks rows; false arranges columns.
    pub vertical: bool,
    /// Stable session identities in visual order. The host validates ownership when restoring.
    pub sessions: Vec<u64>,
    /// Replacing a hidden tab affects the last focused leaf only.
    #[serde(skip)]
    pub focused: u64,
}

/// Measured native geometry for one session; siblings never borrow each other's PTY dimensions.
#[derive(Clone, Copy)]
pub struct ViewGeometry {
    /// Full viewport width in logical pixels, including the terminal's padding.
    pub width: f32,
    /// Full viewport height in logical pixels.
    pub height: f32,
    /// Native advance of one terminal cell, in logical pixels.
    pub cell_width: f32,
    /// Native line height in logical pixels.
    pub cell_height: f32,
}
