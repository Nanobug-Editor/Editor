// SPDX-License-Identifier: Apache-2.0
//! Recorded VT streams validate the local screen against independent expected grids.
//!
//! Adapted from Alacritty 0.26.0; see this crate's NOTICE and LICENSE-ALACRITTY.

use serde::Deserialize;
use serde_json as json;

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use crate::coordinates::{Column, Line};
use crate::events::{Event, EventListener};
use crate::grid::{Dimensions, Grid};
use crate::screen::cell::{Cell, Hyperlink};
use crate::screen::test_support::ScreenSize;
use crate::screen::{Screen, ScreenConfig};
use vte::ansi;

macro_rules! ref_tests {
    ($($name:ident)*) => {
        $(
            #[test]
            fn $name() {
                let test_dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/screen"));
                let test_path = test_dir.join(stringify!($name));
                ref_test(&test_path);
            }
        )*
    };
}

ref_tests! {
    alt_reset
    clear_underline
    colored_reset
    colored_underline
    csi_rep
    decaln_reset
    deccolm_reset
    delete_chars_reset
    delete_lines
    erase_chars_reset
    fish_cc
    grid_reset
    history
    hyperlinks
    indexed_256_colors
    insert_blank_reset
    issue_855
    ll
    newline_with_cursor_beyond_scroll_region
    region_scroll_down
    row_reset
    saved_cursor
    saved_cursor_alt
    scroll_up_reset
    selective_erasure
    sgr
    tab_rendering
    tmux_git_log
    tmux_htop
    underline
    vim_24bitcolors_bce
    vim_large_window_scroll
    vim_simple_edit
    vttest_cursor_movement_1
    vttest_insert
    vttest_origin_mode_1
    vttest_origin_mode_2
    vttest_scroll
    vttest_tab_clear_set
    wrapline_alt_toggle
    zerowidth
    zsh_tab_completion
    erase_in_line
    scroll_in_region_up_preserves_history
    origin_goto
}

fn read_u8<P>(path: P) -> Vec<u8>
where
    P: AsRef<Path>,
{
    let mut res = Vec::new();
    File::open(path.as_ref())
        .unwrap()
        .read_to_end(&mut res)
        .unwrap();

    res
}

#[derive(Deserialize, Default)]
struct RefConfig {
    history_size: u32,
}

#[derive(Copy, Clone)]
struct Mock;

impl EventListener for Mock {
    fn send_event(&self, _event: Event) {}
}

fn ref_test(dir: &Path) {
    let recording = read_u8(dir.join("alacritty.recording"));
    let serialized_size = fs::read_to_string(dir.join("size.json")).unwrap();
    let serialized_grid = fs::read_to_string(dir.join("grid.json")).unwrap();
    let serialized_cfg = fs::read_to_string(dir.join("config.json")).unwrap();

    let size: ScreenSize = json::from_str(&serialized_size).unwrap();
    let grid: Grid<Cell> = json::from_str(&serialized_grid).unwrap();
    let ref_config: RefConfig = json::from_str(&serialized_cfg).unwrap();

    let options = ScreenConfig {
        scrolling_history: ref_config.history_size as usize,
        ..Default::default()
    };

    let mut terminal = Screen::new(options, &size, Mock);
    let mut parser: ansi::Processor = ansi::Processor::new();

    parser.advance(&mut terminal, &recording);

    // Truncate invisible lines from the grid.
    let mut term_grid = terminal.grid().clone();
    term_grid.initialize_all();
    term_grid.truncate();
    normalize_generated_links(&grid, &mut term_grid);

    if grid != term_grid {
        for i in grid.topmost_line().0..=grid.bottommost_line().0 {
            for j in 0..grid.columns() {
                let cell = &term_grid[Line(i)][Column(j)];
                let original_cell = &grid[Line(i)][Column(j)];
                if original_cell != cell {
                    println!("[{i}][{j}] {original_cell:?} => {cell:?}",);
                }
            }
        }

        panic!("Ref test failed; grid doesn't match");
    }

    assert_eq!(grid, term_grid);
}

/// Anonymous OSC8 ids are process-wide opaque counters. Preserve URI/content and the bijection
/// between link groups while removing the recording's assumption that this process starts at zero.
fn normalize_generated_links(expected: &Grid<Cell>, observed: &mut Grid<Cell>) {
    let reference = json::to_value(expected).unwrap();
    let mut actual = json::to_value(&*observed).unwrap();
    let reference_rows = reference["raw"]["inner"].as_array().unwrap();
    let actual_rows = actual["raw"]["inner"].as_array_mut().unwrap();
    if reference_rows.len() != actual_rows.len() {
        return;
    }
    let generated = |id: &str| {
        id.strip_suffix("_alacritty").is_some_and(|number| {
            !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
        })
    };
    let mut forward = BTreeMap::new();
    let mut reverse = BTreeMap::new();
    // Old recordings contain historical ring-index metadata. Compare their serialized cells
    // without treating that metadata as a freshly initialized, indexable runtime screen.
    for (reference_row, actual_row) in reference_rows.iter().zip(actual_rows) {
        let reference_cells = reference_row["inner"].as_array().unwrap();
        let actual_cells = actual_row["inner"].as_array_mut().unwrap();
        if reference_cells.len() != actual_cells.len() {
            return;
        }
        for (reference_cell, actual_cell) in reference_cells.iter().zip(actual_cells) {
            const ID: &str = "/extra/hyperlink/inner/id";
            let (Some(reference_id), Some(actual_id)) = (
                reference_cell.pointer(ID).and_then(json::Value::as_str),
                actual_cell
                    .pointer(ID)
                    .and_then(json::Value::as_str)
                    .map(str::to_owned),
            ) else {
                continue;
            };
            if !generated(reference_id) || !generated(&actual_id) {
                continue;
            }
            if let Some(previous) = forward.insert(actual_id.clone(), reference_id.to_owned()) {
                assert_eq!(
                    previous, reference_id,
                    "distinct recorded links cannot merge"
                );
            }
            if let Some(previous) = reverse.insert(reference_id.to_owned(), actual_id.clone()) {
                assert_eq!(previous, actual_id, "one recorded link cannot split");
            }
            *actual_cell.pointer_mut(ID).unwrap() = reference_id.into();
        }
    }
    *observed = json::from_value(actual).unwrap();
}

/// The integrated suite shares one process with Engine tests, so other OSC8 streams can precede it.
#[test]
fn anonymous_hyperlink_recording_survives_prior_output() {
    let _prior = Hyperlink::new::<String>(None, "https://example.com/prior".into());
    let recording = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/screen/hyperlinks"
    ));
    ref_test(recording);
    ref_test(recording);
}
