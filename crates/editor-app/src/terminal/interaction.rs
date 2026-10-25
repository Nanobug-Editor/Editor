//! Native keyboard, pointer and menu policy; terminal bytes never pass through a WASM dispatcher.

use super::*;
use editor_ui::text_grid::GridPointerPhase as PointerPhase;
use engine::TerminalMode;
use protocol::ui::MenuItem;

impl TerminalPanel {
    pub(super) fn grid_event(&mut self, event: GridEvent, cx: &mut Context<Self>) {
        match event {
            GridEvent::Files { paths } => self.paste_paths(&paths, cx),
            GridEvent::Resize {
                width,
                height,
                grid,
            } => {
                self.width = width;
                self.height = height;
                if let Some(grid) = grid {
                    self.cell_width = grid.cell_width;
                    self.cell_height = grid.cell_height;
                }
                let size = self.grid_size();
                if let Some(index) = self.active_index() {
                    let tab = &mut self.sessions[index];
                    tab.engine.resize(size);
                    let resize = (tab.launched && !tab.exited).then_some(tab.id);
                    if let Some(id) = resize {
                        self.resize_process(id, size, cx);
                    }
                }
                self.launch_active(false);
            }
            GridEvent::Text { text } => {
                if !self.active_index().is_some_and(|index| {
                    self.sessions[index]
                        .engine
                        .mode()
                        .contains(TerminalMode::VI)
                }) {
                    self.write(text.into_bytes(), cx);
                }
            }
            GridEvent::Key {
                key,
                ctrl,
                alt,
                shift,
            } => {
                if ctrl && shift && !alt && key == "t" {
                    self.new_shell(self.settings.default_profile, cx);
                } else if ctrl && shift && !alt && key == "w" {
                    if let Some(id) = self.active {
                        self.close(id, cx);
                    }
                } else if ctrl && !alt && key == "c" {
                    self.copy(cx);
                } else if ctrl && !alt && key == "v" {
                    self.paste(cx);
                } else if let Some(index) = self.active_index() {
                    let mode = self.sessions[index].engine.mode();
                    let page = self.grid_size().rows as i32;
                    if shift && key == "home" {
                        let history = self.sessions[index].engine.history();
                        self.sessions[index].engine.set_offset(history);
                    } else if shift && key == "end" {
                        self.sessions[index].engine.set_offset(0);
                    } else if shift && key == "pageup" {
                        self.sessions[index].engine.scroll(page);
                    } else if shift && key == "pagedown" {
                        self.sessions[index].engine.scroll(-page);
                    } else if shift && key == "enter" && self.settings.shift_enter_newline {
                        self.write(vec![10], cx);
                    } else if let Some(bytes) = input::key(
                        &key,
                        ctrl,
                        alt,
                        shift,
                        mode.contains(TerminalMode::APP_CURSOR),
                    ) {
                        self.write(bytes, cx);
                    }
                }
            }
            GridEvent::Scroll { offset } => {
                if let Some(index) = self.active_index() {
                    let history = self.sessions[index].engine.history();
                    self.sessions[index].engine.set_offset(
                        history.saturating_sub((offset / self.cell_height).round() as usize),
                    );
                }
            }
            GridEvent::Wheel {
                delta_y,
                shift,
                x,
                y,
                modifiers,
                ..
            } => {
                self.pointer_modifiers.set(modifiers);
                if let Some(index) = self.active_index() {
                    if delta_y == 0. {
                        return;
                    }
                    self.wheel_remainder +=
                        delta_y / self.cell_height * self.settings.scroll_multiplier;
                    let lines = self.wheel_remainder.trunc().clamp(-100., 100.) as i32;
                    self.wheel_remainder -= lines as f32;
                    let mode = self.sessions[index].engine.mode();
                    if !shift && mode.intersects(TerminalMode::MOUSE_MODE) {
                        for _ in 0..lines.unsigned_abs() {
                            self.report_mouse(if lines > 0 { 64 } else { 65 }, x, y, false, cx);
                        }
                    } else if !shift
                        && mode.contains(TerminalMode::ALT_SCREEN | TerminalMode::ALTERNATE_SCROLL)
                        && !mode.contains(TerminalMode::VI)
                    {
                        if let Some(bytes) = input::key(
                            if lines > 0 { "up" } else { "down" },
                            false,
                            false,
                            false,
                            mode.contains(TerminalMode::APP_CURSOR),
                        ) {
                            self.write(bytes.repeat(lines.unsigned_abs() as usize), cx);
                        }
                    } else {
                        self.sessions[index].engine.scroll(lines);
                    }
                }
            }
            GridEvent::Pointer {
                phase,
                x,
                y,
                button,
                clicks,
                shift,
                modifiers,
            } => {
                self.pointer_modifiers.set(modifiers);
                if let Some(index) = self.active_index() {
                    if phase == PointerPhase::Down && x >= 8. && y >= 8. {
                        self.link = self.sessions[index].engine.link_at(
                            ((y - 8.) / self.cell_height) as usize,
                            ((x - 8.) / self.cell_width) as usize,
                        );
                    }
                    let mouse = self.sessions[index]
                        .engine
                        .mode()
                        .intersects(TerminalMode::MOUSE_MODE);
                    if phase == PointerPhase::Move && button == 3 && !mouse {
                        return;
                    }
                    if mouse && !shift {
                        let moving = phase == PointerPhase::Move;
                        let mode = self.sessions[index].engine.mode();
                        if !moving
                            || mode.contains(TerminalMode::MOUSE_MOTION)
                            || (button != 3 && mode.contains(TerminalMode::MOUSE_DRAG))
                        {
                            self.report_mouse(
                                button as u8 + if moving { 32 } else { 0 },
                                x,
                                y,
                                phase == PointerPhase::Up,
                                cx,
                            );
                        }
                    } else if button == 2 && phase == PointerPhase::Down {
                        self.requested_menu = Some(true);
                    } else if button == 0 {
                        if phase == PointerPhase::Move {
                            if y < 8. {
                                self.sessions[index].engine.scroll(1);
                            } else if y >= self.height - 8. {
                                self.sessions[index].engine.scroll(-1);
                            }
                        }
                        let row = (((y - 8.) / self.cell_height).floor().max(0.) as usize)
                            .min(self.grid_size().rows - 1);
                        let column = (((x - 8.) / self.cell_width).floor().max(0.) as usize)
                            .min(self.grid_size().columns - 1);
                        if row < self.grid_size().rows
                            && column < self.grid_size().columns
                            && phase != PointerPhase::Up
                        {
                            self.sessions[index].engine.select(
                                row,
                                column,
                                phase == PointerPhase::Down,
                                clicks,
                            );
                            if phase == PointerPhase::Down && self.pointer_modifiers.get().alt {
                                self.sessions[index].engine.block_selection();
                            }
                        }
                        if phase == PointerPhase::Up && self.settings.copy_on_select {
                            self.copy(cx);
                        }
                    }
                }
            }
            GridEvent::Focus { focused } => {
                self.cursor_blink = Instant::now();
                if let Some(index) = self.active_index() {
                    self.sessions[index].engine.cursor_visible = true;
                    self.sessions[index].engine.set_focused(focused);
                }
                if self.active_index().is_some_and(|index| {
                    self.sessions[index]
                        .engine
                        .mode()
                        .contains(TerminalMode::FOCUS_IN_OUT)
                }) {
                    // Focus reporting neither starts a restored Shell nor changes its scroll position.
                    if let Some(index) = self.active_index() {
                        let session = &self.sessions[index];
                        if session.launched && !session.exited {
                            self.send_input(
                                session.id,
                                if focused {
                                    b"\x1b[I".to_vec()
                                } else {
                                    b"\x1b[O".to_vec()
                                },
                                cx,
                            );
                        }
                    }
                }
            }
        }
        self.dirty = true;
        cx.notify();
    }

