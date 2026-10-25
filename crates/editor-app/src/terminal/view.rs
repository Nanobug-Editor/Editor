//! Direct GPUI composition of the terminal canvas, native side tabs and explorer-style menus.

use super::*;
use editor_ui::{
    menu::{MenuStyle, PopupMenu},
    side_tabs::{SideTabBar, SideTabsStyle},
};
use gpui_kit::{AnyElement, rgb};

impl TerminalPanel {
    /// Resolve the previous terminal palette alongside host colors and explicit user settings.
    pub(super) fn colors(&self, cx: &App) -> engine::Colors {
        let rgb_value = |color: gpui_kit::Hsla| {
            let color: gpui_kit::Rgba = color.into();
            ((color.r * 255.).round() as u32) << 16
                | ((color.g * 255.).round() as u32) << 8
                | (color.b * 255.).round() as u32
        };
        let parse = |value: Option<&str>| {
            value.and_then(|value| u32::from_str_radix(value.trim_start_matches('#'), 16).ok())
        };
        static DEFAULTS: std::sync::LazyLock<serde_json::Value> = std::sync::LazyLock::new(|| {
            serde_json::from_str(include_str!("theme.json")).expect("native terminal palette")
        });
        let defaults = &*DEFAULTS;
        let defaults = &defaults[if cx.theme().is_dark() {
            "dark"
        } else {
            "light"
        }]["ansi"];
        let names = [
            "black",
            "red",
            "green",
            "yellow",
            "blue",
            "magenta",
            "cyan",
            "white",
            "bright_black",
            "bright_red",
            "bright_green",
            "bright_yellow",
            "bright_blue",
            "bright_magenta",
            "bright_cyan",
            "bright_white",
        ];
        let tokens = theme::plugin_colors(cx);
        let token = |key: &str| tokens.get(&format!("terminal.{key}")).copied();
        let mut overrides = BTreeMap::new();
        for index in 16..=255 {
            if let Some(color) = token(&format!("indexed.{index}")) {
                overrides.insert(index, color);
            }
        }
        for (index, name) in [(267, "bright_foreground"), (268, "dim_foreground")] {
            if let Some(color) = token(name) {
                overrides.insert(index, color);
            }
        }
        for (index, name) in names[..8].iter().enumerate() {
            if let Some(color) = token(&format!("ansi.dim_{name}")) {
                overrides.insert(index + 259, color);
            }
        }
        engine::Colors {
            background: token("background")
                .or_else(|| parse(self.settings.theme.background.as_deref()))
                .unwrap_or_else(|| rgb_value(cx.theme().background)),
            foreground: token("foreground")
                .or_else(|| parse(self.settings.theme.foreground.as_deref()))
                .unwrap_or_else(|| rgb_value(cx.theme().foreground)),
            cursor: token("cursor")
                .or_else(|| parse(self.settings.theme.cursor.as_deref()))
                .unwrap_or_else(|| rgb_value(cx.theme().primary)),
            selection: token("selection")
                .or_else(|| parse(self.settings.theme.selection.as_deref()))
                .unwrap_or_else(|| rgb_value(cx.theme().selection)),
            ansi: std::array::from_fn(|index| {
                token(&format!("ansi.{}", names[index]))
                    .or_else(|| {
                        parse(
                            self.settings
                                .theme
                                .ansi
                                .as_ref()
                                .map(|values| values[index].as_str()),
                        )
                    })
                    .or_else(|| parse(defaults[names[index]].as_str()))
                    .unwrap_or(0x3574f0)
            }),
            overrides,
        }
    }

