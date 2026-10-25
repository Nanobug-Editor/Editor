//! The owned terminal screen supplies parsing, history, selection and native presentation.

// Host integrations need snapshot cells, search coordinates and reply data; screen storage stays private.
use crate::{
    coordinates::Side,
    events::EventListener,
    grid::{Dimensions, Scroll},
    screen::{Screen, ScreenConfig, cell::Flags},
    selection::{Selection, SelectionType},
};
pub use crate::{
    coordinates::{Column, Line, Point},
    events::{Event, WindowSize},
    screen::{
        ClipboardType, TerminalMode,
        cell::{Cell, Flags as CellFlags},
    },
};
use editor_ui::text_grid::{
    GridDrawing, GridFont, GridPaint as Paint, GridRect as Rect, GridScroll as ScrollRange,
};
use rust_i18n::t;
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    rc::Rc,
};
pub use vte::ansi::{CursorShape, Rgb};
use vte::{
    ansi::Processor,
    ansi::{ClearMode, Color, CursorStyle, Handler, NamedColor},
};

mod drawing;
mod links;
mod navigation;
mod recovery;
mod search;
pub use links::Link;
pub use search::Search;

#[cfg(test)]
mod feature_tests;
#[cfg(test)]
mod tests;

/// Actual character dimensions, independent from dock and sidebar pixels.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct GridSize {
    /// Character columns; native host viewports validate 2–1000.
    pub columns: usize,
    /// Visible character rows, excluding retained history; native host viewports validate 1–500.
    pub rows: usize,
}
impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.rows
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn columns(&self) -> usize {
        self.columns
    }
}

/// Reply events stay on the view thread; process writes are queued to the native supervisor.
#[derive(Clone, Default)]
struct Listener(Rc<RefCell<Vec<Event>>>);
impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        self.0.borrow_mut().push(event);
    }
}

/// Saved logical cells contain no OS resources, executable state or debugger memory.
#[derive(Serialize, Deserialize)]
pub struct SavedGrid {
    /// Width at the time of the snapshot, validated before restoring any cells.
    pub columns: usize,
    /// Live screen height at snapshot time, excluding history.
    pub rows: usize,
    /// Trim trailing default cells, never actual content; history is oldest-first and bounded.
    pub lines: VecDeque<Vec<Cell>>,
    /// Live cursor line and column in the saved screen, both zero-based.
    pub cursor: (i32, usize),
    /// The next printable character must wrap before insertion.
    pub input_needs_wrap: bool,
    /// Rows scrolled above the live bottom.
    pub offset: usize,
}

/// The sole terminal state, including parser, history, search and read-only navigation.
pub struct Engine {
    term: Screen<Listener>,
    parser: Processor,
    listener: Listener,
    history_limit: usize,
    /// Only ConPTY's first inheritance query may reuse an empty restored prompt.
    bootstrap: Option<Vec<u8>>,
    bootstrap_line_start: bool,
    /// ConPTY owns a fresh screen; historical rows must not be pulled into it during resize.
    conpty_screen: bool,
    /// Explicit history scrolling pauses following; user input or returning to the bottom resumes it.
    follow_screen: bool,
    metadata_parser: vte::Parser,
    metadata: super::shell::Metadata,
    /// Search is a view operation on the sole Alacritty grid, never another text buffer.
    pub search: Search,
    /// Find-bar visibility controls highlights without discarding its retained query.
    pub search_visible: bool,
    /// The view clock controls blinking; the IME anchor remains stable while the caret is off.
    pub cursor_visible: bool,
    /// Optional terminal-provided title, separate from the user's retained tab name.
    pub title: Option<String>,
    /// A visual notification remains pending until the user focuses or types in this session.
    pub bell: bool,
    /// Logical cell-height multiplier used by the native renderer and its IME measurement.
    pub line_height: f32,
    /// Last user default, retained while history settings change independently.
    default_cursor: CursorStyle,
}

/// Editor defaults and user overrides are resolved once per frame, outside the cell loop.
pub struct Colors {
    /// Default viewport background RGB.
    pub background: u32,
    /// Default text RGB.
    pub foreground: u32,
    /// Effective caret and active-search marker RGB.
    pub cursor: u32,
    /// Selection and passive search-match background RGB.
    pub selection: u32,
    /// Normal and bright ANSI colors in core index order.
    pub ansi: [u32; 16],
    /// Optional host palette entries; per-process OSC colors take precedence.
    pub overrides: BTreeMap<usize, u32>,
}

impl Engine {
    /// Styled spaces, wide-glyph spacers and wrap metadata belong to retained output too.
    fn has_content(cell: &Cell) -> bool {
        cell != &Cell::default()
    }

