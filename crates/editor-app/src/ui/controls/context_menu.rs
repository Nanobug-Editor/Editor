//! Local context menus share Base buttons, product rows, native focus and action dispatch.
use super::menu::MenuStyle;
use gpui_base::{ElementExt as _, TestSupportExt as _};
use gpui_kit::{
    AnyElement, App, AppContext as _, Bounds, ClickEvent, Context, DismissEvent, Entity,
    EventEmitter, FocusHandle, Focusable, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement, Pixels, Point, Render, Role, SharedString, StatefulInteractiveElement as _,
    Styled, WeakEntity, Window, anchored, div, point, prelude::FluentBuilder as _, px,
};
use std::{cell::Cell, rc::Rc};

/// Activation callbacks retain the caller's target/version checks instead of resolving a new target.
type Click = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
/// Custom row content is repainted against the live theme and command incarnation.
type Row = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// One local menu row; Base supplies button accessibility, disabled behavior and activation.
pub(crate) struct PopupMenuItem {
    /// Visible and accessible label for an ordinary row.
    label: SharedString,
    /// Disabled rows remain visible and are excluded from keyboard selection.
    disabled: bool,
    /// Optional domain-rendered row content, using the same local row contract.
    content: Option<Row>,
    /// Caller-owned operation target and admission logic.
    click: Option<Click>,
}
impl PopupMenuItem {
    /// Build an ordinary labeled context-menu action.
    pub(crate) fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            disabled: false,
            content: None,
            click: None,
        }
    }
    /// Render a live local row, such as a plugin action whose incarnation may have expired.
    pub(crate) fn element<E: IntoElement>(
        build: impl Fn(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        Self {
            content: Some(Rc::new(move |window, cx| {
                build(window, cx).into_any_element()
            })),
            ..Self::new("")
        }
    }
    /// Exclude a row from pointer activation and keyboard navigation.
    pub(crate) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    /// Install the caller's fresh operation admission callback.
    pub(crate) fn on_click(
        mut self,
        click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.click = Some(Rc::new(click));
        self
    }
}
/// Rows and submenu geometry remain presentation data; no file/document operation is performed here.
enum Entry {
    /// Pointer-inert visual grouping boundary.
    Separator,
    /// One selectable local action.
    Item(PopupMenuItem),
    /// Nested local menu, anchored to its actual painted trigger row.
    Submenu {
        label: SharedString,
        menu: Entity<PopupMenu>,
        bounds: Rc<Cell<Bounds<Pixels>>>,
    },
}

