//! User settings expose provider selection, theme colors and stable title-entry ordering.
use super::*;
use editor_ui::{Checkbox, Input};
use gpui_base::input::InputEvent;

/// Render workspace presentation controls in the existing native appearance page.
pub(crate) fn render(
    view: &Entity<EditorApp>,
    window: &mut Window,
    cx: &mut App,
) -> gpui_kit::AnyElement {
    view.update(cx, |app, cx| app.sync_workspace_presentation(cx));
    let (preferences, candidates, entries, error, workspace) = {
        let app = view.read(cx);
        (
            app.workspace_presentation.preferences.clone(),
            app.workspace_presentation.candidates.clone(),
            app.presentation_entries(true),
            app.workspace_presentation.error.clone(),
            app.workspace.root().to_string_lossy().to_string(),
        )
    };
    let toggle = view.clone();
    let mut content = v_flex()
        .gap_3()
        .child(
            div()
                .font_semibold()
                .child(t!("workspace_presentation.settings").to_string()),
        )
        .child(
            Checkbox::new("presentation-filename-colors")
                .label(t!("workspace_presentation.enabled").to_string())
                .checked(preferences.enabled)
                .on_change(move |enabled, _, cx| {
                    let enabled = *enabled;
                    toggle.update(cx, |app, cx| {
                        app.update_presentation_preferences(
                            |preferences| preferences.enabled = enabled,
                            cx,
                        )
                    });
                }),
        )
        .child(
            div()
                .text_sm()
                .child(t!("workspace_presentation.provider").to_string()),
        );
    if candidates.is_empty() {
        content = content.child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(t!("workspace_presentation.no_provider").to_string()),
        );
    }
    if candidates.len() > 1 && !preferences.providers.contains_key(&workspace) {
        content = content.child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(t!("workspace_presentation.choose_provider").to_string()),
        );
    }
    for (id, name) in candidates {
        let selected = preferences.providers.get(&workspace) == Some(&id);
        let owner = view.clone();
        let workspace = workspace.clone();
        let debug = format!("presentation-provider-{id}");
        content = content.child(
            Button::new(SharedString::from(debug.clone()))
                .debug_selector(move || debug.clone())
                .small()
                .label(name)
                .when(selected, |button| button.primary())
                .on_click(move |_, _, cx| {
                    owner.update(cx, |app, cx| {
                        app.update_presentation_preferences(
                            |preferences| {
                                preferences.providers.insert(workspace.clone(), id.clone());
                            },
                            cx,
                        )
                    });
                }),
        );
    }
    let dark = cx.theme().is_dark();
    content = content
        .child(div().text_sm().child(if dark {
            t!("workspace_presentation.dark_colors").to_string()
        } else {
            t!("workspace_presentation.light_colors").to_string()
        }))
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(t!("workspace_presentation.color_hint").to_string()),
        );
    for (index, label) in [
        t!("workspace_presentation.positive"),
        t!("workspace_presentation.informative"),
        t!("workspace_presentation.negative"),
    ]
    .into_iter()
    .enumerate()
    {
        let input = color_input(view, index, dark, window, cx);
        let tone = [
            tree::Tone::Positive,
            tree::Tone::Informative,
            tree::Tone::Negative,
        ][index];
        content = content.child(
            h_flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .w(px(16.))
                        .h(px(16.))
                        .rounded_sm()
                        .bg(preferences.color(tone, cx)),
                )
                .child(div().w(px(170.)).child(label.to_string()))
                .child(
                    div()
                        .id(format!("presentation-color-{index}"))
                        .debug_selector(move || format!("presentation-color-{index}").into())
                        .w(px(130.))
                        .child(Input::new(&input)),
                ),
        );
    }
    content = content.child(
        div()
            .font_semibold()
            .mt_3()
            .child(t!("workspace_presentation.entries").to_string()),
    );
    for (index, entry) in entries.iter().enumerate() {
        let key = entry.key.clone();
        let toggle = view.clone();
        let toggle_key = key.clone();
        let up = view.clone();
        let down = view.clone();
        let keys = entries
            .iter()
            .map(|entry| entry.key.clone())
            .collect::<Vec<_>>();
        let up_keys = keys.clone();
        let debug = format!("presentation-entry-{key}");
        content = content.child(
            h_flex()
                .id(SharedString::from(debug.clone()))
                .debug_selector(move || debug.clone())
                .items_center()
                .gap_2()
                .child(
                    Checkbox::new(SharedString::from(format!("presentation-visible-{key}")))
                        .checked(!preferences.hidden.contains(&key))
                        .label(
                            entry
                                .data
                                .label
                                .for_locale(rust_i18n::locale().as_ref())
                                .to_owned(),
                        )
                        .on_change(move |visible, _, cx| {
                            let visible = *visible;
                            toggle.update(cx, |app, cx| {
                                app.update_presentation_preferences(
                                    |preferences| {
                                        if visible {
                                            preferences.hidden.remove(&toggle_key);
                                        } else {
                                            preferences.hidden.insert(toggle_key.clone());
                                        }
                                    },
                                    cx,
                                )
                            });
                        }),
                )
                .child(
                    Button::new(SharedString::from(format!("presentation-up-{key}")))
                        .icon(IconName::ArrowUp)
                        .small()
                        .compact()
                        .ghost()
                        .disabled(index == 0)
                        .tooltip(t!("workspace_presentation.move_up").to_string())
                        .on_click(move |_, _, cx| {
                            reorder(&up, up_keys.clone(), index, index.saturating_sub(1), cx)
                        }),
                )
                .child(
                    Button::new(SharedString::from(format!("presentation-down-{key}")))
                        .icon(IconName::ArrowDown)
                        .small()
                        .compact()
                        .ghost()
                        .disabled(index + 1 == entries.len())
                        .tooltip(t!("workspace_presentation.move_down").to_string())
                        .on_click(move |_, _, cx| {
                            reorder(&down, keys.clone(), index, index + 1, cx)
                        }),
                ),
        );
    }
    content
        .children(error.map(|error| {
            div()
                .text_sm()
                .text_color(cx.theme().danger)
                .child(t!("workspace_presentation.error", details = error).to_string())
        }))
        .into_any_element()
}