    /// SGR and legacy mouse bytes are addressed only to the selected terminal's owned process.
    fn report_mouse(&mut self, button: u8, x: f32, y: f32, release: bool, cx: &mut App) {
        let Some(index) = self.active_index() else {
            return;
        };
        let column = ((x - 8.) / self.cell_width).floor().max(0.) as usize;
        let row = ((y - 8.) / self.cell_height).floor().max(0.) as usize;
        let session = &self.sessions[index];
        let Some((row, column)) = session.engine.mouse_position(row, column) else {
            // Retained history has no corresponding cell in this child's screen.
            return;
        };
        if !session.launched || session.exited {
            return;
        }
        let (row, column) = (row + 1, column + 1);
        let id = session.id;
        let mode = self.sessions[index].engine.mode();
        let modifiers = self.pointer_modifiers.get();
        let flags = 8 * u8::from(modifiers.alt) + 16 * u8::from(modifiers.control);
        let motion = (id, button | flags, column, row);
        if (32..64).contains(&button) && self.last_mouse == Some(motion) {
            return;
        }
        self.last_mouse = Some(motion);
        let Some(bytes) = input::mouse(
            button,
            column,
            row,
            release,
            mode.contains(TerminalMode::SGR_MOUSE),
            mode.contains(TerminalMode::UTF8_MOUSE),
            flags,
        ) else {
            return;
        };
        // Mouse input must not scroll the viewport like explicit typing; that would move the
        // live row underneath the pointer after encoding its native coordinates.
        self.send_input(id, bytes, cx);
    }

