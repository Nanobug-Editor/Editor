//! Application-owned palette and Base token projection, independent of styled component runtimes.
use crate::language::highlighter::HighlightTheme;
use gpui_base::{Root, RootPlugin};
use gpui_kit::{App, Context, Global, Hsla, Pixels, SharedString, Window, px, rgb};
use std::sync::{Arc, LazyLock};

/// Selected native application appearance.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThemeMode {
    Light,
    Dark,
}

/// Resolved product colors and typography; Base receives a projection for shared behavior.
#[derive(Clone)]
pub(crate) struct Theme {
    /// Resolved background color.
    pub background: Hsla,
    /// Resolved foreground color.
    pub foreground: Hsla,
    /// Resolved border color.
    pub border: Hsla,
    /// Resolved sidebar color.
    pub sidebar: Hsla,
    /// Resolved sidebar foreground color.
    pub sidebar_foreground: Hsla,
    /// Resolved sidebar border color.
    pub sidebar_border: Hsla,
    /// Resolved sidebar accent color.
    pub sidebar_accent: Hsla,
    /// Resolved sidebar accent foreground color.
    pub sidebar_accent_foreground: Hsla,
    /// Resolved tab bar color.
    pub tab_bar: Hsla,
    /// Resolved tab color.
    pub tab: Hsla,
    /// Resolved tab foreground color.
    pub tab_foreground: Hsla,
    /// Resolved tab active color.
    pub tab_active: Hsla,
    /// Resolved tab active foreground color.
    pub tab_active_foreground: Hsla,
    /// Resolved tab bar segmented color.
    pub tab_bar_segmented: Hsla,
    /// Resolved muted color.
    pub muted: Hsla,
    /// Resolved muted foreground color.
    pub muted_foreground: Hsla,
    /// Resolved input color.
    pub input: Hsla,
    /// Resolved list hover color.
    pub list_hover: Hsla,
    /// Resolved list active color.
    pub list_active: Hsla,
    /// Resolved list active border color.
    pub list_active_border: Hsla,
    /// Resolved selection color.
    pub selection: Hsla,
    /// Resolved accent color.
    pub accent: Hsla,
    /// Resolved accent foreground color.
    pub accent_foreground: Hsla,
    /// Resolved primary color.
    pub primary: Hsla,
    /// Resolved primary hover color.
    pub primary_hover: Hsla,
    /// Resolved primary active color.
    pub primary_active: Hsla,
    /// Resolved primary foreground color.
    pub primary_foreground: Hsla,
    /// Resolved button color.
    pub button: Hsla,
    /// Resolved button hover color.
    pub button_hover: Hsla,
    /// Resolved button active color.
    pub button_active: Hsla,
    /// Resolved button foreground color.
    pub button_foreground: Hsla,
    /// Resolved button primary color.
    pub button_primary: Hsla,
    /// Resolved button primary hover color.
    pub button_primary_hover: Hsla,
    /// Resolved button primary active color.
    pub button_primary_active: Hsla,
    /// Resolved button primary foreground color.
    pub button_primary_foreground: Hsla,
    /// Resolved popover color.
    pub popover: Hsla,
    /// Resolved popover foreground color.
    pub popover_foreground: Hsla,
    /// Resolved ring color.
    pub ring: Hsla,
    /// Resolved drag border color.
    pub drag_border: Hsla,
    /// Resolved caret color.
    pub caret: Hsla,
    /// Resolved title bar color.
    pub title_bar: Hsla,
    /// Resolved title bar border color.
    pub title_bar_border: Hsla,
    /// Resolved secondary color.
    pub secondary: Hsla,
    /// Resolved secondary foreground color.
    pub secondary_foreground: Hsla,
    /// Resolved secondary hover color.
    pub secondary_hover: Hsla,
    /// Resolved secondary active color.
    pub secondary_active: Hsla,
    /// Resolved danger color.
    pub danger: Hsla,
    /// Resolved danger active color.
    pub danger_active: Hsla,
    /// Resolved danger foreground color.
    pub danger_foreground: Hsla,
    /// Resolved success color.
    pub success: Hsla,
    /// Resolved warning color.
    pub warning: Hsla,
    /// Resolved overlay color.
    pub overlay: Hsla,
    /// Resolved transparent color.
    pub transparent: Hsla,
    /// Current appearance used by theme-sensitive assets.
    pub mode: ThemeMode,
    /// Interface font family.
    pub font_family: SharedString,
    /// Interface size in logical pixels, also used as the window rem size.
    pub font_size: Pixels,
    /// Source font family.
    pub mono_font_family: SharedString,
    /// Source font size in logical pixels.
    pub mono_font_size: Pixels,
    /// Ordinary surface corner radius.
    pub radius: Pixels,
    /// Dialog surface corner radius.
    pub radius_lg: Pixels,
    /// Semantic defaults shared with Base rendering.
    pub tokens: gpui_base::SemanticThemeTokens,
    /// Local capture resolver, including editor and diagnostic colors.
    pub highlight_theme: Arc<HighlightTheme>,
}
impl Global for Theme {}
impl Default for Theme {
    fn default() -> Self {
        Self::for_mode(ThemeMode::Light)
    }
}

/// Local renderers read the same product palette through any application context.
pub(crate) trait ActiveTheme {
    fn theme(&self) -> &Theme;
}
impl ActiveTheme for App {
    fn theme(&self) -> &Theme {
        Theme::global(self)
    }
}

