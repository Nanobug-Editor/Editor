//! Native title placement, overflow and callback admission are shared by every workspace plugin.
use super::*;
use editor_ui::menu::ActionMenuEvent;
use gpui_kit::{canvas, point};
use std::{cell::Cell, rc::Rc};

impl EditorApp {
    /// Mount stable native entries outside the window drag region, preserving a sorted overflow tail.
    pub(crate) fn render_workspace_entries(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let entries = self.presentation_entries(false);
        let budget = (window.viewport_size().width / px(1.) - 740.).clamp(0., 560.);
        let mut used = 0.;
        let mut visible = 0;
        for entry in &entries {
            let width = (entry
                .data
                .label
                .for_locale(rust_i18n::locale().as_ref())
                .chars()
                .count() as f32
                * 8.
                + 52.)
                .clamp(80., 190.);
            if used + width + if visible + 1 < entries.len() { 30. } else { 0. } > budget {
                break;
            }
            used += width;
            visible += 1;
        }
        self.reconcile_presentation_popup(&entries, visible, window, cx);
        let mut bar = h_flex().h_full().min_w_0().gap_1().items_center();
        for entry in entries.iter().take(visible) {
            let anchor = Rc::new(Cell::new(point(px(0.), px(0.))));
            let measure = anchor.clone();
            let entry = entry.clone();
            let tooltip = entry
                .data
                .tooltip
                .for_locale(rust_i18n::locale().as_ref())
                .to_owned();
            let label = entry
                .data
                .label
                .for_locale(rust_i18n::locale().as_ref())
                .to_owned();
            let icon = entry
                .resource
                .snapshot()
                .icons
                .get(entry.data.icon.path(cx.theme().is_dark()))
                .cloned();
            let key = entry.key.clone();
            let debug = format!("workspace-entry-{key}");
            let button = Button::new(SharedString::from(debug.clone()))
                .debug_selector(move || debug.clone())
                .small()
                .ghost()
                .disabled(entry.data.disabled)
                .accessibility_label(label.clone())
                .tooltip(tooltip)
                .max_w(px(190.))
                .min_w_0()
                .child(
                    h_flex()
                        .items_center()
                        .min_w_0()
                        .gap_1()
                        .children(icon.map(|bytes| Icon::default().data(&bytes).small()))
                        .child(div().min_w_0().truncate().child(label))
                        .when(entry.data.busy, |row| {
                            row.child(editor_ui::spinner::Spinner::new().small())
                        })
                        .child(Icon::new(IconName::ChevronDown).xsmall()),
                )
                .on_click(cx.listener(move |app, _, window, cx| {
                    app.open_presentation_popup(
                        vec![entry.clone()],
                        false,
                        anchor.get(),
                        window,
                        cx,
                    )
                }));
            bar = bar.child(
                div()
                    .relative()
                    .h_full()
                    .flex()
                    .items_center()
                    .child(button)
                    .child(
                        canvas(
                            move |bounds, _, _| measure.set(bounds.bottom_left()),
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .inset_0(),
                    ),
            );
        }
        if visible < entries.len() {
            let overflow = entries[visible..].to_vec();
            let anchor = Rc::new(Cell::new(point(px(0.), px(0.))));
            let measure = anchor.clone();
            bar = bar.child(
                div()
                    .relative()
                    .h_full()
                    .flex()
                    .items_center()
                    .child(
                        Button::new("workspace-entry-overflow")
                            .debug_selector(|| "workspace-entry-overflow".into())
                            .small()
                            .compact()
                            .ghost()
                            .label("…")
                            .accessibility_label(t!("workspace_presentation.more").to_string())
                            .tooltip(t!("workspace_presentation.more").to_string())
                            .on_click(cx.listener(move |app, _, window, cx| {
                                app.open_presentation_popup(
                                    overflow.clone(),
                                    true,
                                    anchor.get(),
                                    window,
                                    cx,
                                )
                            })),
                    )
                    .child(
                        canvas(
                            move |bounds, _, _| measure.set(bounds.bottom_left()),
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .inset_0(),
                    ),
            );
        }
        bar.children(self.workspace_presentation.popup.clone())
    }
    /// Open only the user-selected entry set; refreshing is an ordinary checked package command.
    fn open_presentation_popup(
        &mut self,
        entries: Vec<Entry>,
        overflow: bool,
        position: gpui_kit::Point<gpui_kit::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(old) = self.workspace_presentation.popup.take() {
            window.defer(cx, move |window, cx| {
                old.update(cx, |menu, cx| menu.dismiss(window, cx))
            });
        }
        self.workspace_presentation.serial = self.workspace_presentation.serial.wrapping_add(1);
        let serial = self.workspace_presentation.serial;
        for entry in &entries {
            if entry.data.on_open.is_some() {
                self.invoke_presentation_action(entry, None, cx);
            }
        }
        let (items, icons, routes) = menu_rows(&entries, overflow);
        self.workspace_presentation.popup_actions = routes;
        let owner = cx.entity().downgrade();
        let popup = cx.new(|cx| {
            ActionMenu::new(
                items,
                rust_i18n::locale().to_string(),
                icons,
                position,
                Some(t!("workspace_presentation.search").to_string()),
                move |action, window, cx| {
                    // Capture before deferral. A replacement publisher or a later overflow layout
                    // cannot change the owner, command or publication revision of this painted hit.
                    let target =
                        if let ActionMenuEvent::Select(id) | ActionMenuEvent::Open(id) = &action {
                            owner.upgrade().and_then(|app| {
                                app.read(cx)
                                    .workspace_presentation
                                    .popup_actions
                                    .get(id)
                                    .cloned()
                            })
                        } else {
                            None
                        };
                    let owner = owner.clone();
                    window.defer(cx, move |_, cx| {
                        let _ = owner.update(cx, |app, cx| {
                            if app.workspace_presentation.serial != serial {
                                return;
                            }
                            let opening = matches!(action, ActionMenuEvent::Open(_));
                            if let Some((entry, action)) = target {
                                app.invoke_presentation_action(&entry, Some(&action), cx);
                            }
                            if !opening {
                                app.workspace_presentation.popup = None;
                                app.workspace_presentation.popup_entries.clear();
                                app.workspace_presentation.popup_actions.clear();
                            }
                            cx.notify();
                        });
                    });
                },
                window,
                cx,
            )
        });
        self.workspace_presentation.popup = Some(popup);
        self.workspace_presentation.popup_entries = entries;
        self.workspace_presentation.popup_overflow = overflow;
        cx.notify();
    }
    /// Keep an open popup current after local refresh, preserving search/expansion without old callbacks.
    fn reconcile_presentation_popup(
        &mut self,
        entries: &[Entry],
        visible: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(popup) = self.workspace_presentation.popup.clone() else {
            return;
        };
        let previous = &self.workspace_presentation.popup_entries;
        let current = if self.workspace_presentation.popup_overflow {
            entries[visible..].to_vec()
        } else {
            entries
                .iter()
                .filter(|entry| {
                    previous.iter().any(|old| {
                        old.key == entry.key && old.resource.handle() == entry.resource.handle()
                    })
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        if current.is_empty() {
            self.workspace_presentation.serial = self.workspace_presentation.serial.wrapping_add(1);
            self.workspace_presentation.popup = None;
            self.workspace_presentation.popup_entries.clear();
            self.workspace_presentation.popup_actions.clear();
            window.defer(cx, move |window, cx| {
                popup.update(cx, |menu, cx| menu.dismiss(window, cx))
            });
        } else if current
            .iter()
            .map(|entry| (&entry.key, entry.resource.handle(), entry.revision))
            .collect::<Vec<_>>()
            != previous
                .iter()
                .map(|entry| (&entry.key, entry.resource.handle(), entry.revision))
                .collect::<Vec<_>>()
        {
            let (items, icons, routes) =
                menu_rows(&current, self.workspace_presentation.popup_overflow);
            popup.update(cx, |menu, cx| menu.replace(items, icons, cx));
            self.workspace_presentation.popup_entries = current;
            self.workspace_presentation.popup_actions = routes;
        }
    }
    /// Freeze typed arguments and authority before queueing; later publications cannot retarget this click.
    fn invoke_presentation_action(
        &mut self,
        entry: &Entry,
        action: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let result = (|| {
            let command = entry
                .resource
                .command(entry.revision, &entry.data.id, action)?;
            let authority =
                entry
                    .resource
                    .callback_authority(entry.revision, &entry.data.id, action)?;
            let guard = self
                .plugin_command_intents
                .capture(None, None, None, false)?
                .with_authority(authority);
            self.extensions
                .read(cx)
                .enqueue_owned_command(
                    &entry.resource.handle().instance,
                    &command.command,
                    command.arguments,
                    guard,
                )
                .map_err(|message| api::Failure::new(api::ErrorCode::InvalidState, message))
        })();
        if let Err(error) = result {
            self.status = error.message;
            cx.notify();
        }
    }
}

/// Overflow uses one native submenu per entry while preserving a route to each original action ID.
fn menu_rows(
    entries: &[Entry],
    overflow: bool,
) -> (
    Vec<ui::ActionMenuItem>,
    Arc<BTreeMap<String, Arc<[u8]>>>,
    BTreeMap<String, (Entry, String)>,
) {
    let mut icons = BTreeMap::new();
    let mut items = Vec::new();
    let mut routes = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        let snapshot = entry.resource.snapshot();
        let prefix = format!("p{index}/");
        let mut menu = entry.data.menu.clone();
        // Local artwork keys are namespaced as well; unrelated publishers may use the same path.
        for (path, bytes) in snapshot.icons.iter() {
            icons.insert(format!("{prefix}{path}"), bytes.clone());
        }
        fn rewrite(
            items: &mut [ui::ActionMenuItem],
            prefix: &str,
            entry: &Entry,
            routes: &mut BTreeMap<String, (Entry, String)>,
        ) {
            for item in items {
                let original = item.id.clone();
                // Length framing is injective even when a legal publisher ID contains separators.
                item.id = format!("action-{}:{}{}", entry.key.len(), entry.key, original);
                routes.insert(item.id.clone(), (entry.clone(), original));
                if let Some(icon) = &mut item.icon {
                    icon.light = format!("{prefix}{}", icon.light);
                    icon.dark = format!("{prefix}{}", icon.dark);
                }
                match &mut item.kind {
                    ui::ActionMenuKind::Group { items, .. }
                    | ui::ActionMenuKind::Submenu { items, .. } => {
                        rewrite(items, prefix, entry, routes)
                    }
                    _ => {}
                }
            }
        }
        rewrite(&mut menu, &prefix, entry, &mut routes);
        if overflow {
            items.push(ui::ActionMenuItem {
                id: format!("entry-{}", entry.key),
                label: entry.data.label.clone(),
                detail: None,
                icon: Some(ui::ToolIcon::new(
                    format!("{prefix}{}", entry.data.icon.light),
                    format!("{prefix}{}", entry.data.icon.dark),
                )),
                checked: false,
                disabled: entry.data.disabled,
                separator_before: false,
                kind: ui::ActionMenuKind::Submenu {
                    items: menu,
                    on_open: None,
                },
            });
        } else {
            items = menu;
        }
    }
    (items, Arc::new(icons), routes)
}
