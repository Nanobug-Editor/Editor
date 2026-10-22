//! Per-window local modal and context-menu hosting through Base Root plugins.
use super::{context_menu::PopupMenu, dialog::Dialog};
use gpui_base::{Root, RootPlugin};
use gpui_kit::{
    AnyElement, App, Context, Entity, FocusHandle, IntoElement, ParentElement, Render, Window, div,
};
use std::rc::Rc;

/// A retained dialog rebuilds appearance from the current theme and preserves its native focus.
struct Modal {
    /// Stable identity prevents delayed dismissal from closing a newer dialog.
    id: u64,
    /// Original focused control, restored only while this modal still owns focus.
    previous: Option<FocusHandle>,
    /// One Base modal focus owner survives rerenders.
    focus: FocusHandle,
    /// Product content and callbacks, rebuilt on every frame.
    build: Rc<dyn Fn(Dialog, &mut Window, &mut App) -> Dialog>,
}
/// Window-local overlays hold presentation only; plugin/session owners retain business state.
pub struct Overlays {
    /// Ordered modal stack, with Base providing focus trapping and keyboard dispatch.
    modals: Vec<Modal>,
    /// Local context menu for native editor source actions.
    popup: Option<Entity<PopupMenu>>,
    /// Each opened modal receives a distinct dismissal token.
    next_id: u64,
}
/// Register before constructing Base Roots; repeated registration does not duplicate mounted state.
pub fn init(cx: &mut App) {
    Root::register_plugin(cx, |_, _| Overlays {
        modals: Vec::new(),
        popup: None,
        next_id: 0,
    });
}
impl RootPlugin for Overlays {}
impl Render for Overlays {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.modals.len();
        let mut elements: Vec<AnyElement> = Vec::new();
        for (index, modal) in self.modals.iter().enumerate() {
            let id = modal.id;
            let owner = cx.entity().downgrade();
            let dialog = Dialog::new(cx).focus_handle(modal.focus.clone());
            let dialog = (modal.build)(dialog, window, cx).request_close(move |window, cx| {
                let _ = owner.update(cx, |overlay, cx| overlay.close(id, window, cx));
            });
            elements.push(dialog.layer(index, index + 1 == count).into_any_element());
        }
        div().children(self.popup.clone()).children(elements)
    }
}
impl Overlays {
    /// Retire one exact modal and restore focus only if its old dispatch scope remains active.
    fn close(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.modals.iter().position(|modal| modal.id == id) else {
            return;
        };
        let modal = self.modals.remove(index);
        if modal.focus.contains_focused(window, cx) {
            if let Some(previous) = modal.previous {
                previous.focus(window, cx);
            }
        }
        cx.notify();
    }
}
/// Local modal operations read the Base Root belonging to this exact native window.
pub trait WindowExt {
    /// Open product content using Base's dialog behavior and focus trap.
    fn open_dialog(
        &mut self,
        cx: &mut App,
        build: impl Fn(Dialog, &mut Window, &mut App) -> Dialog + 'static,
    );
    /// Whether this window currently has a local modal.
    fn has_active_dialog(&mut self, cx: &mut App) -> bool;
    /// Close the topmost dialog through the retained window overlay owner.
    fn close_dialog(&mut self, cx: &mut App);
}
impl WindowExt for Window {
    fn open_dialog(
        &mut self,
        cx: &mut App,
        build: impl Fn(Dialog, &mut Window, &mut App) -> Dialog + 'static,
    ) {
        let overlay = Root::read(self, cx)
            .plugin::<Overlays>()
            .expect("local overlays are initialized before Root");
        overlay.update(cx, |overlay, cx| {
            let previous = self.focused(cx);
            let focus = cx.focus_handle();
            focus.focus(self, cx);
            overlay.popup = None;
            overlay.next_id += 1;
            overlay.modals.push(Modal {
                id: overlay.next_id,
                previous,
                focus,
                build: Rc::new(build),
            });
            cx.notify();
        });
    }
    fn has_active_dialog(&mut self, cx: &mut App) -> bool {
        Root::read(self, cx)
            .plugin::<Overlays>()
            .is_some_and(|overlay| !overlay.read(cx).modals.is_empty())
    }
    fn close_dialog(&mut self, cx: &mut App) {
        if let Some(overlay) = Root::read(self, cx).plugin::<Overlays>() {
            overlay.update(cx, |overlay, cx| {
                if let Some(modal) = overlay.modals.last() {
                    overlay.close(modal.id, self, cx);
                }
            });
        }
    }
}
/// Mount one local action menu and clear it on its own dismissal, without changing another window.
pub fn show_context_menu(popup: Entity<PopupMenu>, window: &mut Window, cx: &mut App) {
    if let Some(overlay) = Root::read(window, cx).plugin::<Overlays>() {
        let id = popup.entity_id();
        overlay.update(cx, |overlay, cx| {
            cx.subscribe(&popup, move |overlay, _, _: &gpui_kit::DismissEvent, cx| {
                if overlay
                    .popup
                    .as_ref()
                    .is_some_and(|popup| popup.entity_id() == id)
                {
                    overlay.popup = None;
                    cx.notify();
                }
            })
            .detach();
            overlay.popup = Some(popup);
            cx.notify();
        });
    }
}
