//! Local dock title presentation attached to Base panel identity and lifecycle.
pub use gpui_base::dock::{
    DockAreaState, DockState, DragPanel, DropIndicator, Panel as BasePanel,
    PanelView as BasePanelView, TabGroupContext, TabGroupRenderer,
};
pub use gpui_base::dock::{PanelId, PanelState, TabGroup};
use gpui_kit::{AnyElement, App, Context, Entity, IntoElement, Window};
use std::sync::Arc;

/// Local title rendering is the only presentation extension on the Base panel contract.
pub trait Panel: BasePanel {
    /// Optional plain label for compact drag cards; controls and content stay in their native slot.
    fn drag_title(&self, _: &App) -> Option<gpui_kit::SharedString> {
        None
    }
    /// Render this panel's title and local toolbar using its original entity context.
    fn title(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement;
    /// Panels carrying their own header may omit the surrounding dock title bar.
    fn title_bar(&self, _: &App) -> bool {
        true
    }
}

/// Type-erased local title callbacks retain the same Base panel authority.
#[derive(Clone)]
pub(crate) struct PanelHandle {
    /// Original entity delegates every behavioral call to Base's public entity implementation.
    base: Arc<dyn BasePanelView>,
    /// Title renderer updates the original entity instead of copying its state.
    title: Arc<dyn Fn(&mut Window, &mut App) -> AnyElement + Send + Sync>,
    /// Header visibility is resolved from the live panel.
    title_bar: Arc<dyn Fn(&App) -> bool + Send + Sync>,
    /// Plain display label is independent of the rich native title and toolbar.
    drag_title: Arc<dyn Fn(&App) -> Option<gpui_kit::SharedString> + Send + Sync>,
}
impl PanelHandle {
    /// Return a contribution's plain drag label when its static Base name is insufficient.
    pub(crate) fn drag_title(&self, cx: &App) -> Option<gpui_kit::SharedString> {
        (self.drag_title)(cx)
    }
    /// Recover the local presentation carried through Base's panel view object.
    pub(crate) fn of(panel: &Arc<dyn BasePanelView>) -> Option<&Self> {
        if let Some(presented) = panel
            .as_any()
            .downcast_ref::<super::presentation::PresentedPanel>()
        {
            return Self::of(&presented.inner);
        }
        panel.as_any().downcast_ref()
    }
    /// Draw the live panel title.
    pub(crate) fn title(&self, window: &mut Window, cx: &mut App) -> AnyElement {
        (self.title)(window, cx)
    }
    /// Whether this panel requests local dock chrome.
    pub(crate) fn title_bar(&self, cx: &App) -> bool {
        (self.title_bar)(cx)
    }
    /// Raw visibility intent, independent of temporary regional suppression.
    pub(crate) fn visible(&self, cx: &App) -> bool {
        self.base.visible(cx)
    }
}
/// Bind local title callbacks to `panel`; Base continues to own layout, focus and lifecycle.
pub fn panel_handle<P: Panel>(panel: Entity<P>) -> Arc<dyn BasePanelView> {
    let title = panel.clone();
    let header = panel.clone();
    let label = panel.clone();
    Arc::new(PanelHandle {
        base: Arc::new(panel),
        title: Arc::new(move |window, cx| {
            title.update(cx, |panel, cx| panel.title(window, cx).into_any_element())
        }),
        title_bar: Arc::new(move |cx| header.read(cx).title_bar(cx)),
        drag_title: Arc::new(move |cx| label.read(cx).drag_title(cx)),
    })
}
impl BasePanelView for PanelHandle {
    fn panel_name(&self, cx: &App) -> &'static str {
        self.base.panel_name(cx)
    }
    fn panel_id(&self, cx: &App) -> PanelId {
        self.base.panel_id(cx)
    }
    fn visible(&self, cx: &App) -> bool {
        self.base.visible(cx)
    }
    fn closable(&self, cx: &App) -> bool {
        self.base.closable(cx)
    }
    fn zoomable(&self, cx: &App) -> bool {
        self.base.zoomable(cx)
    }
    fn set_active(&self, active: bool, window: &mut Window, cx: &mut App) {
        self.base.set_active(active, window, cx)
    }
    fn set_zoomed(&self, zoomed: bool, window: &mut Window, cx: &mut App) {
        self.base.set_zoomed(zoomed, window, cx)
    }
    fn on_added_to(
        &self,
        group: gpui_kit::WeakEntity<TabGroup>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.base.on_added_to(group, window, cx)
    }
    fn on_removed(&self, window: &mut Window, cx: &mut App) {
        self.base.on_removed(window, cx)
    }
    fn view(&self) -> gpui_kit::AnyView {
        self.base.view()
    }
    fn focus_handle(&self, cx: &App) -> gpui_kit::FocusHandle {
        self.base.focus_handle(cx)
    }
    fn dump(&self, cx: &App) -> PanelState {
        self.base.dump(cx)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
