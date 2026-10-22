//! Owns the settings navigation and the content shown by each category.

use crate::*;
use editor_ui::{Checkbox, DialogContent};
mod history;
mod owner;
use editor_ui::scroll::ScrollableElement;
use gpui_kit::{AnyElement, App, Entity};
use owner::SettingsOwner;

/// Top-level settings categories shown in the navigation sidebar.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsSection {
    AppearanceAndBehavior,
    Keymap,
    Editor,
    Plugins,
    Languages,
    /// Profile-wide local history applies even before a workspace is open.
    History,
}

impl SettingsSection {
    /// Returns the localized category name used in the sidebar and page heading.
    fn label(self) -> String {
        match self {
            Self::AppearanceAndBehavior => t!("settings.appearance_and_behavior").to_string(),
            Self::Keymap => t!("settings.keymap").to_string(),
            Self::Editor => t!("settings.editor").to_string(),
            Self::Plugins => t!("settings.plugins").to_string(),
            Self::Languages => t!("settings.languages").to_string(),
            Self::History => t!("history.title").to_string(),
        }
    }

    /// Provides a stable selector for interaction tests and accessibility tooling.
    fn selector(self) -> &'static str {
        match self {
            Self::AppearanceAndBehavior => "settings-nav-appearance",
            Self::Keymap => "settings-nav-keymap",
            Self::Editor => "settings-nav-editor",
            Self::Plugins => "settings-nav-plugins",
            Self::Languages => "settings-nav-languages",
            Self::History => "settings-nav-history",
        }
    }
}

impl EditorApp {
    /// Project settings and empty-window settings share the same application controls.
    pub(super) fn render_settings_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        trigger(SettingsOwner::Project(cx.entity()))
    }
}
/// Empty startup can configure the application before a workspace exists.
pub(in crate::app) fn empty_trigger(
    view: Entity<app::projects::ProjectWindow>,
) -> impl IntoElement {
    trigger(SettingsOwner::Empty(view))
}
/// Each owner retains one native settings dialog and closes it with its root window.
fn trigger(view: SettingsOwner) -> impl IntoElement {
    let owner = view.clone();
    let existing = view.clone();
    div()
        .debug_selector(|| "settings-trigger".into())
        .child(app_dialog::app_dialog(
            Button::new("open-settings")
                .icon(IconName::Settings)
                .small()
                .compact()
                .ghost()
                .tooltip(t!("settings.title").to_string()),
            t!("settings.title").to_string(),
            move |content, window, cx| render_settings_content(&view, content, window, cx),
            move |cx| existing.dialog_window(cx),
            move |dialog, handle, _, cx| owner.retain_dialog(dialog, handle, cx),
        ))
}

/// Places persistent category navigation beside the independently scrollable page.
fn render_settings_content(
    view: &SettingsOwner,
    content: DialogContent,
    window: &mut Window,
    cx: &mut App,
) -> DialogContent {
    let selected = view.section(cx);
    let nav = [
        SettingsSection::AppearanceAndBehavior,
        SettingsSection::Keymap,
        SettingsSection::Editor,
        SettingsSection::Plugins,
        SettingsSection::Languages,
        SettingsSection::History,
    ]
    .into_iter()
    .filter(|section| view.project().is_some() || *section != SettingsSection::Languages)
    .map(|section| {
        let nav_view = view.clone();
        div()
            .id(section.selector())
            .debug_selector(move || section.selector().into())
            .w_full()
            .h(px(34.))
            .px_3()
            .flex()
            .items_center()
            .rounded(cx.theme().radius)
            .cursor_pointer()
            .when(selected == section, |item| {
                item.bg(cx.theme().list_active)
                    .text_color(cx.theme().foreground)
                    .font_semibold()
            })
            .when(selected != section, |item| {
                item.text_color(cx.theme().sidebar_foreground)
                    .hover(|style| style.bg(cx.theme().list_hover))
            })
            .child(section.label())
            .on_click(move |_, _, cx| {
                nav_view.select_section(section, cx);
            })
    });

    content.h_full().child(
        h_flex()
            .h_full()
            .w_full()
            .items_start()
            .child(
                v_flex()
                    .id("settings-sidebar")
                    .w(px(214.))
                    .h_full()
                    .flex_shrink_0()
                    .gap_1()
                    .p_2()
                    .border_r_1()
                    .border_color(cx.theme().sidebar_border)
                    .bg(cx.theme().sidebar)
                    .overflow_y_scrollbar()
                    .children(nav),
            )
            .child(
                v_flex()
                    .id("settings-page")
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .gap_5()
                    .p_5()
                    .overflow_y_scrollbar()
                    .child(div().text_lg().font_semibold().child(selected.label()))
                    .child(render_section(view, selected, window, cx)),
            ),
    )
}

