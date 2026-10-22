//! One-shot lazy defaults and theme-aware row text never acquire activation or focus authority.
use super::*;

/// Semantic tones use existing success/danger tokens and a readable informational blue in each theme.
pub(super) fn text_color(tone: api::Tone, cx: &App) -> gpui_kit::Hsla {
    match tone {
        api::Tone::Positive => cx.theme().success,
        api::Tone::Negative => cx.theme().danger,
        api::Tone::Informative => gpui_kit::rgb(if cx.theme().is_dark() {
            0x60a5fa
        } else {
            0x1d4ed8
        })
        .into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{TestAppContext, gpui};
    /// Both appearances preserve red/green meanings and a blue changed-file label.
    #[gpui::test]
    fn semantic_tree_tones_resolve_in_both_themes(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            editor_ui::typography::init(cx);
            for dark in [false, true] {
                crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(dark), cx);
                let positive = gpui_kit::Rgba::from(text_color(api::Tone::Positive, cx));
                let negative = gpui_kit::Rgba::from(text_color(api::Tone::Negative, cx));
                let information = gpui_kit::Rgba::from(text_color(api::Tone::Informative, cx));
                assert!(positive.g > positive.r && positive.g > positive.b);
                assert!(negative.r > negative.g && negative.r > negative.b);
                assert!(information.b > information.r && information.b > information.g);
            }
        });
    }
}

impl TreeView {
    /// Observe real input during capture, before Base consumes scrolling or selects a row. The
    /// display-only canvas never consumes the event and cannot withdraw another scene's seed.
    pub(super) fn initial_input_guard(&self, cx: &Context<Self>) -> gpui_kit::AnyElement {
        let owner = cx.entity().downgrade();
        let gesture = self.action_gesture;
        gpui_kit::canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let wheel = owner.clone();
                window.on_mouse_event(move |event: &gpui_kit::ScrollWheelEvent, phase, _, cx| {
                    if phase.capture() && bounds.contains(&event.position) {
                        let _ = wheel.update(cx, |this, _| {
                            if this.frame_current(gesture) {
                                this.resource.dismiss_initial_selection();
                            }
                        });
                    }
                });
                let pointer = owner.clone();
                window.on_mouse_event(move |event: &gpui_kit::MouseDownEvent, phase, _, cx| {
                    if phase.capture() && bounds.contains(&event.position) {
                        let _ = pointer.update(cx, |this, _| {
                            if this.frame_current(gesture) {
                                this.resource.dismiss_initial_selection();
                            }
                        });
                    }
                });
            },
        )
        .absolute()
        .inset_0()
        .size_full()
        .into_any_element()
    }

    /// Follow only the provider's exact ancestor chain, with ordinary bounded pages and retry policy.
    /// The returned cached target may be selected silently after Base's item projection catches up.
    pub(super) fn initial_target(
        &self,
        snapshot: &TreeSnapshot,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        let path = snapshot.initial_selection.as_ref()?;
        let mut parent = None;
        for (index, id) in path.iter().enumerate() {
            let branch = snapshot.branches.get(&parent)?;
            if !branch.children.contains(id) {
                if !branch.loading && branch.error.is_none() {
                    if !branch.loaded || branch.next.is_some() {
                        (self.sink)(
                            Dispatch::Load {
                                parent,
                                continuation: branch.loaded,
                                initial: true,
                            },
                            self.revision,
                            self.action_gesture,
                            cx,
                        );
                    } else {
                        self.resource.dismiss_initial_selection();
                    }
                }
                return None;
            }
            let node = snapshot.nodes.get(id)?;
            if node.disabled || (index + 1 < path.len() && !node.branch) {
                self.resource.dismiss_initial_selection();
                return None;
            }
            if index + 1 == path.len() {
                self.resource.dismiss_initial_selection();
                return Some(id.clone());
            }
            if !snapshot.expanded.contains(id) {
                (self.sink)(
                    Dispatch::Load {
                        parent: Some(id.clone()),
                        continuation: false,
                        initial: true,
                    },
                    self.revision,
                    self.action_gesture,
                    cx,
                );
                return None;
            }
            parent = Some(id.clone());
        }
        None
    }
}
