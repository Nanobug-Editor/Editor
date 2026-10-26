//! Shared event validation keeps deferred native callbacks and worker queues on the same view version.
use super::*;
use crate::api::{ErrorCode, Failure};

impl Document {
    pub fn validate_event(&self, event: &UiEvent) -> Result<(), Failure> {
        let scroll_2d = match &event.action {
            Action::Canvas(input @ CanvasEvent::Scroll2D { .. })
            | Action::ViewportInput(ViewportInput {
                event: input @ CanvasEvent::Scroll2D { .. },
                ..
            }) => Some(input),
            _ => None,
        };
        if scroll_2d.is_some_and(|input| {
            matches!(input,CanvasEvent::Scroll2D{source,file,..}
            if *source!=self.source || *file!=self.file)
        }) {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Retained scroll source was replaced",
            ));
        }
        let target_input = match &event.action {
            Action::Canvas(CanvasEvent::Input { input })
            | Action::ViewportInput(ViewportInput {
                event: CanvasEvent::Input { input },
                ..
            }) => Some(input),
            _ => None,
        };
        // This narrow admission carries no action/source authority. Manager separately consumes
        // an active focus/preedit/pointer lease from the exact old native frame, even after a modal,
        // source or target replacement. Ordinary old Text/Key/A11y remains rejected below.
        if event.revision <= self.revision
            && target_input.is_some_and(|input| input.cleanup_kind().is_some())
        {
            return Ok(());
        }
        if target_input.is_some_and(|input| {
            input.source != self.source
                || input.file != self.file
                || input.projection.as_ref().is_some_and(|projection| {
                    projection
                        .validate(&Kind::Canvas(Canvas::default()))
                        .is_err()
                })
        }) {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Native target source or projection was replaced",
            ));
        }
        let layout = match &event.action {
            Action::Canvas(CanvasEvent::TextLayout {
                source,
                file,
                projection,
                ..
            })
            | Action::ViewportInput(ViewportInput {
                event:
                    CanvasEvent::TextLayout {
                        source,
                        file,
                        projection,
                        ..
                    },
                ..
            }) => Some((source, file, projection)),
            _ => None,
        };
        if layout.is_some_and(|(source, file, projection)| {
            *source != self.source
                || *file != self.file
                || !self
                    .active_node(&event.node)
                    .is_some_and(|node| node.viewport == *projection)
        }) {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Native text layout source was replaced",
            ));
        }
        let presentation = match &event.action {
            Action::Canvas(receipt @ CanvasEvent::ScenePresented { .. })
            | Action::ViewportInput(ViewportInput {
                event: receipt @ CanvasEvent::ScenePresented { .. },
                ..
            }) => Some(receipt),
            _ => None,
        };
        if let Some(CanvasEvent::ScenePresented {
            source,
            file,
            projection,
            ..
        }) = presentation
        {
            if *source != self.source
                || *file != self.file
                || !self
                    .active_node(&event.node)
                    .is_some_and(|node| node.viewport == *projection)
            {
                return Err(Failure::new(
                    ErrorCode::StaleRevision,
                    "Canvas presentation source was replaced",
                ));
            }
        }
        // Observations carry no input authority. Retain a receipt through unrelated UI painting
        // only while the exact scene and source remain current and no modal owns input.
        let current_presentation = event.revision < self.revision
            && self.dialog.is_none()
            && self.menu.is_none()
            && self.editor_action_source(&event.node).is_none()
            && presentation.is_some_and(|receipt| {
                self.active_node(&event.node).is_some_and(
                    |node| matches!(&node.kind,Kind::Canvas(canvas) if canvas.accepts(receipt)),
                )
            });
        // An explicit content epoch lets queued range movements survive painting only.
        // Future scenes, modal transitions and editor-source gestures never use this exception.
        let current_scroll = event.revision < self.revision
            && self.dialog.is_none()
            && self.menu.is_none()
            && self.editor_action_source(&event.node).is_none()
            && (matches!(&event.action, Action::Canvas(CanvasEvent::ScrollInput { epoch, .. })
                if *epoch != 0 && self.active_node(&event.node).is_some_and(|node|
                    node.viewport.is_none() && matches!(&node.kind, Kind::Canvas(canvas)
                        if canvas.scroll_epoch == Some(*epoch))))
                || scroll_2d.is_some_and(|input| {
                    self.active_node(&event.node).is_some_and(
                        |node| matches!(&node.kind,Kind::Canvas(canvas) if canvas.accepts(input)),
                    )
                }));
        let current_columns = event.revision < self.revision
            && self.dialog.is_none()
            && self.menu.is_none()
            && self.editor_action_source(&event.node).is_none()
            && matches!(&event.action, Action::Canvas(CanvasEvent::ColumnsResized { epoch, widths })
                if self.active_node(&event.node).is_some_and(|node| node.viewport.is_none()
                    && matches!(&node.kind, Kind::Canvas(canvas) if canvas.column_resize.as_ref().is_some_and(|columns| columns.accepts(*epoch, widths)))));
        // Target input identifies its actual displayed frame separately. Manager must still match
        // that frame's native receipt; this gate only retains the currently live source/target epoch.
        let current_target=event.revision<self.revision&&self.dialog.is_none()&&self.menu.is_none()
            &&self.editor_action_source(&event.node).is_none()
            &&target_input.is_some_and(|input|self.active_node(&event.node).is_some_and(|node|
                matches!(&node.kind,Kind::Canvas(canvas) if canvas.scene.as_ref().is_some_and(|scene|scene.accepts_input(input)))));
        if event.revision != self.revision
            && !current_scroll
            && !current_columns
            && !current_presentation
            && !current_target
        {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "UI document was replaced",
            ));
        }
        // Menus are modal event targets; they cannot route stale clicks into the underlying view.
        if let Some(menu) = &self.menu {
            if self.dialog.is_none()
                && event.node == menu.id
                && (matches!(event.action, Action::Dismiss)
                    || matches!(&event.action, Action::Select(id) if menu.items.iter().any(|item| item.id == *id && !item.disabled)))
            {
                return Ok(());
            }
            if self.dialog.is_none() {
                return Err(Failure::new(ErrorCode::InvalidState, "Popup owns input"));
            }
        }
        if matches!(event.action, Action::Dismiss)
            && self
                .dialog
                .as_ref()
                .is_some_and(|dialog| dialog.id == event.node)
        {
            return Ok(());
        }
        let node = self.active_node(&event.node).ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidHandle,
                "UI node is absent, inactive or disabled",
            )
        })?;
        let valid = match (&node.kind, &event.action) {
            (Kind::Tree { handle }, Action::Tree(event)) => handle == event.handle(),
            (
                Kind::Canvas(_) | Kind::Image { .. } | Kind::FileImage { .. },
                Action::ViewportInput(input),
            ) => {
                node.viewport.is_some()
                    && input.content.valid()
                    && match &node.kind {
                        Kind::Canvas(canvas) => {
                            (node.viewport.as_ref().unwrap().content == Some(input.content)
                                // Absolute retained offsets carry no viewport-local point/delta authority.
                                || current_scroll && matches!(input.event,CanvasEvent::Scroll2D{..}))
                                && canvas.accepts(&input.event)
                        }
                        _ => Canvas::default().accepts(&input.event),
                    }
            }
            (Kind::Scroll { content }, Action::Viewport(position)) => {
                self.dialog.is_none()
                    && self.menu.is_none()
                    && self.editor_viewport.as_deref() == Some(&node.id)
                    && position.validate().is_ok()
                    && content
                        .viewport_block(&position.block)
                        .is_some_and(|block| block.source_range == Some(position.source_range))
            }
            (Kind::Image { .. } | Kind::Text { .. }, Action::Link { uri }) => {
                self.link_events && node.links.iter().any(|link| link.uri == *uri)
            }
            (Kind::RichText { .. }, Action::Link { uri }) => {
                self.link_events
                    && !uri.is_empty()
                    && uri.len() <= 4096
                    && !uri.chars().any(char::is_control)
            }
            (Kind::SideTabs(tabs), action) => tabs.accepts(action),
            (Kind::Button { .. }, Action::Click) | (Kind::Checkbox { .. }, Action::Toggle(_)) => {
                true
            }
            (Kind::Input(_) | Kind::Textarea(_), Action::Change(value) | Action::Submit(value)) => {
                value.len() <= 65536
            }
            (Kind::Choice { options, .. }, Action::Select(id)) => options
                .iter()
                .any(|option| option.id == *id && !option.disabled),
            (Kind::Tabs { tabs, .. }, Action::Select(id)) => tabs.iter().any(|tab| tab.id == *id),
            (Kind::Canvas(canvas), Action::Canvas(event)) => {
                (node.viewport.is_none()
                    || matches!(
                        event,
                        CanvasEvent::TextMeasured { .. } | CanvasEvent::Input { .. }
                    ))
                    && canvas.accepts(event)
            }
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Event does not match this UI node or exceeds its limits",
            ))
        }
    }
}

