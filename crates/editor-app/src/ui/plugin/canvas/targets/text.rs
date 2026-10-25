//! Seal each native input entity to one target epoch; ordinary drawing revisions retain that entity.
use super::*;
use editor_ui::text::input::{CanvasInputOwner, CanvasInputState};

/// Dropping the native owner revokes even a platform-retained handler before its entity is released.
pub(super) struct TextInput {
    pub state: Entity<CanvasInputState>,
    _owner: CanvasInputOwner,
    _changed: Subscription,
}
impl CanvasView {
    /// Find current native target focus independently of the Canvas-level legacy handler.
    fn text_focus(&self, window: &Window) -> Option<(String, u64)> {
        if !self.enabled {
            return None;
        }
        self.targets
            .focus
            .iter()
            .find(|(key, handle)| {
                handle.is_focused(window)
                    && self.drawing.scene.as_ref().is_some_and(|scene| {
                        scene.targets.iter().any(|target| {
                            target.id == key.0
                                && target.epoch == key.1
                                && target.text_input.is_some()
                        })
                    })
            })
            .map(|(key, _)| key.clone())
    }
    /// Reconcile one exact source, creating its native entity lazily when focus first reaches it.
    fn ensure_text(
        &mut self,
        key: &(String, u64),
        cx: &mut Context<Self>,
    ) -> Result<(), protocol::api::Failure> {
        let scene = self.drawing.scene.as_ref().ok_or_else(|| {
            protocol::api::Failure::new(
                protocol::api::ErrorCode::InvalidState,
                "Canvas text owner is absent",
            )
        })?;
        let binding = scene
            .targets
            .iter()
            .find(|target| target.id == key.0 && target.epoch == key.1)
            .and_then(|target| target.text_input.as_ref())
            .ok_or_else(|| {
                protocol::api::Failure::new(
                    protocol::api::ErrorCode::InvalidHandle,
                    "Canvas text target was replaced",
                )
            })?;
        let source = scene
            .paragraph_text(&binding.paragraph, &binding.layout_key)
            .ok_or_else(|| {
                protocol::api::Failure::new(
                    protocol::api::ErrorCode::StaleRevision,
                    "Canvas paragraph source was replaced",
                )
            })?;
        if let Some(input) = self.targets.sessions.get(key) {
            input
                .state
                .update(cx, |input, _| input.reconcile(binding, source))?;
        } else {
            let lifetime = CanvasInputOwner::new();
            let owner = cx.entity().downgrade();
            let target = key.clone();
            let input = CanvasInputState::new(
                binding,
                source,
                self.targets.focus[key].clone(),
                lifetime.lease(),
                self.targets.sequence.clone(),
                move |kind, _, cx| {
                    owner
                        .update(cx, |view, cx| {
                            let accepted = view.emit_target(target.clone(), kind, cx);
                            if accepted {
                                cx.notify();
                            }
                            accepted
                        })
                        .unwrap_or(false)
                },
            )?;
            let state = cx.new(|_| input);
            let changed = cx.observe(&state, |_, _, cx| cx.notify());
            self.targets.sessions.insert(
                key.clone(),
                TextInput {
                    state,
                    _owner: lifetime,
                    _changed: changed,
                },
            );
        }
        self.targets.sequence.set(
            self.targets
                .sequence
                .get()
                .max(binding.acknowledged_sequence),
        );
        Ok(())
    }
    /// Source publications acknowledge older commits without recreating focused IME entities.
    pub(super) fn sync_text_sessions(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.targets.sessions.retain(|key, _| {
            self.drawing.scene.as_ref().is_some_and(|scene| {
                scene.targets.iter().any(|target| {
                    target.id == key.0 && target.epoch == key.1 && target.text_input.is_some()
                })
            })
        });
        let mut keys = self.targets.sessions.keys().cloned().collect::<Vec<_>>();
        if let Some(key) = self.text_focus(window) {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
        for key in keys {
            match self.ensure_text(&key, cx) {
                Ok(()) => {
                    self.targets.text_errors.remove(&key);
                }
                Err(error) => {
                    let message = error.message.chars().take(128).collect::<String>();
                    let condition = (error.code, message.clone());
                    if self.targets.text_errors.get(&key) != Some(&condition) {
                        self.emit_target(
                            key.clone(),
                            CanvasInputKind::TextError {
                                code: error.code,
                                message,
                            },
                            cx,
                        );
                        self.targets.text_errors.insert(key, condition);
                    }
                }
            }
        }
    }
    /// Native blur cancels preedit while retaining already submitted domain-text acknowledgements.
    pub(super) fn cancel_target_text(&mut self, key: &(String, u64), cx: &mut Context<Self>) {
        let event = self
            .targets
            .sessions
            .get(key)
            .and_then(|input| input.state.update(cx, |input, _| input.cancel()));
        if let Some(event) = event {
            self.emit_target(key.clone(), event, cx);
            cx.notify();
        }
    }
    /// Resolve the currently focused private draft only for its current declared paragraph.
    pub(in crate::ui::plugin::canvas) fn paragraph_input(
        &self,
        id: &str,
        layout_key: &str,
        window: &Window,
        cx: &App,
    ) -> Option<Entity<CanvasInputState>> {
        let key = self.text_focus(window)?;
        let binding = self
            .drawing
            .scene
            .as_ref()?
            .targets
            .iter()
            .find(|target| target.id == key.0 && target.epoch == key.1)?
            .text_input
            .as_ref()?;
        let input = &self.targets.sessions.get(&key)?.state;
        (binding.paragraph == id
            && binding.layout_key == layout_key
            && input.read(cx).active(window))
        .then(|| input.clone())
    }
    /// Register the sealed target entity, never the enclosing legacy Canvas entity, with the OS.
    pub(in crate::ui::plugin::canvas) fn install_target_text_input(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(key) = self.text_focus(window) else {
            return;
        };
        let Some(input) = self
            .targets
            .sessions
            .get(&key)
            .map(|input| input.state.clone())
        else {
            return;
        };
        let Some(frame) = &self.targets.frame else {
            return;
        };
        let paragraph = input.read(cx).binding().paragraph.clone();
        let Ok(context) = scene::paragraph::input_context(frame, &paragraph) else {
            return;
        };
        let bounds = context.window_rect(context.bounds);
        input.update(cx, |input, _| input.set_layout(context, window));
        let focus = input.read(cx).focus_handle();
        if input.read(cx).active(window) {
            window.handle_input(&focus, ElementInputHandler::new(bounds, input), cx);
        }
    }
}
