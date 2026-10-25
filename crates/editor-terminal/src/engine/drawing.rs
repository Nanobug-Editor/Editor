//! Project the owned Alacritty grid into native drawing operations.

use super::*;

impl Engine {
    /// Produce native drawing data directly; no Document tree, serialization or guest event loop.
    pub fn drawing(
        &mut self,
        width: f32,
        height: f32,
        cell_width: f32,
        cell_height: f32,
        font: &str,
        font_size: f32,
        colors: &Colors,
    ) -> GridDrawing {
        let matches = self.visible_matches();
        let mut canvas = GridDrawing {
            font: GridFont {
                family: font.into(),
                size_px: font_size,
                line_height: self.line_height,
            },
            ..Default::default()
        };
        let rect = |x, y, w, h| Rect { x, y, w, h };
        canvas.paint.push(Paint::Fill {
            rect: rect(0., 0., width, height),
            color: colors.background,
            extend_to_bottom: true,
        });
        let content = self.term.view_state();
        for item in self.term.grid().display_iter() {
            let cell = item.cell;
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let row = item.point.line.0 + self.offset() as i32;
            if row < 0 || row as usize >= self.term.screen_lines() {
                continue;
            }
            let x = 8. + item.point.column.0 as f32 * cell_width;
            let y = 8. + row as f32 * cell_height;
            let mut fg = self.color(cell.fg, colors, cell.flags.contains(Flags::BOLD));
            if cell.flags.contains(Flags::DIM) {
                let index = match cell.fg {
                    Color::Named(NamedColor::Foreground) => Some(268),
                    Color::Named(color) if (color as usize) < 8 => Some(color as usize + 259),
                    Color::Indexed(index @ 0..=7) => Some(index as usize + 259),
                    _ => None,
                };
                if let Some(index) = index {
                    fg = self.color_index(index, colors);
                }
            }
            let mut bg = self.color(cell.bg, colors, false);
            if cell.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            if matches.iter().any(|found| found.contains(&item.point)) {
                bg = colors.selection;
            }
            if self.search_visible
                && self
                    .search
                    .current
                    .as_ref()
                    .is_some_and(|found| found.contains(&item.point))
                && !content
                    .selection
                    .as_ref()
                    .is_some_and(|selection| selection.contains(item.point))
            {
                // Active matches remain distinguishable from the other matches in either theme.
                bg = colors.cursor;
                fg = colors.background;
            }
            if content
                .selection
                .as_ref()
                .is_some_and(|selection| selection.contains(item.point))
            {
                bg = colors.selection;
            }
            let span = if cell.flags.contains(Flags::WIDE_CHAR) {
                2.
            } else {
                1.
            };
            if bg != colors.background {
                canvas.paint.push(Paint::Fill {
                    rect: rect(x, y, span * cell_width, cell_height),
                    color: bg,
                    extend_to_bottom: false,
                });
            }
            if cell.c != ' ' && !cell.flags.contains(Flags::HIDDEN) {
                let mut text = cell.c.to_string();
                if let Some(extra) = cell.zerowidth() {
                    text.extend(extra);
                }
                canvas.paint.push(Paint::Text {
                    x,
                    y,
                    text,
                    color: fg,
                    size: font_size,
                    bold: cell.flags.contains(Flags::BOLD),
                    italic: cell.flags.contains(Flags::ITALIC),
                    font: Some(font.into()),
                });
            }
            if !cell.flags.contains(Flags::HIDDEN) && cell.flags.intersects(Flags::ALL_UNDERLINES) {
                let color = cell
                    .underline_color()
                    .map_or(fg, |color| self.color(color, colors, false));
                decoration(
                    &mut canvas.paint,
                    x,
                    y,
                    span * cell_width,
                    cell_height,
                    color,
                    cell.flags,
                );
            }
            if cell.flags.contains(Flags::STRIKEOUT) && !cell.flags.contains(Flags::HIDDEN) {
                canvas.paint.push(Paint::Fill {
                    rect: rect(x, y + cell_height / 2., span * cell_width, 1.),
                    color: fg,
                    extend_to_bottom: false,
                });
            }
        }
        // The restored prefix can be visible above the new native screen. Map its caret to the
        // viewport without altering the coordinates sent to the PTY.
        if content.cursor.shape != CursorShape::Hidden
            && let Some(cursor) =
                crate::screen::point_to_viewport(self.offset(), content.cursor.point)
            && cursor.line < self.term.screen_lines()
        {
            let span = if self.term.grid()[content.cursor.point]
                .flags
                .contains(Flags::WIDE_CHAR)
            {
                2.
            } else {
                1.
            };
            let caret = rect(
                8. + cursor.column.0 as f32 * cell_width,
                8. + cursor.line as f32 * cell_height,
                cell_width * span,
                cell_height,
            );
            if self.cursor_visible || !self.term.cursor_style().blinking || !self.term.is_focused {
                let shape = if !self.term.is_focused && content.cursor.shape == CursorShape::Block {
                    CursorShape::HollowBlock
                } else {
                    content.cursor.shape
                };
                let cursor_rect = match shape {
                    CursorShape::Beam => Rect { w: 1.5, ..caret },
                    CursorShape::Underline => Rect {
                        y: caret.y + caret.h - 2.,
                        h: 2.,
                        ..caret
                    },
                    _ => caret,
                };
                if shape == CursorShape::HollowBlock {
                    for rect in [
                        Rect { h: 1., ..caret },
                        Rect {
                            y: caret.y + caret.h - 1.,
                            h: 1.,
                            ..caret
                        },
                        Rect { w: 1., ..caret },
                        Rect {
                            x: caret.x + caret.w - 1.,
                            w: 1.,
                            ..caret
                        },
                    ] {
                        canvas.paint.push(Paint::Fill {
                            rect,
                            color: colors.cursor,
                            extend_to_bottom: false,
                        });
                    }
                } else {
                    canvas.paint.push(Paint::Fill {
                        rect: cursor_rect,
                        color: colors.cursor,
                        extend_to_bottom: false,
                    });
                }
                if shape == CursorShape::Block {
                    let cell = &self.term.grid()[content.cursor.point];
                    if cell.c != ' ' && !cell.flags.contains(Flags::HIDDEN) {
                        let mut text = cell.c.to_string();
                        if let Some(extra) = cell.zerowidth() {
                            text.extend(extra);
                        }
                        canvas.paint.push(Paint::Text {
                            x: caret.x,
                            y: caret.y,
                            text,
                            color: colors.background,
                            size: font_size,
                            bold: cell.flags.contains(Flags::BOLD),
                            italic: cell.flags.contains(Flags::ITALIC),
                            font: Some(font.into()),
                        });
                    }
                }
            }
            canvas.caret = Some(caret);
        }
        if self.history() > 0 {
            canvas.scroll = Some(ScrollRange {
                content: height + self.history() as f32 * cell_height,
                offset: self.history().saturating_sub(self.offset()) as f32 * cell_height,
            });
        }
        canvas
    }
}