    /// Translate a visible cell to native screen coordinates, rejecting retained history rows.
    pub fn mouse_position(&self, row: usize, column: usize) -> Option<(usize, usize)> {
        let row = row.checked_sub(self.offset())?;
        (row < self.term.screen_lines() && column < self.term.columns()).then_some((row, column))
    }

    /// Create a steady-beam terminal with visible `size` and at most `history` retained rows.
    /// Callers must supply nonzero dimensions; the host validates its bounded viewport before calling.
    pub fn new(size: GridSize, history: usize) -> Self {
        let listener = Listener::default();
        let config = ScreenConfig {
            scrolling_history: history,
            default_cursor_style: CursorStyle {
                shape: CursorShape::Beam,
                blinking: false,
            },
            vi_mode_cursor_style: Some(CursorStyle {
                shape: CursorShape::Block,
                blinking: false,
            }),
            ..ScreenConfig::default()
        };
        Self {
            term: Screen::new(config, &size, listener.clone()),
            parser: Processor::new(),
            listener,
            history_limit: history,
            bootstrap: None,
            bootstrap_line_start: false,
            conpty_screen: false,
            follow_screen: true,
            metadata_parser: vte::Parser::new(),
            metadata: Default::default(),
            search: Search::default(),
            search_visible: false,
            cursor_visible: true,
            title: None,
            bell: false,
            line_height: 1.45,
            default_cursor: CursorStyle {
                shape: CursorShape::Beam,
                blinking: false,
            },
        }
    }
    /// Parsing never touches native handles or UI widgets; replies are returned to the owner.
    pub fn process(&mut self, bytes: &[u8]) -> Vec<Event> {
        const QUERY: &[u8] = b"\x1b[6n";
        let buffered = if let Some(mut initial) = self.bootstrap.take() {
            initial.extend_from_slice(bytes);
            if initial.len() < QUERY.len() && QUERY.starts_with(&initial) {
                self.bootstrap = Some(initial);
                return vec![];
            }
            if initial.starts_with(QUERY) {
                let start = self.start_restored_screen();
                self.listener
                    .send_event(Event::PtyWrite(format!("\x1b[{};1R", start.0 + 1)));
                self.bootstrap_line_start = true;
                Some(initial.split_off(QUERY.len()))
            } else {
                Some(initial)
            }
        } else {
            None
        };
        let bytes = buffered.as_deref().unwrap_or(bytes);
        if self.bootstrap_line_start && !bytes.is_empty() {
            // ConPTY redraw starts at the logical prompt's first row, not its last wrapped row.
            // Measurement may have reflowed rows while native startup was pending. Resolve the
            // current start again rather than retaining a stale pre-resize screen coordinate.
            let start = self.visible_prompt_start();
            let cursor = &mut self.term.grid_mut().cursor;
            cursor.point = Point::new(start, Column(0));
            cursor.input_needs_wrap = false;
            self.bootstrap_line_start = false;
        }
        self.parser.advance(&mut self.term, bytes);
        self.metadata_parser.advance(&mut self.metadata, bytes);
        // Output may scroll old coordinates or switch screens. Retire the old match without
        // scrolling the user's view; the next explicit navigation searches the current grid.
        self.search.current = None;
        self.reveal_native_screen();
        let events: Vec<_> = self.listener.0.borrow_mut().drain(..).collect();
        for event in &events {
            match event {
                Event::Title(title) => {
                    self.title = Some(
                        title
                            .chars()
                            .filter(|ch| !ch.is_control())
                            .take(256)
                            .collect(),
                    )
                }
                Event::ResetTitle => self.title = None,
                Event::Bell => self.bell = true,
                _ => {}
            }
        }
        events
    }
    /// Directory metadata never changes the grid and cannot execute a program itself.
    pub fn take_cwd(&mut self) -> Option<String> {
        self.metadata.cwd.take()
    }
    /// Release an incomplete synchronized update once `now` reaches its core-owned deadline.
    /// Returns whether bytes were released. Consume queued replies with `process(&[])` on the same
    /// owning thread; a child that stops writing must never leave its last frame buffered forever.
    pub fn expire_sync(&mut self, now: std::time::Instant) -> bool {
        if self
            .parser
            .sync_timeout()
            .sync_timeout()
            .is_some_and(|deadline| deadline <= now)
        {
            self.parser.stop_sync(&mut self.term);
            true
        } else {
            false
        }
    }
    /// Reflow to `size`, preserving the independent ConPTY live-screen/history boundary.
    pub fn resize(&mut self, size: GridSize) {
        if self.size() == size {
            return;
        }
        self.search.current = None;
        if self.conpty_screen
            && self.bootstrap.is_none()
            && !self.term.mode().contains(TerminalMode::ALT_SCREEN)
        {
            self.resize_conpty_screen(size);
            return;
        }
        self.term.resize(size);
    }