impl Canvas {
    /// Host-created measurements remain bounded too, avoiding NaN/Infinity entering a guest layout loop.
    fn accepts(&self, event: &CanvasEvent) -> bool {
        let finite = |value: f32| value.is_finite() && value.abs() <= 1_000_000.;
        match event {
            CanvasEvent::Input { input } => self
                .scene
                .as_ref()
                .is_some_and(|scene| scene.accepts_input(input)),
            CanvasEvent::TextLayout {
                identity,
                scene_revision,
                results,
                ..
            } => self.accepts_text_layout(*identity, *scene_revision, results),
            CanvasEvent::ScenePresented {
                identity,
                scene_revision,
                error,
                ..
            } => self.scene.as_ref().is_some_and(|scene| {
                scene.identity == *identity
                    && scene.revision == *scene_revision
                    && error.as_ref().is_none_or(|error| error.len() <= 1024)
            }),
            CanvasEvent::ColumnsResized { epoch, widths } => self
                .column_resize
                .as_ref()
                .is_some_and(|columns| columns.accepts(*epoch, widths)),
            CanvasEvent::TextMeasured { widths } => {
                !self.measure_text.is_empty()
                    && widths.len() == self.measure_text.len()
                    && widths
                        .iter()
                        .zip(&self.measure_text)
                        .all(|(result, request)| {
                            result.id == request.id && finite(result.width) && result.width >= 0.
                        })
            }
            CanvasEvent::Scroll2D {
                identity,
                epoch,
                offset,
                ..
            } => {
                self.scene
                    .as_ref()
                    .is_some_and(|scene| scene.identity == *identity)
                    && self.scroll_2d.as_ref().is_some_and(|scroll| {
                        scroll.epoch == *epoch && scroll.valid_offset(*offset)
                    })
            }
            CanvasEvent::ScrollInput { epoch, change } => {
                *epoch != 0
                    && self.scroll_epoch == Some(*epoch)
                    && self.scroll.as_ref().is_some_and(|range| match change {
                        ScrollChange::Delta { delta_y } => finite(*delta_y),
                        ScrollChange::To { offset } => {
                            offset.is_finite() && *offset >= 0. && *offset <= range.content
                        }
                    })
            }
            CanvasEvent::Scroll { offset } => self.scroll.as_ref().is_some_and(|range| {
                offset.is_finite() && *offset >= 0. && *offset <= range.content
            }),
            CanvasEvent::Resize {
                width,
                height,
                grid,
            } => {
                finite(*width)
                    && finite(*height)
                    && *width >= 0.
                    && *height >= 0.
                    && grid.is_some() == self.grid
                    && grid.as_ref().is_none_or(|grid| {
                        finite(grid.cell_width)
                            && finite(grid.cell_height)
                            && grid.cell_width > 0.
                            && grid.cell_height > 0.
                    })
            }
            CanvasEvent::Focus { .. } => self.focusable,
            CanvasEvent::Key { key, .. } => self.focusable && key.len() <= 128,
            CanvasEvent::Text { text } => self.focusable && text.len() <= 65536,
            CanvasEvent::Pointer { x, y, button, .. } => finite(*x) && finite(*y) && *button <= 2,
            CanvasEvent::Wheel {
                x,
                y,
                delta_x,
                delta_y,
                ..
            } => [*x, *y, *delta_x, *delta_y].into_iter().all(finite),
        }
    }
}

impl SideTabs {
    /// Item identity and current availability are rechecked before native callbacks reach the guest.
    fn accepts(&self, action: &Action) -> bool {
        let item = |id: &str| {
            self.items
                .iter()
                .find(|item| item.id == id && !item.disabled)
        };
        match action {
            Action::Select(id) => item(id).is_some(),
            Action::Close(id) => item(id).is_some_and(|item| item.closable),
            Action::Rename { id, value } => item(id).is_some() && value.len() <= 4096,
            Action::Move { from, to } => item(from).is_some() && item(to).is_some(),
            Action::Context { id, x, y } => {
                item(id).is_some()
                    && x.is_finite()
                    && y.is_finite()
                    && (0. ..=10000.).contains(x)
                    && (0. ..=10000.).contains(y)
            }
            Action::Resize(width) => {
                width.is_finite() && (self.min_width..=self.max_width).contains(width)
            }
            _ => false,
        }
    }
}