/// Local menu lifetime, navigation and focus restoration, shared by explorer and editor actions.
pub(crate) struct PopupMenu {
    /// Retained local rows and child menus.
    entries: Vec<Entry>,
    /// Keyboard-selected row; None matches a menu newly opened by a pointer.
    selected: Option<usize>,
    /// Currently displayed child, identified by its stable row index.
    submenu: Option<usize>,
    /// Native focus owner for this menu level.
    focus: FocusHandle,
    /// Original command dispatch scope, restored before running the chosen action.
    previous: Option<FocusHandle>,
    /// Parent owner for hierarchical dismissal and Left-arrow navigation.
    parent: Option<WeakEntity<Self>>,
    /// Local desired width, clamped by the actual window at render time.
    width: Pixels,
    /// A closed popup never runs its callback twice.
    closed: bool,
    /// Stationary pointer hover cannot undo keyboard navigation during rerenders.
    pointer: Point<Pixels>,
    /// Root menus may be mounted directly in the per-window overlay host.
    position: Option<Point<Pixels>>,
    /// Last painted surface bounds let outside presses exclude a displayed child menu.
    bounds: Rc<Cell<Bounds<Pixels>>>,
}
impl PopupMenu {
    /// Build a retained local menu; callers explicitly focus it after binding dismissal observers.
    pub(crate) fn build(
        window: &mut Window,
        cx: &mut App,
        build: impl FnOnce(Self, &mut Window, &mut Context<Self>) -> Self,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let menu = Self {
                entries: Vec::new(),
                selected: None,
                submenu: None,
                focus: cx.focus_handle(),
                previous: window.focused(cx),
                parent: None,
                width: px(212.),
                closed: false,
                pointer: window.mouse_position(),
                position: None,
                bounds: Rc::new(Cell::new(Bounds::default())),
            };
            build(menu, window, cx)
        })
    }
    /// Select a desired surface width; viewport clamping is performed after real layout.
    pub(crate) fn min_w(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }
    /// Restore action dispatch to the caller's original focused document/control.
    pub(crate) fn action_context(mut self, focus: FocusHandle) -> Self {
        self.previous = Some(focus);
        self
    }
    /// Append an ordinary or custom local row.
    pub(crate) fn item(mut self, item: PopupMenuItem) -> Self {
        self.entries.push(Entry::Item(item));
        self
    }
    /// Append one visual group boundary.
    pub(crate) fn separator(mut self) -> Self {
        self.entries.push(Entry::Separator);
        self
    }
    /// Build a retained nested menu, sharing dismissal and the original command scope.
    pub(crate) fn submenu(
        mut self,
        label: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
        build: impl FnOnce(Self, &mut Window, &mut Context<Self>) -> Self,
    ) -> Self {
        let parent = cx.weak_entity();
        let menu = Self::build(window, cx, |menu, window, cx| {
            let mut menu = build(menu, window, cx);
            menu.parent = Some(parent);
            menu.previous = None;
            menu
        });
        self.entries.push(Entry::Submenu {
            label: label.into(),
            menu,
            bounds: Rc::new(Cell::new(Bounds::default())),
        });
        self
    }
    /// Ignore separators and disabled actions during keyboard traversal.
    fn enabled(&self, index: usize) -> bool {
        match &self.entries[index] {
            Entry::Separator => false,
            Entry::Item(item) => !item.disabled,
            Entry::Submenu { .. } => true,
        }
    }
    /// A submenu paints beyond its parent's viewport but belongs to the same dismissal boundary.
    fn contains(&self, point: Point<Pixels>, cx: &App) -> bool {
        self.bounds.get().contains(&point)
            || self
                .submenu
                .is_some_and(|index| match &self.entries[index] {
                    Entry::Submenu { menu, .. } => menu.read(cx).contains(point, cx),
                    _ => false,
                })
    }
    /// Restore the root action scope before callbacks can open and focus another surface.
    fn restore(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(parent) = &self.parent {
            let _ = parent.update(cx, |parent, cx| parent.restore(window, cx));
        } else if let Some(previous) = self.previous.take() {
            previous.focus(window, cx);
        }
    }
    /// Emit dismissal through the complete hierarchy without stealing callback-owned focus.
    fn dismiss_chain(&mut self, cx: &mut Context<Self>) {
        self.closed = true;
        cx.emit(DismissEvent);
        cx.notify();
        if let Some(parent) = &self.parent {
            let _ = parent.update(cx, |parent, cx| parent.dismiss_chain(cx));
        }
    }
    /// Cancel the hierarchy through its normal focus restoration gate.
    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.restore(window, cx);
        self.dismiss_chain(cx);
    }
    /// Open a selected submenu, keeping the parent row visually selected.
    fn open_child(
        &mut self,
        index: usize,
        keyboard: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Entry::Submenu { menu, .. } = &self.entries[index] {
            self.selected = Some(index);
            self.submenu = Some(index);
            menu.update(cx, |menu, cx| {
                menu.closed = false;
                if keyboard {
                    menu.selected = menu
                        .entries
                        .iter()
                        .enumerate()
                        .position(|(index, _)| menu.enabled(index));
                    menu.focus.focus(window, cx);
                }
                cx.notify();
            });
            cx.notify();
        }
    }
    /// Run one admitted row at most once; business callbacks keep their exact captured target.
    fn activate(
        &mut self,
        index: usize,
        event: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.closed || !self.enabled(index) {
            return;
        }
        if matches!(self.entries[index], Entry::Submenu { .. }) {
            self.open_child(index, true, window, cx);
            return;
        }
        let callback = match &self.entries[index] {
            Entry::Item(item) => item.click.clone(),
            _ => None,
        };
        self.restore(window, cx);
        if let Some(callback) = callback {
            callback(event, window, cx);
        }
        self.dismiss_chain(cx);
    }
    /// Arrow traversal, hierarchical navigation, activation and Escape share a single local route.
    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let enabled = (0..self.entries.len())
            .filter(|index| self.enabled(*index))
            .collect::<Vec<_>>();
        match event.keystroke.key.as_str() {
            "down" | "up" => {
                let selected = self
                    .selected
                    .and_then(|selected| enabled.iter().position(|index| *index == selected));
                if !enabled.is_empty() {
                    let next = if event.keystroke.key == "down" {
                        selected.map_or(0, |selected| (selected + 1) % enabled.len())
                    } else {
                        selected.map_or(enabled.len() - 1, |selected| {
                            (selected + enabled.len() - 1) % enabled.len()
                        })
                    };
                    self.selected = Some(enabled[next]);
                    self.submenu = None;
                }
            }
            "right" => {
                if let Some(index) = self.selected {
                    self.open_child(index, true, window, cx);
                }
            }
            "left" => {
                if let Some(parent) = &self.parent {
                    let _ = parent.update(cx, |parent, cx| {
                        parent.submenu = None;
                        parent.focus.focus(window, cx);
                        cx.notify();
                    });
                }
            }
            "enter" | "space" => {
                if let Some(index) = self.selected {
                    self.activate(index, &ClickEvent::default(), window, cx);
                }
            }
            "escape" => self.cancel(window, cx),
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }
}
impl Focusable for PopupMenu {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl EventEmitter<DismissEvent> for PopupMenu {}
impl Render for PopupMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.closed {
            return div().into_any_element();
        }
        let style = MenuStyle::current(cx);
        let mut rows = Vec::new();
        for (index, entry) in self.entries.iter().enumerate() {
            let selected = self.selected == Some(index);
            let (label, disabled, content) = match entry {
                Entry::Separator => {
                    rows.push(div().h(px(1.)).my_1().bg(style.border).into_any_element());
                    continue;
                }
                Entry::Item(item) => (
                    item.label.clone(),
                    item.disabled,
                    item.content.as_ref().map(|build| build(window, cx)),
                ),
                Entry::Submenu { label, .. } => (label.clone(), false, None),
            };
            let has_submenu = matches!(entry, Entry::Submenu { .. });
            let bounds = match entry {
                Entry::Submenu { bounds, .. } => Some(bounds.clone()),
                _ => None,
            };
            rows.push(
                style
                    .row(index, label.clone(), selected)
                    .selected(selected)
                    .aria_selected(selected)
                    .disabled(disabled)
                    .styles(|styles| styles.disabled(|style| style.opacity(0.5)))
                    .on_prepaint(move |rect, _, _| {
                        if let Some(bounds) = &bounds {
                            bounds.set(rect);
                        }
                    })
                    .on_mouse_move(cx.listener(
                        move |this, event: &gpui_kit::MouseMoveEvent, window, cx| {
                            if this.pointer != event.position && this.enabled(index) {
                                this.pointer = event.position;
                                this.selected = Some(index);
                                this.submenu = None;
                                if matches!(this.entries[index], Entry::Submenu { .. }) {
                                    this.open_child(index, false, window, cx);
                                }
                                cx.notify();
                            }
                        },
                    ))
                    .on_click(cx.listener(move |this, event, window, cx| {
                        this.activate(index, event, window, cx)
                    }))
                    .when_some(content, |row, content| row.child(content))
                    .when(!label.is_empty(), |row| {
                        row.child(div().flex_1().child(label))
                    })
                    .when(has_submenu, |row| {
                        row.child(
                            super::Icon::new(gpui_kit::assets::IconName::ChevronRight).xsmall(),
                        )
                    })
                    .into_any_element(),
            );
        }
        let child = self.submenu.and_then(|index| match &self.entries[index] {
            Entry::Submenu { menu, bounds, .. } => {
                let bounds = bounds.get();
                let width = menu.read(cx).width;
                let x = if bounds.right() + width + px(8.) <= window.viewport_size().width {
                    bounds.right()
                } else {
                    bounds.left() - width
                };
                Some(
                    anchored()
                        .position(point(x, bounds.top()))
                        .snap_to_window_with_margin(px(8.))
                        .child(div().id("submenu").child(menu.clone()))
                        .into_any_element(),
                )
            }
            _ => None,
        });
        let bounds = self.bounds.clone();
        let surface = style
            .card(f32::from(
                self.width
                    .min((window.viewport_size().width - px(16.)).max(px(0.))),
            ))
            .id("local-context-menu")
            .test_support()
            .role(Role::Menu)
            .track_focus(&self.focus)
            .key_context("LocalContextMenu")
            .on_prepaint(move |rect, _, _| bounds.set(rect))
            .max_h((window.viewport_size().height - px(16.)).max(px(0.)))
            .overflow_y_scroll()
            .occlude()
            .on_key_down(cx.listener(Self::key))
            .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
            .on_mouse_down_out(
                cx.listener(|this, event: &gpui_kit::MouseDownEvent, window, cx| {
                    if this.parent.is_none() && !this.contains(event.position, cx) {
                        this.cancel(window, cx);
                        cx.stop_propagation();
                    }
                }),
            )
            .children(rows);
        // Deferred painting escapes the parent's scroll clipping without making another menu state.
        let surface = div()
            .child(surface)
            .children(child.map(|child| gpui_kit::deferred(child).with_priority(30)));
        if let Some(position) = self.position {
            anchored()
                .position(position)
                .snap_to_window_with_margin(px(8.))
                .child(surface)
                .into_any_element()
        } else {
            surface.into_any_element()
        }
    }
}