    /// Once native EOF is observed, no future redraw can restore clipped screen cells. Ended tabs
    /// therefore return to Alacritty's full logical reflow before any subsequent geometry change.
    pub fn end_process(&mut self) {
        self.conpty_screen = false;
        self.bootstrap = None;
        self.bootstrap_line_start = false;
        // EOF is also a completion boundary for a child that forgot its synchronized-update end.
        self.parser.stop_sync(&mut self.term);
        let _ = self.process(&[]);
    }

    /// History settings update Alacritty's limit without resetting the active screen or mode.
    pub fn set_history(&mut self, history: usize) {
        self.history_limit = history;
        self.term.set_options(ScreenConfig {
            scrolling_history: history,
            default_cursor_style: self.default_cursor,
            vi_mode_cursor_style: Some(CursorStyle {
                shape: CursorShape::Block,
                blinking: false,
            }),
            ..ScreenConfig::default()
        });
    }

    /// Apply an explicit user cursor preference; application DECSCUSR overrides remain Alacritty-owned.
    pub fn set_cursor_style(&mut self, shape: CursorShape, blinking: bool) {
        let style = CursorStyle { shape, blinking };
        if self.default_cursor == style {
            return;
        }
        self.default_cursor = style;
        self.set_history(self.history_limit);
    }

    /// The view clock blinks only the effective cursor requested by the application or user.
    pub fn cursor_blinks(&self) -> bool {
        self.term.cursor_style().blinking
    }

    /// Update cursor focus without sending any bytes or modifying the viewport.
    pub fn set_focused(&mut self, focused: bool) {
        self.term.is_focused = focused;
        if focused {
            self.bell = false;
        }
    }
    /// Return the current negotiated ANSI modes, including alternate screen, mouse and Vi state.
    pub fn mode(&self) -> TerminalMode {
        *self.term.mode()
    }
    /// Return the number of retained rows above the current live screen.
    pub fn history(&self) -> usize {
        self.term.grid().history_size()
    }
    /// Return the displayed distance in rows from the live bottom.
    pub fn offset(&self) -> usize {
        self.term.grid().display_offset()
    }
    /// Scroll by `lines` rows; positive moves into history and negative moves toward live output.
    pub fn scroll(&mut self, lines: i32) {
        self.term.scroll_display(Scroll::Delta(lines));
        self.follow_screen = self.offset() == 0;
    }
    /// Set the displayed distance from the bottom in rows, clamped by the maintained core.
    pub fn set_offset(&mut self, offset: usize) {
        self.follow_screen = offset == 0;
        self.term
            .scroll_display(Scroll::Delta(offset as i32 - self.offset() as i32));
    }
    /// Explicitly clear normal history and selection while preserving the active child's protocol modes.
    pub fn clear(&mut self) {
        self.search.current = None;
        // Clear primary history as well when a TUI is active, retaining its input/mouse modes.
        let alternate = self.term.mode().contains(TerminalMode::ALT_SCREEN);
        if alternate {
            self.term.swap_alt();
        }
        self.term.clear_screen(ClearMode::All);
        self.term.clear_screen(ClearMode::Saved);
        self.term.grid_mut().cursor.point = Point::new(Line(0), Column(0));
        self.term.grid_mut().cursor.input_needs_wrap = false;
        if alternate {
            self.term.swap_alt();
        }
        self.term.selection = None;
        self.set_offset(0);
    }
    /// Save public logical cells, rather than depending on Alacritty's private storage layout.
    pub fn snapshot(&self) -> SavedGrid {
        let grid = self.term.grid();
        let lines = (-(grid.history_size() as i32)..grid.screen_lines() as i32)
            .map(|line| {
                let mut cells = (0..grid.columns())
                    .map(|column| grid[Point::new(Line(line), Column(column))].clone())
                    .collect::<Vec<_>>();
                while cells.last() == Some(&Cell::default()) {
                    cells.pop();
                }
                cells
            })
            .collect();
        SavedGrid {
            columns: grid.columns(),
            rows: grid.screen_lines(),
            lines,
            cursor: (grid.cursor.point.line.0, grid.cursor.point.column.0),
            input_needs_wrap: grid.cursor.input_needs_wrap,
            offset: grid.display_offset(),
        }
    }
    /// Validate geometry before allocating or indexing; a damaged snapshot cannot panic the UI.
    pub fn restore(&mut self, saved: SavedGrid) -> anyhow::Result<()> {
        anyhow::ensure!(
            (2..=1000).contains(&saved.columns) && (1..=500).contains(&saved.rows),
            t!("terminal.invalid_snapshot")
        );
        anyhow::ensure!(
            saved.lines.len() >= saved.rows
                && saved.lines.len() <= saved.rows + self.history_limit
                && saved.lines.iter().all(|row| row.len() <= saved.columns),
            t!("terminal.invalid_snapshot")
        );
        anyhow::ensure!(
            (0..saved.rows as i32).contains(&saved.cursor.0) && saved.cursor.1 < saved.columns,
            t!("terminal.invalid_snapshot")
        );
        let history = saved.lines.len() - saved.rows;
        let grid = self.term.grid_mut();
        // A snapshot replaces cells rather than overlaying trimmed rows on a previous screen.
        // Construct history with the default rendition: scroll_up also initializes blank suffixes
        // in history. Restoring the live template earlier would tint old rows with the new SGR.
        // Keep live rendition/saved cursor for subsequent output, after all saved cells are copied.
        let template = grid.cursor.template.clone();
        let saved_cursor = grid.saved_cursor.clone();
        grid.reset();
        grid.scroll_up::<Color>(&(Line(0)..Line(saved.rows as i32)), history);
        for (row, cells) in saved.lines.into_iter().enumerate() {
            for (column, cell) in cells.into_iter().enumerate() {
                grid[Point::new(Line(row as i32 - history as i32), Column(column))] = cell;
            }
        }
        grid.cursor.template = template;
        grid.saved_cursor = saved_cursor;
        grid.cursor.point = Point::new(Line(saved.cursor.0), Column(saved.cursor.1));
        grid.cursor.input_needs_wrap = saved.input_needs_wrap;
        grid.scroll_display(Scroll::Delta(saved.offset.min(history) as i32));
        Ok(())
    }
    /// Return Alacritty's selected Unicode text, excluding empty/invalid selections and wide spacers.
    pub fn selected_text(&self) -> Option<String> {
        self.term
            .selection_to_string()
            .filter(|text| !text.is_empty())
    }
    /// Drop the current selection without modifying content, scroll position or child input.
    pub fn clear_selection(&mut self) {
        self.term.selection = None;
    }