    pub(super) fn copy(&mut self, cx: &mut App) {
        if let Some(text) = self
            .active_index()
            .and_then(|index| self.sessions[index].engine.selected_text())
        {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            if !self.settings.keep_selection_on_copy
                && let Some(index) = self.active_index()
            {
                self.sessions[index].engine.clear_selection();
            }
        }
    }
    fn paste(&mut self, cx: &mut App) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        let paths = item
            .entries()
            .iter()
            .filter_map(|entry| {
                if let gpui_kit::ClipboardEntry::ExternalPaths(paths) = entry {
                    Some(paths.paths().iter().cloned())
                } else {
                    None
                }
            })
            .flatten()
            .collect::<Vec<_>>();
        if !paths.is_empty() {
            self.paste_paths(&paths, cx);
            return;
        }
        if self.settings.forward_image_paste
            && item
                .entries()
                .iter()
                .any(|entry| matches!(entry, gpui_kit::ClipboardEntry::Image(_)))
        {
            self.write(vec![22], cx);
            return;
        }
        if let Some(text) = item.text() {
            if let Some(index) = self.active_index()
                && self.sessions[index]
                    .engine
                    .mode()
                    .contains(TerminalMode::VI)
            {
                self.sessions[index].engine.toggle_vi();
            }
            let bracketed = self.active_index().is_some_and(|index| {
                self.sessions[index]
                    .engine
                    .mode()
                    .contains(TerminalMode::BRACKETED_PASTE)
            });
            self.write(input::paste(&text, bracketed), cx);
        }
    }

    /// User-selected file paths are quoted for the selected Shell and pasted without a newline.
    fn paste_paths(&mut self, paths: &[PathBuf], cx: &mut App) {
        let Some(index) = self.active_index() else {
            return;
        };
        if self.sessions[index]
            .engine
            .mode()
            .contains(TerminalMode::VI)
        {
            self.sessions[index].engine.toggle_vi();
        }
        let Some(text) = shell::quote_paths(&self.sessions[index].profile, paths) else {
            self.error = Some(t!("terminal.unsafe_path_paste").to_string());
            return;
        };
        let bracketed = self.sessions[index]
            .engine
            .mode()
            .contains(TerminalMode::BRACKETED_PASTE);
        self.write(input::paste(&text, bracketed), cx);
    }

    pub(super) fn menu_items(&self, output: bool) -> Vec<MenuItem> {
        let selection = self
            .active_index()
            .and_then(|index| self.sessions[index].engine.selected_text())
            .is_some();
        let mut items = vec![
            MenuItem {
                id: "copy".into(),
                label: t!("editor.copy").to_string(),
                disabled: !selection,
                separator_before: false,
            },
            MenuItem {
                id: "paste".into(),
                label: t!("editor.paste").to_string(),
                disabled: self.active.is_none(),
                separator_before: false,
            },
            MenuItem {
                id: "clear".into(),
                label: t!("terminal.clear").to_string(),
                disabled: self.active.is_none(),
                separator_before: false,
            },
        ];
        if !output {
            for (id, key) in [
                ("split-columns", "terminal.split_columns"),
                ("split-rows", "terminal.split_rows"),
                ("unsplit", "terminal.unsplit"),
            ] {
                items.push(MenuItem {
                    id: id.into(),
                    label: t!(key).to_string(),
                    disabled: self.active.is_none() || (id == "unsplit" && self.panes.is_none()),
                    separator_before: id == "split-columns",
                });
            }
            items.push(MenuItem {
                id: "find".into(),
                label: t!("terminal.find").to_string(),
                disabled: self.active.is_none(),
                separator_before: true,
            });
            items.push(MenuItem {
                id: "interrupt".into(),
                label: t!("terminal.interrupt").to_string(),
                disabled: self.active.is_none(),
                separator_before: true,
            });
            items.push(MenuItem {
                id: "settings".into(),
                label: t!("terminal.settings").to_string(),
                disabled: false,
                separator_before: false,
            });
            for (index, profile) in self.settings.profiles.iter().enumerate() {
                items.push(MenuItem {
                    id: format!("new:{index}"),
                    label: profile.name.clone(),
                    disabled: false,
                    separator_before: index == 0,
                });
            }
        }
        if output && self.link.is_some() {
            items.insert(
                0,
                MenuItem {
                    id: "open-link".into(),
                    label: t!("terminal.open_link").to_string(),
                    disabled: false,
                    separator_before: false,
                },
            );
        }
        items
    }

    pub(super) fn menu_action(
        &mut self,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.popup = None;
        if let Action::Select(id) = action {
            match id.as_str() {
                "split-columns" => self.split_shell(false, cx),
                "split-rows" => self.split_shell(true, cx),
                "unsplit" => self.unsplit(cx),
                "find" => self.open_search(cx),
                "open-link" => {
                    self.open_link(window, cx);
                }
                "copy" => self.copy(cx),
                "paste" => self.paste(cx),
                "clear" => {
                    if let Some(index) = self.active_index() {
                        self.sessions[index].engine.clear();
                        self.dirty = true;
                    }
                }
                "interrupt" => self.write(vec![3], cx),
                "settings" => {
                    let path = self.storage.join("settings.json");
                    // Existing user edits remain intact when reopening settings.
                    let result = std::fs::create_dir_all(&self.storage).and_then(|_| {
                        if path.exists() {
                            Ok(())
                        } else {
                            std::fs::write(
                                &path,
                                serde_json::to_vec_pretty(&self.settings).unwrap_or_default(),
                            )
                        }
                    });
                    if let Err(error) = result {
                        self.report_error(FailureKind::Settings, error);
                    } else {
                        let _ = self
                            .parent
                            .update(cx, |app, cx| app.open_file(path, window, cx));
                    }
                }
                _ => {
                    if let Some(index) = id.strip_prefix("new:").and_then(|id| id.parse().ok()) {
                        self.new_shell(index, cx);
                    }
                }
            }
        }
        cx.notify();
    }
}