    /// Native child entities retain focus, IME, drag and rename state across output redraws.
    fn reconcile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.reconcile_panes(window, cx);
        if std::mem::take(&mut self.focus_pending) {
            let active = self.active;
            // The cached dock body renders after its toolbar. Defer focus so a Vi/new-tab click
            // cannot register the toolbar and grid as focused in the same accessibility frame.
            cx.defer_in(window, move |panel, window, cx| {
                if panel.visible && panel.active == active {
                    if let Some(grid) = &panel.grid {
                        grid.read(cx).focus_handle().focus(window, cx);
                    }
                }
            });
        }
        if std::mem::take(&mut self.measure_pending) {
            for canvas in self.grids.values() {
                canvas.update(cx, |canvas, cx| {
                    canvas.invalidate_measurement();
                    cx.notify();
                });
            }
        }
        let model = self.tabs();
        let colors = self.colors(cx);
        let tokens = theme::plugin_colors(cx);
        let color = |key: &str| {
            tokens
                .get(&format!("terminal.ui.{key}"))
                .copied()
                .map(|color| rgb(color).into())
        };
        let mut tab_menu = MenuStyle::current(cx);
        tab_menu.foreground = color("tab.inactive.foreground").unwrap_or(tab_menu.foreground);
        tab_menu.surface = color("tab.inactive.background").unwrap_or(tab_menu.surface);
        tab_menu.border = color("tab.border").unwrap_or(tab_menu.border);
        let styles = theme::plugin_text_styles(cx);
        if let Some(font) = styles.get("terminal.tab") {
            if let Some(family) = &font.family {
                tab_menu.font_family = family.clone().into();
            }
            if let Some(size) = font.size_px {
                tab_menu.font_size = size;
            }
            tab_menu.bold = font.bold.unwrap_or(false);
        }
        let style = SideTabsStyle {
            background: color("tab_bar.background").unwrap_or(if cx.theme().is_dark() {
                cx.theme().tab_bar
            } else {
                rgb(0xf7f8fa).into()
            }),
            border: color("tab_bar.border").unwrap_or(cx.theme().border),
            menu: tab_menu,
            active: color("tab.active.background").unwrap_or(rgb(colors.background).into()),
            active_foreground: color("tab.active.foreground").unwrap_or(cx.theme().primary),
            active_inner_border: color("tab.active.inner_border").unwrap_or(rgb(0x3574f0).into()),
            close_background: color("tab.close.background"),
            close_foreground: color("tab.close.foreground"),
            rename_background: color("tab.rename.background"),
            rename_foreground: color("tab.rename.foreground"),
        };
        if let Some(sidebar) = &self.sidebar {
            // Unchanged models do not invalidate the child while a captured divider is being dragged.
            if sidebar.read(cx).model != model || sidebar.read(cx).style != style {
                sidebar.update(cx, |sidebar, cx| sidebar.update(model, style, window, cx));
            }
        } else {
            let owner = cx.entity().downgrade();
            let redraw = owner.clone();
            let focus = self.focus.clone();
            let preview = self.preview_width.clone();
            self.sidebar = Some(cx.new(|cx| {
                SideTabBar::new(
                    model,
                    style,
                    focus,
                    move |action, _, cx| {
                        let _ = owner.update(cx, |panel, cx| panel.tab_action(action, cx));
                    },
                    preview,
                    move |cx| {
                        let _ = redraw.update(cx, |_, cx| cx.notify());
                    },
                    cx,
                )
            }));
        }
        let styles = theme::plugin_text_styles(cx);
        let content_style = styles.get("terminal.content");
        let family = content_style
            .and_then(|style| style.family.as_deref())
            .unwrap_or(&self.settings.font_family)
            .to_owned();
        let font_size = content_style
            .and_then(|style| style.size_px)
            .unwrap_or(self.settings.font_size);
        self.draw_panes(&family, font_size, &colors, cx);
        if let Some(output_menu) = self.requested_menu.take() {
            let owner = cx.entity().downgrade();
            let items = self.menu_items(output_menu);
            self.popup = Some(cx.new(|cx| {
                PopupMenu::new(
                    items,
                    MenuStyle::current(cx),
                    window.mouse_position(),
                    move |action, window, cx| {
                        let _ = owner.update(cx, |panel, cx| panel.menu_action(action, window, cx));
                    },
                    window,
                    cx,
                )
            }));
        }
    }

    fn body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.reconcile(window, cx);
        let inspection = self
            .active_task()
            .and_then(|key| self.inspections.get(&key))
            .cloned();
        let canvas = v_flex()
            .debug_selector(|| "native-terminal-tab".into())
            .min_w_0()
            .h_full()
            .flex_1()
            .min_h_0()
            .children(self.search_bar(window, cx))
            .child(
                div()
                    .debug_selector(|| "native-terminal-output".into())
                    .flex_1()
                    .min_h_0()
                    .child(self.pane_content(cx)),
            )
            .when_some(inspection, |body, view| {
                body.child(
                    div()
                        .h(px(260.))
                        .max_h(gpui_kit::relative(0.75))
                        .flex_shrink_0()
                        .min_h_0()
                        .child(view),
                )
            });
        let sidebar = div()
            .w(px(self.preview_width.get().unwrap_or(self.tab_width)))
            .flex_shrink_0()
            .h_full()
            .child(self.sidebar.as_ref().unwrap().clone());
        let body = if self.settings.tab_position == editor_terminal::TabPosition::Left {
            h_flex().size_full().child(sidebar).child(canvas)
        } else {
            h_flex().size_full().child(canvas).child(sidebar)
        };
        v_flex()
            .capture_key_down(
                cx.listener(|panel, event, window, cx| panel.navigation_key(event, window, cx)),
            )
            .capture_any_mouse_down(
                cx.listener(|panel, event, window, cx| panel.navigation_pointer(event, window, cx)),
            )
            .relative()
            .size_full()
            .min_h_0()
            .child(body.flex_1().min_h_0())
            .when_some(self.pending_close, |body, id| {
                let name = self
                    .sessions
                    .iter()
                    .find(|session| session.id == id)
                    .map(|session| session.name.clone())
                    .unwrap_or_default();
                body.child(
                    h_flex()
                        .id("terminal-close-confirmation")
                        .debug_selector(|| "terminal-close-confirmation".into())
                        .flex_shrink_0()
                        .p_2()
                        .gap_2()
                        .items_center()
                        .bg(cx.theme().background)
                        .child(t!("terminal.close_task", name = name).to_string())
                        .child(
                            Button::new("terminal-close-cancel")
                                .debug_selector(|| "terminal-close-cancel".into())
                                .label(t!("terminal.cancel"))
                                .on_click(cx.listener(|panel, _, _, cx| {
                                    panel.pending_close = None;
                                    panel.focus_pending = true;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("terminal-close-stop")
                                .debug_selector(|| "terminal-close-stop".into())
                                .label(t!("terminal.stop_close"))
                                .on_click(cx.listener(|panel, _, _, cx| panel.confirm_close(cx))),
                        ),
                )
            })
            .when_some(self.error.clone(), |body, error| {
                body.child(div().px_2().text_color(cx.theme().danger).child(error))
            })
            .when_some(self.popup.clone(), |body, menu| body.child(menu))
            .into_any_element()
    }
}

impl Render for TerminalPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.body(window, cx)
    }
}

