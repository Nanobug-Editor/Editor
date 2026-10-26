//! Native transfer routes carry trusted layout/OS evidence directly to the owning runtime actor.
use plugin_runtime::{
    HostTransferOffer,
    plugin_protocol::transfer::{Address, Phase, Position},
};

/// This native ingress cannot be fabricated through ordinary guest UI notifications.
pub(crate) enum Ingress {
    /// The actor validates a native gesture before it returns any bytes or persistent source authority.
    StartDrag {
        address: Address,
        expected: plugin_runtime::plugin_protocol::transfer::DragSource,
        alive: std::sync::Arc<std::sync::atomic::AtomicBool>,
        reply: futures::channel::oneshot::Sender<
            Result<
                std::sync::Arc<plugin_runtime::TransferOfferResource>,
                plugin_runtime::plugin_protocol::api::Failure,
            >,
        >,
    },
    /// Exactly one OS outcome is correlated to the original actor-issued source handle.
    FinishDrag {
        handle: plugin_runtime::plugin_protocol::api::ResourceHandle,
        outcome: plugin_runtime::plugin_protocol::transfer::DragOutcome,
    },
    /// One OS gesture enters a specific actually painted receiver.
    Enter {
        serial: u64,
        address: Address,
        expected: plugin_runtime::plugin_protocol::transfer::DropTarget,
        source: HostTransferOffer,
        position: Position,
    },
    /// Motion/terminal events keep the original serial and its actor-issued opaque offer.
    Update {
        serial: u64,
        address: Address,
        phase: Phase,
        position: Position,
    },
}
/// Native callbacks synchronously learn whether the original owning actor accepted queue delivery.
pub(crate) type Sink = std::rc::Rc<dyn Fn(Ingress) -> bool>;

#[cfg(windows)]
mod source;
#[cfg(windows)]
mod windows;

/// Per-view native scope; the window's shared OLE registration outlives individual plugin panels.
#[derive(Default)]
pub(super) struct Transfers {
    #[cfg(windows)]
    scope: Option<std::rc::Rc<windows::Scope>>,
    #[cfg(windows)]
    source: source::Source,
}

impl super::PluginView {
    /// A Canvas source preserves the exact target, displayed UI revision and immutable representations.
    pub(super) fn canvas_source_pointer_down(
        &self,
        node: &str,
        anchor: plugin_runtime::plugin_protocol::ui::CanvasAnchor,
        expected: plugin_runtime::plugin_protocol::transfer::DragSource,
        revision: u64,
        position: gpui_kit::Point<gpui_kit::Pixels>,
    ) {
        #[cfg(windows)]
        if self.scene_current.get()
            && let Some(scope) = &self.transfers.scope
        {
            let mut address = Address::node(node, revision);
            address.source = self.document.source.clone();
            address.file = self.document.file.clone();
            address.canvas = Some(anchor);
            self.transfers
                .source
                .press(scope, address, expected, position);
        }
    }
    /// A source declaration never starts a drag without this actual hit node's native left press.
    pub(super) fn source_pointer_down(
        &self,
        node: &str,
        revision: u64,
        event: &gpui_kit::MouseDownEvent,
    ) {
        #[cfg(windows)]
        if event.button == gpui_kit::MouseButton::Left
            && self.scene_current.get()
            && revision == self.document.revision
            && self
                .document
                .active_node(node)
                .is_some_and(|node| node.drag_source.is_some())
        {
            if let Some(scope) = &self.transfers.scope {
                let mut address = Address::node(node, revision);
                address.source = self.document.source.clone();
                address.file = self.document.file.clone();
                let expected = self
                    .document
                    .active_node(node)
                    .unwrap()
                    .drag_source
                    .clone()
                    .unwrap();
                self.transfers
                    .source
                    .press(scope, address, expected, event.position);
            }
        }
        #[cfg(not(windows))]
        let _ = (node, revision, event);
    }
    /// A retained native view keeps its original epoch-bound port across ordinary redraws.
    pub(crate) fn native_transfers_attached(&self) -> bool {
        #[cfg(windows)]
        {
            self.transfers.scope.is_some()
        }
        #[cfg(not(windows))]
        {
            false
        }
    }
    /// Attach the owning actor's native port. Declaration and rendering alone never start a transfer.
    pub(crate) fn set_native_transfers(
        &mut self,
        sink: Sink,
        window: &mut gpui_kit::Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> anyhow::Result<()> {
        #[cfg(windows)]
        {
            self.transfers.scope = Some(windows::Scope::attach(sink, window, cx)?);
            self.reconcile_native_transfers();
            cx.notify();
        }
        #[cfg(not(windows))]
        let _ = (sink, window, cx);
        Ok(())
    }
    /// Target/source/modal/ancestor changes revoke OS reads before another native frame is needed.
    pub(super) fn reconcile_native_transfers(&self) {
        #[cfg(windows)]
        if let Some(scope) = &self.transfers.scope {
            scope.reconcile(&self.document, self.scene_current.get(), |id| {
                self.overlays.scopes.get(id).map(|scope| scope.generation)
            });
        }
    }
}
impl Transfers {
    /// Release/visibility paths revoke native readers independently of old GPUI paint closures.
    pub(super) fn suspend(&self, document: &plugin_runtime::plugin_protocol::ui::Document) {
        #[cfg(windows)]
        self.source.clear();
        #[cfg(windows)]
        if let Some(scope) = &self.scope {
            scope.reconcile(document, false, |_| None);
        }
        #[cfg(not(windows))]
        let _ = document;
    }
    /// Passive window hooks retain a press outside its original box; they create no timer or redraw.
    pub(super) fn listen(&self, window: &mut gpui_kit::Window) {
        #[cfg(windows)]
        self.source.listen(window);
        #[cfg(not(windows))]
        let _ = window;
    }
    /// A fresh paint replaces hit rectangles; it does not replace semantic target authority.
    pub(super) fn begin_frame(&self) {
        #[cfg(windows)]
        if let Some(scope) = &self.scope {
            scope.begin_frame();
        }
    }
    /// Observe actual clipped native geometry without adding another mouse/focus/Tab target.
    pub(super) fn probe(
        &self,
        node: &plugin_runtime::plugin_protocol::ui::Node,
        scale: f32,
        canvas: Option<&gpui_kit::Entity<super::canvas::CanvasView>>,
    ) -> Option<gpui_kit::AnyElement> {
        #[cfg(windows)]
        {
            self.scope.as_ref()?.probe(node, scale, canvas.cloned())
        }
        #[cfg(not(windows))]
        {
            let _ = (node, scale, canvas);
            None
        }
    }
}