/// Keeps each category's controls in one content pane.
fn render_section(
    view: &SettingsOwner,
    section: SettingsSection,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    match section {
        SettingsSection::AppearanceAndBehavior => {
            let workspace = view.project().map(|project| {
                v_flex()
                    .gap_6()
                    .child(render_workspace_appearance(&project, cx))
                    .child(extensions::workspace_presentation::settings::render(
                        &project, window, cx,
                    ))
            });
            v_flex()
                .gap_6()
                .child(render_theme(view, cx))
                .child(render_startup(view, cx))
                .children(workspace)
                .into_any_element()
        }
        SettingsSection::Keymap => render_keymap(cx),
        SettingsSection::Editor => render_editor(view, cx),
        SettingsSection::Languages => view
            .project()
            .map(|project| super::languages::render_settings(&project, cx))
            .unwrap_or_else(|| div().into_any_element()),
        SettingsSection::Plugins => view.plugin_form(cx).into_any_element(),
        SettingsSection::History => history::render(view, window, cx),
    }
}

/// Startup policy is application-wide and remains editable before choosing a workspace.
fn render_startup(view: &SettingsOwner, cx: &App) -> AnyElement {
    let preferences = view.startup_preferences(cx);
    let enabled = preferences
        .as_ref()
        .map_or(true, |(_, preferences)| preferences.restore_last_project);
    let error = preferences
        .as_ref()
        .err()
        .map(ToString::to_string)
        .or_else(|| view.startup_error(cx));
    let change_view = view.clone();
    v_flex()
        .gap_3()
        .child(
            div()
                .text_sm()
                .font_semibold()
                .child(t!("settings.startup").to_string()),
        )
        .child(
            div()
                .id("settings-restore-last-project")
                .debug_selector(|| "settings-restore-last-project".into())
                .child(
                    Checkbox::new("settings-restore-last-project-checkbox")
                        .label(t!("settings.restore_last_project").to_string())
                        .checked(enabled)
                        .disabled(preferences.is_err())
                        .on_change(move |checked, _, cx| {
                            change_view.set_restore_last_project(*checked, cx);
                        }),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(t!("settings.restore_last_project_hint").to_string()),
        )
        .children(error.map(|error| {
            div()
                .debug_selector(|| "settings-startup-error".into())
                .text_xs()
                .text_color(cx.theme().danger)
                .child(t!("settings.startup_error", details = error).to_string())
        }))
        .into_any_element()
}

/// Theme selection is application-wide, including all already open native editor windows.
fn render_theme(view: &SettingsOwner, cx: &App) -> AnyElement {
    let dark = cx.theme().is_dark();
    let light_view = view.clone();
    let dark_view = view.clone();
    v_flex()
        .gap_3()
        .child(
            div()
                .text_sm()
                .font_semibold()
                .child(t!("settings.appearance").to_string()),
        )
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .child(div().child(t!("settings.theme").to_string()))
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("settings-theme-light")
                                .label(t!("settings.light").to_string())
                                .debug_selector(|| "settings-theme-light".into())
                                .when(!dark, |button| button.primary())
                                .on_click(move |_, window, cx| {
                                    light_view.set_theme(false, window, cx)
                                }),
                        )
                        .child(
                            Button::new("settings-theme-dark")
                                .label(t!("settings.dark").to_string())
                                .debug_selector(|| "settings-theme-dark".into())
                                .when(dark, |button| button.primary())
                                .on_click(move |_, window, cx| {
                                    dark_view.set_theme(true, window, cx)
                                }),
                        ),
                ),
        )
        .into_any_element()
}