impl DockPanel for TerminalPanel {
    /// A title-only drag card leaves the PTY view mounted in its existing allocation.
    fn drag_title(&self, _: &App) -> Option<gpui_kit::SharedString> {
        Some(t!("terminal.title").to_string().into())
    }

    fn title(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(icon(cx))
                    .child(t!("terminal.title").to_string()),
            )
            .child(
                h_flex()
                    .gap_1()
                    .when_some(self.task_toolbar(cx), |row, controls| row.child(controls))
                    .child(
                        Button::new("native-terminal-find")
                            .debug_selector(|| "native-terminal-find".into())
                            .icon(Icon::new(IconName::Search).small())
                            .small()
                            .compact()
                            .ghost()
                            .tooltip(t!("terminal.find").to_string())
                            .on_click(cx.listener(|panel, _, _, cx| panel.open_search(cx))),
                    )
                    .child(
                        Button::new("native-terminal-vi")
                            .debug_selector(|| "native-terminal-vi".into())
                            .label("Vi")
                            .small()
                            .compact()
                            .ghost()
                            .selected(self.active_index().is_some_and(|index| {
                                self.sessions[index]
                                    .engine
                                    .mode()
                                    .contains(engine::TerminalMode::VI)
                            }))
                            .tooltip(t!("terminal.vi").to_string())
                            .on_click(cx.listener(|panel, _, _, cx| {
                                if let Some(index) = panel.active_index() {
                                    panel.sessions[index].engine.toggle_vi();
                                }
                                panel.focus_pending = true;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("native-terminal-new")
                            .debug_selector(|| "native-terminal-new".into())
                            .icon(Icon::new(IconName::Plus).small())
                            .small()
                            .compact()
                            .ghost()
                            .tooltip(t!("terminal.new").to_string())
                            .on_click(cx.listener(|panel, _, _, cx| {
                                cx.stop_propagation();
                                panel.new_shell(panel.settings.default_profile, cx);
                            })),
                    )
                    .child(
                        Button::new("native-terminal-menu")
                            .debug_selector(|| "native-terminal-menu".into())
                            .icon(Icon::new(IconName::ChevronDown).small())
                            .small()
                            .compact()
                            .ghost()
                            .tooltip(t!("terminal.menu").to_string())
                            .on_click(cx.listener(|panel, _, _, cx| {
                                cx.stop_propagation();
                                panel.requested_menu = Some(false);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("native-terminal-hide")
                            .debug_selector(|| "native-terminal-hide".into())
                            .icon(Icon::new(IconName::Minus).small())
                            .small()
                            .compact()
                            .ghost()
                            .on_click(cx.listener(|panel, _, _, cx| {
                                cx.stop_propagation();
                                panel.set_visible(false, cx);
                            })),
                    ),
            )
    }
}