impl Theme {
    /// Resolve a baseline palette before project theme resources are applied.
    fn for_mode(mode: ThemeMode) -> Self {
        let dark = mode == ThemeMode::Dark;
        let tokens = gpui_base::SemanticThemeTokens::default();
        let foreground = Hsla::from(rgb(if dark { 0xe4e4e7 } else { 0x18181b }));
        let background = Hsla::from(rgb(if dark { 0x18181b } else { 0xffffff }));
        let danger = Hsla::from(rgb(if dark { 0xf87171 } else { 0xdc2626 }));
        Self {
            background: background,
            foreground: foreground,
            border: background,
            sidebar: background,
            sidebar_foreground: foreground,
            sidebar_border: background,
            sidebar_accent: background,
            sidebar_accent_foreground: foreground,
            tab_bar: background,
            tab: background,
            tab_foreground: foreground,
            tab_active: background,
            tab_active_foreground: foreground,
            tab_bar_segmented: background,
            muted: background,
            muted_foreground: rgb(if dark { 0xa1a1aa } else { 0x71717a }).into(),
            input: background,
            list_hover: background,
            list_active: background,
            list_active_border: background,
            selection: background,
            accent: background,
            accent_foreground: foreground,
            primary: background,
            primary_hover: background,
            primary_active: background,
            primary_foreground: foreground,
            button: background,
            button_hover: background,
            button_active: background,
            button_foreground: foreground,
            button_primary: background,
            button_primary_hover: background,
            button_primary_active: background,
            button_primary_foreground: foreground,
            popover: background,
            popover_foreground: foreground,
            ring: background,
            drag_border: background,
            caret: background,
            title_bar: background,
            title_bar_border: background,
            secondary: background,
            secondary_foreground: foreground,
            secondary_hover: background,
            secondary_active: background,
            danger: danger,
            danger_active: danger,
            danger_foreground: gpui_kit::white(),
            success: rgb(0x22c55e).into(),
            warning: rgb(0xeab308).into(),
            overlay: gpui_kit::black().opacity(0.5),
            transparent: gpui_kit::transparent_black(),
            mode,
            tokens,
            font_family: ".SystemUIFont".into(),
            font_size: px(14.),
            mono_font_family: if cfg!(target_os = "windows") {
                "Consolas"
            } else if cfg!(target_os = "macos") {
                "Menlo"
            } else {
                "DejaVu Sans Mono"
            }
            .into(),
            mono_font_size: px(14.),
            radius: px(6.),
            radius_lg: px(8.),
            highlight_theme: if dark {
                HighlightTheme::default_dark()
            } else {
                HighlightTheme::default_light()
            },
        }
    }
    /// Read a fallback during early initialization; mutable application state remains in App.
    pub(crate) fn global(cx: &App) -> &Self {
        static DEFAULT: LazyLock<Theme> = LazyLock::new(Theme::default);
        cx.try_global::<Self>().unwrap_or(&DEFAULT)
    }
    /// Initialize the local palette before returning its sole mutable application instance.
    pub(crate) fn global_mut(cx: &mut App) -> &mut Self {
        if !cx.has_global::<Self>() {
            cx.set_global(Self::default());
        }
        cx.global_mut::<Self>()
    }
    /// Whether dark artwork should be selected.
    pub(crate) fn is_dark(&self) -> bool {
        self.mode == ThemeMode::Dark
    }
    /// Reset the appearance baseline and register local Root presentation for future windows.
    pub(crate) fn change(mode: ThemeMode, _: Option<&mut Window>, cx: &mut App) {
        cx.set_global(Self::for_mode(mode));
        Root::register_plugin(cx, |_, _| PaletteRoot);
        crate::ui::controls::overlay::init(cx);
    }
    /// Mutate local product tokens before synchronizing their Base projection.
    pub(crate) fn update<R>(cx: &mut App, edit: impl FnOnce(&mut Self) -> R) -> R {
        edit(Self::global_mut(cx))
    }
    /// Project resolved colors without replacing Base scroll, motion or resize preferences.
    pub(crate) fn sync_base(cx: &mut App) {
        let theme = Self::global_mut(cx);
        let colors = &mut theme.tokens.colors;
        colors.background = theme.background;
        colors.foreground = theme.foreground;
        colors.surface = theme.popover;
        colors.surface_foreground = theme.popover_foreground;
        colors.primary = theme.primary;
        colors.primary_foreground = theme.primary_foreground;
        colors.secondary = theme.button;
        colors.secondary_foreground = theme.foreground;
        colors.muted = theme.muted;
        colors.muted_foreground = theme.muted_foreground;
        colors.accent = theme.accent;
        colors.accent_foreground = theme.accent_foreground;
        colors.destructive = theme.danger;
        colors.destructive_foreground = theme.danger_foreground;
        colors.border = theme.border;
        colors.input = theme.input;
        colors.ring = theme.ring;
        colors.selection = theme.selection;
        let tokens = theme.tokens.clone();
        let appearance = if theme.is_dark() {
            gpui_base::ThemeAppearance::Dark
        } else {
            gpui_base::ThemeAppearance::Light
        };
        let base = gpui_base::Theme::global_mut(cx);
        base.tokens = tokens;
        base.appearance = appearance;
    }
}

/// Base Root owns content, focus and overlays; this plugin only supplies product typography.
struct PaletteRoot;
impl RootPlugin for PaletteRoot {
    fn prepare(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.set_rem_size(Theme::global(cx).font_size);
    }
    fn style(&self, surface: &mut gpui_kit::Stateful<gpui_kit::Div>, _: &mut Window, cx: &mut App) {
        use gpui_kit::Styled;
        let theme = Theme::global(cx);
        surface.style().text.font_family = Some(theme.font_family.clone());
        surface.style().text.font_size = Some(theme.font_size.into());
        surface.style().text.color = Some(theme.foreground);
    }
}
impl gpui_kit::Render for PaletteRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui_kit::IntoElement {
        gpui_kit::div()
    }
}
