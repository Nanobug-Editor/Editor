//! User theme tokens override bounded plugin content defaults and generic native chrome.
use super::*;
use crate::ui::theme::ActiveTheme as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{FontWeight, Hsla, Styled};
use gpui_kit::{px, rgb};

pub(super) struct Colors {
    pub background: Hsla,
    pub foreground: Hsla,
    pub muted_foreground: Hsla,
    pub border: Hsla,
    pub hover: Hsla,
    pub active: Hsla,
    pub accent: Hsla,
    pub accent_foreground: Hsla,
    /// Fenced code and table headers use the document's neutral block surface.
    pub code_background: Hsla,
    /// Primer uses translucent neutral inline code independently of block/table surfaces.
    pub inline_code_background: Hsla,
}

impl PluginView {
    /// Style `button` for `node` using the active theme in `cx`; explicit guest/user tokens override defaults.
    /// Primary actions share plugin management's outlined primary button, including hover and focus.
    pub(super) fn button_appearance(
        &self,
        button: editor_ui::Button,
        node: &ui::Node,
        cx: &App,
    ) -> editor_ui::Button {
        let role = node.theme_role();
        let button = match role {
            "primary_button" => button.primary().outline(),
            "toolbar_button" => button.ghost(),
            _ => button,
        };
        let custom = !matches!(role, "button" | "primary_button" | "toolbar_button")
            || [
                "background",
                "foreground",
                "hover_background",
                "active_background",
            ]
            .iter()
            .any(|key| {
                self.environment
                    .color(&self.plugin, &format!("ui.{role}.{key}"))
                    .is_some()
                    || self
                        .document
                        .content_colors
                        .contains_key(&format!("{role}.{key}"))
            });
        let button = button.when(custom, |button| {
            let colors = self.colors(role, cx);
            button.custom(
                editor_ui::ButtonCustomVariant::new(cx)
                    .color(colors.background)
                    .foreground(colors.foreground)
                    .hover(colors.hover)
                    .active(colors.active),
            )
        });
        // Ordinary buttons inherit exactly the shared component's font and sizing. Explicit role
        // typography still follows the public override contract instead of a second button preset.
        if self
            .environment
            .theme_text_styles
            .contains_key(&format!("{}.{role}", self.plugin))
            || self.document.content_styles.contains_key(role)
        {
            self.font(button, role)
        } else {
            button
        }
    }

    pub(super) fn colors(&self, role: &str, cx: &App) -> Colors {
        let palette = cx.theme();
        // User theme tokens override the current plugin's bounded document defaults.
        // Domain palettes are supplied by the guest; native rendering has no plugin preset.
        let color = |key: &str, fallback: Hsla| {
            self.environment
                .color(&self.plugin, &format!("ui.{role}.{key}"))
                .or_else(|| {
                    self.document
                        .content_colors
                        .get(&format!("{role}.{key}"))
                        .copied()
                })
                .map(|c| rgb(c).into())
                .unwrap_or(fallback)
        };
        Colors {
            background: color(
                "background",
                if role == "primary_button" {
                    palette.primary
                } else if role == "toolbar_button" {
                    palette.transparent
                } else if role == "button" {
                    palette.button
                } else {
                    palette.background
                },
            ),
            foreground: color(
                "foreground",
                if role == "primary_button" {
                    palette.primary_foreground
                } else if role == "button" {
                    palette.button_foreground
                } else {
                    palette.foreground
                },
            ),
            muted_foreground: color("muted_foreground", palette.muted_foreground),
            border: color(
                "border",
                if role == "input" {
                    palette.input
                } else {
                    palette.border
                },
            ),
            hover: color(
                "hover_background",
                if role == "button" {
                    palette.button_hover
                } else if role == "primary_button" {
                    palette.primary
                } else {
                    palette.list_hover
                },
            ),
            active: color(
                "active_background",
                if role == "button" {
                    palette.button_active
                } else if role == "primary_button" {
                    palette.primary
                } else {
                    palette.list_active
                },
            ),
            accent: color("accent", palette.primary),
            accent_foreground: color("accent_foreground", palette.primary_foreground),
            code_background: color("code_background", color("background", palette.background)),
            inline_code_background: color(
                "inline_code_background",
                color("code_background", color("background", palette.background)),
            ),
        }
    }

    pub(super) fn font<T: Styled>(&self, element: T, role: &str) -> T {
        let font = self.content_font(role, false);
        let element = element
            .font_family(font.family.unwrap_or_else(|| "Segoe UI".into()))
            .text_size(px(font.size_px.unwrap_or(14.) * self.native_scale))
            .font_weight(if font.bold.unwrap_or(false) {
                FontWeight::BOLD
            } else {
                FontWeight::NORMAL
            });
        if let Some(height) = self
            .document
            .content_styles
            .get(role)
            .and_then(|style| style.line_height)
        {
            element.line_height(gpui_kit::relative(height))
        } else {
            element
        }
    }

    /// Guest presentation supplies content defaults; explicit user role typography wins per field.
    pub(super) fn content_font(
        &self,
        role: &str,
        monospace: bool,
    ) -> plugin_runtime::plugin_protocol::FontStyle {
        let base = self.environment.font_style(&self.plugin, role, monospace);
        let mut font = self
            .document
            .content_styles
            .get(role)
            .map(|style| style.font.clone())
            .unwrap_or_default();
        // A prose family cannot replace the user's monospace family in literal code.
        if monospace {
            font.family = None;
        }
        let font = font.over(base);
        self.environment
            .theme_text_styles
            .get(&format!("{}.{role}", self.plugin))
            .cloned()
            .unwrap_or_default()
            .over(font)
    }
}
