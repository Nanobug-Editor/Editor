//! Tree context menus preserve source selection and capture exact tree/scene action identities.
use super::*;
use editor_ui::menu::{ActionMenu, ActionMenuEvent};

/// Flat legacy menus and negotiated cascading menus share one native dismissal owner.
#[derive(Clone)]
pub(super) enum TreeMenu {
    Flat(Entity<PopupMenu>),
    Actions(Entity<ActionMenu>),
}
impl TreeMenu {
    /// Retire through the control's focus lease, including lifecycle-driven scene withdrawal.
    pub(super) fn dismiss(self, window: &mut Window, cx: &mut App) {
        match self {
            Self::Flat(menu) => menu.update(cx, |menu, cx| menu.dismiss(window, cx)),
            Self::Actions(menu) => menu.update(cx, |menu, cx| menu.dismiss(window, cx)),
        }
    }
    /// Mount the existing local control without recreating its focus or scroll state.
    pub(super) fn element(&self) -> gpui_kit::AnyElement {
        match self {
            Self::Flat(menu) => menu.clone().into_any_element(),
            Self::Actions(menu) => menu.clone().into_any_element(),
        }
    }
}

impl TreeView {
    /// Captured menus carry tree and scene epochs; menu highlighting does not change selection.
    pub(super) fn open_menu(
        &mut self,
        id: &str,
        position: gpui_kit::Point<gpui_kit::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(node) = self.snapshot.nodes.get(id).cloned().filter(|node| {
            !node.disabled && (!node.menu.is_empty() || !node.action_menu.is_empty())
        }) else {
            return;
        };
        let cascading = !node.action_menu.is_empty();
        let owner = cx.entity().downgrade();
        let id = id.to_string();
        self.popup_node = Some(id.clone());
        let epoch = self.snapshot.epoch;
        let revision = self.revision;
        let gesture = self.action_gesture;
        self.popup_serial = self.popup_serial.wrapping_add(1);
        let serial = self.popup_serial;
        let sink = move |action: ActionMenuEvent, window: &mut Window, cx: &mut App| {
            let owner = owner.clone();
            let id = id.clone();
            window.defer(cx, move |_, cx| {
                let _ = owner.update(cx, |this, cx| {
                    if !this.frame_current(gesture)
                        || this.popup_node.as_ref() != Some(&id)
                        || this.popup_serial != serial
                        || this.popup_epoch != Some((epoch, revision))
                        || this.revision != revision
                        || this.snapshot.epoch != epoch
                    {
                        return;
                    }
                    let opening = matches!(action, ActionMenuEvent::Open(_));
                    if let ActionMenuEvent::Select(item) | ActionMenuEvent::Open(item) = action {
                        let action = if cascading {
                            Some(api::NodeAction::MenuItem { id: item })
                        } else {
                            item.strip_prefix("tree-menu-")
                                .and_then(|index| index.parse::<u32>().ok())
                                .map(|index| api::NodeAction::Menu { index })
                        };
                        if let Some(action) = action {
                            this.invoke(&id, action, cx);
                        }
                    }
                    if !opening {
                        this.popup = None;
                        this.popup_node = None;
                        this.popup_epoch = None;
                    }
                    cx.notify();
                });
            });
        };
        let menu = if cascading {
            TreeMenu::Actions(cx.new(|cx| {
                ActionMenu::new(
                    node.action_menu,
                    self.environment.locale.clone(),
                    Arc::new(self.snapshot.icons.clone()),
                    position,
                    None,
                    sink,
                    window,
                    cx,
                )
            }))
        } else {
            let items = node
                .menu
                .iter()
                .enumerate()
                .map(|(index, command)| ui::MenuItem {
                    id: format!("tree-menu-{index}"),
                    label: command.label.for_locale(&self.environment.locale).into(),
                    disabled: command.disabled,
                    separator_before: command.separator_before,
                })
                .collect();
            TreeMenu::Flat(cx.new(|cx| {
                PopupMenu::new(
                    items,
                    MenuStyle::current(cx),
                    position,
                    move |action, window, cx| {
                        sink(
                            match action {
                                Action::Select(id) => ActionMenuEvent::Select(id),
                                _ => ActionMenuEvent::Dismiss,
                            },
                            window,
                            cx,
                        )
                    },
                    window,
                    cx,
                )
            }))
        };
        self.popup = Some(menu);
        self.popup_epoch = Some((epoch, revision));
        cx.notify();
    }
}
