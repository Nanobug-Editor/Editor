//! Template selection reuses the local popup's focus, keyboard, scrolling and dismissal behavior.
use super::*;
use editor_ui::menu::MenuRowContent;
#[cfg(all(test, windows))]
mod tests;

/// One window-local popup retains its catalog snapshot so asynchronous discovery can refresh it.
pub(super) struct TemplateMenu {
    /// The shared local menu owns all transient interaction and focus state.
    pub popup: Entity<NativePopupMenu>,
    /// Only changed template publications replace rows, preserving keyboard selection between paints.
    catalog: Vec<(String, contract::Template)>,
}

/// Open the current form's templates below the measured add button, in native window coordinates.
pub(super) fn open(
    app: &Entity<EditorApp>,
    form: &Entity<RunConfigForm>,
    trigger: gpui_kit::Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let state = form.read(cx).plugin.as_ref().unwrap();
    if state.commit.is_some() || state.template_menu.is_some() {
        return;
    }
    let catalog = state.catalog.clone();
    // Mouse and keyboard activation share the add button as the popup's explicit return target.
    let focus = state.template_trigger_focus.clone();
    focus.focus(window, cx);
    let (items, content) = rows(&catalog);
    let owner = app.downgrade();
    let origin = form.downgrade();
    let popup = cx.new(|cx| {
        NativePopupMenu::new(
            items,
            MenuStyle::current(cx),
            point(trigger.left(), trigger.bottom()),
            move |action, _, cx| {
                let Some(form) = origin.upgrade() else { return };
                let target = form.update(cx, |form, cx| {
                    let state = form.plugin.as_mut()?;
                    let target = match action {
                        native::Action::Select(ref id) => state
                            .catalog
                            .iter()
                            .find(|(provider, template)| {
                                item_id(provider, template) == *id && template.unavailable.is_none()
                            })
                            .cloned(),
                        _ => None,
                    };
                    state.template_menu = None;
                    cx.notify();
                    target
                });
                if let Some((provider, template)) = target {
                    let _ = owner.update(cx, |app, cx| {
                        // A retired window cannot add a draft to a newly opened configuration dialog.
                        if app
                            .run_form
                            .as_ref()
                            .is_some_and(|current| current == &form)
                        {
                            app.add_plugin_configuration(provider, template, cx);
                        }
                    });
                }
            },
            window,
            cx,
        )
        .width(260.)
        .max_height(360.)
        .below_anchor()
        .row_content(content)
    });
    form.update(cx, |form, cx| {
        form.plugin.as_mut().unwrap().template_menu = Some(TemplateMenu { popup, catalog });
        cx.notify();
    });
}

/// Reconcile asynchronous template discovery without replacing the menu's focus lease or scroll owner.
pub(super) fn synchronize(form: &Entity<RunConfigForm>, cx: &mut App) {
    let changed = form.update(cx, |form, _| {
        let state = form.plugin.as_mut()?;
        let menu = state.template_menu.as_mut()?;
        if menu.catalog == state.catalog {
            return None;
        }
        menu.catalog = state.catalog.clone();
        Some((menu.popup.clone(), rows(&menu.catalog)))
    });
    if let Some((popup, (items, content))) = changed {
        popup.update(cx, |popup, cx| popup.replace_rows(items, content, cx));
    }
}

/// Retain the existing provider/template identity for menu actions and native test selectors.
fn item_id(provider: &str, template: &contract::Template) -> String {
    format!("run-template-{provider}-{}", template.id)
}

/// Project the provider catalog into inert group headings and icon rows; no command is executed.
fn rows(
    catalog: &[(String, contract::Template)],
) -> (Vec<native::MenuItem>, BTreeMap<String, MenuRowContent>) {
    let mut groups = BTreeMap::<&str, Vec<_>>::new();
    for (provider, template) in catalog {
        groups
            .entry(&template.group)
            .or_default()
            .push((provider, template));
    }
    let mut items = Vec::new();
    let mut content = BTreeMap::new();
    for (index, (group, templates)) in groups.into_iter().enumerate() {
        let id = format!("run-template-group-{index}");
        items.push(native::MenuItem {
            id: id.clone(),
            label: group.into(),
            disabled: true,
            separator_before: index > 0,
        });
        content.insert(id, MenuRowContent::Heading);
        for (provider, template) in templates {
            let id = item_id(provider, template);
            items.push(native::MenuItem {
                id: id.clone(),
                label: template.label.clone(),
                disabled: template.unavailable.is_some(),
                separator_before: false,
            });
            content.insert(
                id,
                MenuRowContent::Icon {
                    icon: render::template_icon(&template.icon),
                    tooltip: template
                        .unavailable
                        .clone()
                        .unwrap_or_else(|| provider.clone()),
                },
            );
        }
    }
    if items.is_empty() {
        items.push(native::MenuItem {
            id: "run-template-empty".into(),
            label: t!("run.plugin_no_templates").into(),
            disabled: true,
            separator_before: false,
        });
    }
    (items, content)
}
