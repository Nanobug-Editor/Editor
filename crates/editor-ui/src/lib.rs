//! Editor-owned visual controls built over gpui-base behavior.

pub mod aligned_editor;
pub mod aligned_pointer;
pub mod aligned_rows;
pub mod button;
pub mod checkbox;
pub mod column_resize;
pub mod comparison_header;
pub mod context_menu;
pub mod definition_popup;
pub mod diagnostic_popup;
pub mod dialog;
pub mod disclosure;
pub mod dock;
pub mod editor;
pub mod editor_canvas;
pub mod file_operation;
pub mod icon;
pub mod input;
pub mod interaction;
pub mod menu;
pub mod message_panel;
pub mod notification;
pub mod overlay;
pub mod popover;
pub mod problems;
pub mod project_dialog;
pub mod reorder_group;
pub mod row_backgrounds;
pub mod scroll;
pub mod scrollbar;
pub mod segmented_tabs;
pub mod shortcut_panel;
pub mod side_tabs;
pub mod source_editor;
pub mod spinner;
pub mod split;
pub mod status_bar;
pub mod status_icon;
pub mod tabs;
pub mod text;
pub mod text_grid;
pub mod textarea;
pub mod tooltip;
pub mod tree;
pub mod window;

/// Install local window presentation after upstream registration and before creating native roots.
/// The final decorator projects aligned-editor input before any root/overlay capture listener.
pub fn init(cx: &mut gpui_kit::App) {
    overlay::init(cx);
    aligned_editor::init(cx);
    scroll::init(cx);
    aligned_pointer::init(cx);
}

pub use button::{Button, ButtonCustomVariant};
pub use checkbox::Checkbox;
pub use comparison_header::{comparison_header_surface, comparison_source_header};
pub use definition_popup::definition_popup;
pub use diagnostic_popup::diagnostic_popup;
pub use dialog::DialogContent;
pub use disclosure::disclosure;
pub use editor::{gutter_separator, synchronize_editor_appearance};
pub use editor_canvas::empty_editor_canvas;
pub use icon::Icon;
pub use input::Input;
pub use notification::Notification;
pub use popover::popover;
pub use reorder_group::{ReorderEvent, ReorderGroup, ReorderGroupState};
pub use scrollbar::{
    install_scrollbar_theme, native_editor as native_editor_scrollbars, vertical_scrollbar,
    vertical_viewport_scrollbar,
};
pub use segmented_tabs::SegmentedTabs;
pub use shortcut_panel::{
    shortcut_footer, shortcut_keycaps, shortcut_list, shortcut_modal, shortcut_pending_hint,
    shortcut_row, shortcut_search, shortcut_tabs,
};
pub use source_editor::{EditorActionElements, EditorActionProjection};
pub use spinner::Spinner;
pub use split::split_container;
pub use status_bar::StatusBar;
pub use status_icon::StatusIcon;
pub use tabs::{
    FileTabTitle, comparison_tab, file_tab, file_tab_actions, preview_tab_badge, tab_strip,
};
pub use text::{RichTextColors, markdown_view, rich_text_view, untrusted_markdown_view};
pub use textarea::Textarea;
pub use tooltip::Tooltip;
pub use tree::{TreeRowAppearance, TreeRowExtras, TreeRowIcon, TreeRowState, tree_row};

/// Fixed native panel/caption height in logical pixels, shared by all product windows.
pub const PANEL_HEADER_HEIGHT: f32 = 28.;

pub mod assets;
pub mod theme;
pub mod typography;

#[cfg(any(test, feature = "test-support"))]
pub mod testing;

// Both consumers compile the same translation catalog; locale selection remains process-wide.
rust_i18n::i18n!("locales", fallback = "en");

#[cfg(test)]
#[path = "../../plugin-runtime/tests/support/ui_demand.rs"]
mod test_demand;
#[cfg(test)]
#[path = "../../plugin-runtime/tests/support/packages.rs"]
mod test_packages;