/// Renders the existing theme and file explorer controls under appearance.
fn render_workspace_appearance(view: &Entity<EditorApp>, cx: &mut App) -> AnyElement {
    let (explorer_visible, reveal_on_tab_switch, workspace_trusted) = {
        let settings = view.read(cx);
        (
            settings.explorer_visible,
            settings.session_state.explorer_reveal_on_tab_switch,
            settings.session_state.workspace_trusted,
        )
    };
    let explorer_view = view.clone();
    let reveal_view = view.clone();
    let trust_view = view.clone();

    v_flex()
        .gap_6()
        .child(
            Checkbox::new("settings-workspace-trust")
                .label(t!("settings.workspace_trusted").to_string())
                .checked(workspace_trusted)
                .on_change(move |checked, _, cx| {
                    trust_view.update(cx, |this, cx| this.set_workspace_trusted(*checked, cx));
                }),
        )
        .child(
            v_flex()
                .gap_3()
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .child(t!("settings.layout").to_string()),
                )
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .child(div().child(t!("settings.file_explorer").to_string()))
                        .child(
                            Button::new("settings-explorer-visibility")
                                .label(if explorer_visible {
                                    t!("settings.visible").to_string()
                                } else {
                                    t!("settings.hidden").to_string()
                                })
                                .on_click(move |_, _, cx| {
                                    // This control lives in the settings HWND; dock reconciliation
                                    // and focus continue in the original editor window.
                                    let owner = explorer_view.read(cx).owner_window;
                                    let _ = owner.update(cx, |_, window, cx| {
                                        explorer_view.update(cx, |this, cx| {
                                            this.toggle_explorer(window, cx)
                                        });
                                    });
                                }),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(t!("settings.resize_hint").to_string()),
                )
                .child(
                    div()
                        .id("settings-explorer-reveal")
                        .debug_selector(|| "settings-explorer-reveal".into())
                        .child(
                            Checkbox::new("settings-explorer-reveal-checkbox")
                                .label(t!("settings.explorer_reveal_on_tab_switch").to_string())
                                .checked(reveal_on_tab_switch)
                                .on_change(move |checked, _, cx| {
                                    // Apply immediately and retain the choice when reopening this workspace.
                                    reveal_view.update(cx, |this, cx| {
                                        this.session_state.explorer_reveal_on_tab_switch = *checked;
                                        this.persist_session();
                                        this.refresh_dialog(cx);
                                        cx.notify();
                                    });
                                }),
                        ),
                ),
        )
        .into_any_element()
}

/// Shows the category without inventing shortcut editing before a keymap model exists.
fn render_keymap(cx: &mut App) -> AnyElement {
    div()
        .debug_selector(|| "settings-keymap-empty".into())
        .text_color(cx.theme().muted_foreground)
        .child(t!("settings.keymap_empty").to_string())
        .into_any_element()
}

/// Renders the shared interface and editor font size control.
fn render_editor(view: &SettingsOwner, cx: &mut App) -> AnyElement {
    let font_size = typography::font_size(cx) / px(1.);
    let decrease_view = view.clone();
    let increase_view = view.clone();

    v_flex()
        .gap_3()
        .child(
            h_flex()
                .debug_selector(|| "settings-font-size".into())
                .items_center()
                .justify_between()
                .child(div().child(t!("settings.font_size").to_string()))
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("font-size-decrease")
                                .debug_selector(|| "font-size-decrease".into())
                                .label("−")
                                .on_click(move |_, window, cx| {
                                    decrease_view.step_font(-1, window, cx);
                                }),
                        )
                        .child(
                            div().w(px(52.)).text_center().child(
                                t!("settings.font_size_value", size = format!("{font_size:.0}"))
                                    .to_string(),
                            ),
                        )
                        .child(
                            Button::new("font-size-increase")
                                .debug_selector(|| "font-size-increase".into())
                                .label("+")
                                .on_click(move |_, window, cx| {
                                    increase_view.step_font(1, window, cx);
                                }),
                        ),
                ),
        )
        .into_any_element()
}