    /// Ignore padding after the last printed cell, while allowing spaces inside actual content.
    /// `row`/`column` are visible cells; `start` begins a selection and `clicks` selects text/word/line.
    pub fn select(&mut self, row: usize, column: usize, start: bool, clicks: u8) {
        let line = Line(row as i32 - self.offset() as i32);
        let last = (0..self.term.columns()).rev().find(|&column| {
            let cell = &self.term.grid()[Point::new(line, Column(column))];
            cell.c != ' '
                || cell
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                || cell.zerowidth().is_some_and(|text| !text.is_empty())
        });
        if start && last.is_none_or(|last| column > last) {
            self.clear_selection();
            return;
        }
        let point = Point::new(
            line,
            Column(
                column
                    .min(last.map_or(0, |last| last + 1))
                    .min(self.term.columns() - 1),
            ),
        );
        if start {
            let kind = match clicks {
                2 => SelectionType::Semantic,
                3.. => SelectionType::Lines,
                _ => SelectionType::Simple,
            };
            self.term.selection = Some(Selection::new(kind, point, Side::Left));
        } else if let Some(selection) = &mut self.term.selection {
            selection.update(point, Side::Right);
        }
    }

    fn color(&self, value: Color, colors: &Colors, bold: bool) -> u32 {
        let index = match value {
            Color::Spec(color) => {
                return u32::from(color.r) << 16 | u32::from(color.g) << 8 | u32::from(color.b);
            }
            Color::Indexed(index) => index as usize,
            Color::Named(color) => color as usize,
        };
        let index = if bold && index < 8 {
            index + 8
        } else if bold && index == 256 {
            267
        } else {
            index
        };
        self.color_index(index, colors)
    }

    /// The same effective color answers OSC queries and paints cells, including runtime overrides.
    pub fn color_index(&self, index: usize, colors: &Colors) -> u32 {
        if index < 269
            && let Some(color) = self.term.colors()[index]
        {
            return u32::from(color.r) << 16 | u32::from(color.g) << 8 | u32::from(color.b);
        }
        if let Some(color) = colors.overrides.get(&index) {
            return *color;
        }
        match index {
            0..=15 => colors.ansi[index],
            16..=231 => {
                let n = index - 16;
                let level = |value: usize| if value == 0 { 0 } else { 55 + value * 40 };
                ((level(n / 36) << 16) | (level(n / 6 % 6) << 8) | level(n % 6)) as u32
            }
            232..=255 => {
                let gray = (8 + (index - 232) * 10) as u32;
                gray << 16 | gray << 8 | gray
            }
            value if value == NamedColor::Background as usize => colors.background,
            258 => colors.cursor,
            259..=266 => colors.ansi[index - 259],
            _ => colors.foreground,
        }
    }
}