/// Stable Base inputs retain unfinished hex entry and Chinese IME across settings redraws.
fn color_input(
    view: &Entity<EditorApp>,
    index: usize,
    dark: bool,
    window: &mut Window,
    cx: &mut App,
) -> Entity<InputState> {
    view.update(cx, |app, cx| {
        let key = format!("{dark}/{index}");
        if let Some(input) = app.workspace_presentation.color_inputs.get(&key) {
            return input.clone();
        }
        let colors = if dark {
            &app.workspace_presentation.preferences.dark
        } else {
            &app.workspace_presentation.preferences.light
        };
        let value = colors[index]
            .map(|rgb| format!("#{rgb:06X}"))
            .unwrap_or_default();
        let input = cx.new(|cx| {
            let mut input = InputState::new(window, cx)
                .placeholder(t!("workspace_presentation.theme_default").to_string());
            input.set_value(value, window, cx);
            input
        });
        let subscription = cx.subscribe_in(
            &input,
            window,
            move |app, input, event: &InputEvent, _, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let value = input.read(cx).value();
                let value = value.trim();
                let color = if value.is_empty() {
                    Some(None)
                } else {
                    value
                        .strip_prefix('#')
                        .filter(|value| value.len() == 6)
                        .and_then(|value| u32::from_str_radix(value, 16).ok())
                        .map(Some)
                };
                if let Some(color) = color {
                    app.update_presentation_preferences(
                        |preferences| {
                            if dark {
                                preferences.dark[index] = color;
                            } else {
                                preferences.light[index] = color;
                            }
                        },
                        cx,
                    );
                } else {
                    app.workspace_presentation.error =
                        Some(t!("workspace_presentation.invalid_color").to_string());
                    app.refresh_dialog(cx);
                }
            },
        );
        app.workspace_presentation.subscriptions.push(subscription);
        app.workspace_presentation
            .color_inputs
            .insert(key, input.clone());
        input
    })
}

/// Reorder visible candidates while retaining inactive plugin records for later reinstallation.
fn reorder(view: &Entity<EditorApp>, mut keys: Vec<String>, from: usize, to: usize, cx: &mut App) {
    if from >= keys.len() || to >= keys.len() {
        return;
    }
    keys.swap(from, to);
    view.update(cx, |app, cx| {
        app.update_presentation_preferences(
            |preferences| {
                keys.extend(
                    preferences
                        .order
                        .iter()
                        .filter(|key| !keys.contains(key))
                        .cloned()
                        .collect::<Vec<_>>(),
                );
                preferences.order = keys;
            },
            cx,
        )
    });
}