/// Paint SGR decorations in physical grid pixels, preserving explicit underline color.
/// `width` covers the complete glyph span; the pattern phase uses absolute x across adjacent cells.
fn decoration(
    paint: &mut Vec<Paint>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: u32,
    flags: Flags,
) {
    let mut line = |x, y, w| {
        paint.push(Paint::Fill {
            rect: Rect { x, y, w, h: 1. },
            color,
            extend_to_bottom: false,
        })
    };
    let baseline = y + height - 2.;
    if flags.contains(Flags::UNDERCURL) {
        for pixel in 0..width.ceil() as usize {
            let phase = (x as usize + pixel) % 6;
            let offset = [0., 1., 2., 2., 1., 0.][phase];
            line(
                x + pixel as f32,
                baseline - offset,
                1_f32.min(width - pixel as f32),
            );
        }
    } else if flags.intersects(Flags::DOTTED_UNDERLINE | Flags::DASHED_UNDERLINE) {
        let dash = if flags.contains(Flags::DOTTED_UNDERLINE) {
            1
        } else {
            3
        };
        for pixel in 0..width.ceil() as usize {
            if (x as usize + pixel) % (dash * 2) < dash {
                line(x + pixel as f32, baseline, 1_f32.min(width - pixel as f32));
            }
        }
    } else {
        line(x, baseline, width);
        if flags.contains(Flags::DOUBLE_UNDERLINE) {
            line(x, baseline - 2., width);
        }
    }
}