/// Typed action definitions use the same local context-menu renderer as explorer commands.
#[derive(Default)]
pub(crate) struct NativeMenu {
    /// Ordered local labels, disabled flags and typed application actions; None denotes a separator.
    items: Vec<Option<(SharedString, bool, Box<dyn gpui_kit::Action>)>>,
}
impl NativeMenu {
    /// Start a source action menu with no implicit host or plugin commands.
    pub(crate) fn new() -> Self {
        Self::default()
    }
    /// Append a typed enabled action for the caller's original focus scope.
    pub(crate) fn menu(
        self,
        label: impl Into<SharedString>,
        action: Box<dyn gpui_kit::Action>,
    ) -> Self {
        self.menu_with_disabled(label, false, action)
    }
    /// Append a typed action while preserving the caller's permission/availability result.
    pub(crate) fn menu_with_disabled(
        mut self,
        label: impl Into<SharedString>,
        disabled: bool,
        action: Box<dyn gpui_kit::Action>,
    ) -> Self {
        self.items.push(Some((label.into(), disabled, action)));
        self
    }
    /// Separate command groups visually without changing dispatch authority.
    pub(crate) fn separator(mut self) -> Self {
        self.items.push(None);
        self
    }
    /// Show a local Base-backed menu at the already projected physical source anchor.
    pub(crate) fn show(self, position: Point<Pixels>, window: &mut Window, cx: &mut App) {
        let popup = PopupMenu::build(window, cx, |mut menu, _, _| {
            menu.position = Some(position);
            for item in self.items {
                menu = if let Some((label, disabled, action)) = item {
                    menu.item(PopupMenuItem::new(label).disabled(disabled).on_click(
                        move |_, window, cx| window.dispatch_action(action.boxed_clone(), cx),
                    ))
                } else {
                    menu.separator()
                };
            }
            menu
        });
        popup.focus_handle(cx).focus(window, cx);
        super::overlay::show_context_menu(popup, window, cx);
    }
}
