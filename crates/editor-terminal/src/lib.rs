//! Native terminal implementation shared by host integrations.
//!
//! This crate owns the only screen/history/selection state, terminal parsing,
//! recovery, input protocols, navigation and native drawing. Adopted VT/grid algorithms are
//! integrated as private modules; there is no upstream terminal crate or separate fork.
//! Generic GPUI input/IME/scrollbar
//! primitives live in editor-ui; workspace, Run/Debug and process authority remain with the host.
//! No terminal code depends on a plugin CanvasView, guest scene or serialized paint/input protocol.

pub mod config;
mod coordinates;
pub mod engine;
mod events;
mod grid;
pub mod input;
mod screen;
pub mod search_view;
mod selection;
pub mod session;
pub mod shell;
mod vi;

pub use config::{Palette, Profile, Settings, TabPosition};
pub use engine::{Colors, Engine, GridSize, Link, SavedGrid, Search};
pub use search_view::{SearchAction, SearchBar, SearchModel};
pub use session::{PaneLayout, Session, ViewGeometry};

// Share the existing bilingual catalog; terminal errors retain the host's locale selection.
rust_i18n::i18n!("../editor-ui/locales", fallback = "en");
