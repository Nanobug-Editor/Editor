//! Explicit difference boundaries confirm cross-file navigation without wrapping or moving source selections.
use super::*;
use std::time::{Duration, Instant};

const NOTICE_DURATION: Duration = Duration::from_secs(2);

/// A second press must have the same direction and precede this exact notice's expiry.
#[derive(Clone, Copy)]
pub(super) struct BoundaryPrompt {
    next: bool,
    started: Instant,
}

impl BoundaryPrompt {
    fn confirms(self, next: bool, now: Instant) -> bool {
        self.next == next && now.saturating_duration_since(self.started) < NOTICE_DURATION
    }
}

impl EditorApp {
    /// Initial edge requests scroll complete content, while ordinary opens retain first-difference centering.
    pub(super) fn reveal_comparison_position(
        &mut self,
        position: api::ComparisonPosition,
        cx: &mut App,
    ) {
        let Some(comparison) = &self.document_comparison else {
            return;
        };
        let count = third::navigation_targets(comparison).len();
        if let Some(view) = self
            .comparison_views
            .iter_mut()
            .find(|view| Some(&view.id) == self.active_comparison.as_ref())
        {
            view.edge = (!position.is_default()).then_some(position);
            view.hunk = if position == api::ComparisonPosition::End {
                count.saturating_sub(1)
            } else {
                0
            };
        }
        let unified = self
            .comparison_views
            .iter()
            .any(|view| Some(&view.id) == self.active_comparison.as_ref() && view.unified);
        if unified && let Some(editor) = &comparison.unified_editor {
            let locator = match position {
                api::ComparisonPosition::FirstDifference => comparison
                    .unified_hunks
                    .first()
                    .map(|range| crate::editor::viewport::Locator::centered(range.start)),
                api::ComparisonPosition::Start => {
                    Some(crate::editor::viewport::Locator::new(0, 0.))
                }
                api::ComparisonPosition::End => Some(crate::editor::viewport::Locator::bottom(
                    editor.read(cx).text().len(),
                )),
            };
            *comparison.navigation.borrow_mut() = [locator, None, None];
            return;
        }
        if position == api::ComparisonPosition::FirstDifference {
            self.reveal_comparison_hunk(0, cx);
            return;
        }
        let end = position == api::ComparisonPosition::End;
        *comparison.navigation.borrow_mut() = [None, None, None];
        if !comparison.alignment.reveal_content_edge(end, cx) {
            let editors = [
                Some(&comparison.left_editor),
                Some(&comparison.right_editor),
                comparison.third.as_ref().map(|third| &third.editor),
            ];
            *comparison.navigation.borrow_mut() = editors.map(|editor| {
                editor.map(|editor| {
                    if end {
                        crate::editor::viewport::Locator::bottom(editor.read(cx).text().len())
                    } else {
                        crate::editor::viewport::Locator::new(0, 0.)
                    }
                })
            });
        }
    }

    /// Return a nonfocusable notice only for the currently mounted comparison and its live deadline.
    pub(super) fn comparison_boundary_notice(&self, cx: &App) -> Option<gpui_kit::AnyElement> {
        let prompt = self
            .comparison_views
            .iter()
            .find(|view| Some(&view.id) == self.active_comparison.as_ref())?
            .boundary?;
        if !prompt.confirms(prompt.next, Instant::now()) {
            return None;
        }
        Some(
            editor_ui::centered_notice(
                if prompt.next {
                    t!("editor.diff_last_boundary")
                } else {
                    t!("editor.diff_first_boundary")
                }
                .to_string(),
                cx,
            )
            .into_any_element(),
        )
    }

    /// Reaching a boundary first warns; only another same-direction press in two seconds leaves this file.
    pub(super) fn navigate_comparison_hunk(
        &mut self,
        next: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(comparison) = &self.document_comparison else {
            return;
        };
        let count = third::navigation_targets(comparison).len();
        if count == 0 {
            return;
        }
        let Some(view) = self
            .comparison_views
            .iter_mut()
            .find(|view| Some(&view.id) == self.active_comparison.as_ref())
        else {
            return;
        };
        let index = view.hunk.min(count - 1);
        // Entering from a content edge must visit its nearest hunk before advancing past it.
        let inward = match (view.edge, next) {
            (Some(api::ComparisonPosition::Start), true) => Some(0),
            (Some(api::ComparisonPosition::End), false) => Some(count - 1),
            _ => None,
        };
        if inward.is_none() && ((next && index == count - 1) || (!next && index == 0)) {
            let now = Instant::now();
            if view
                .boundary
                .is_some_and(|prompt| prompt.confirms(next, now))
            {
                self.dispatch_comparison_file(next, true, cx);
                cx.notify();
                return;
            }
            let id = view.id.clone();
            view.boundary = Some(BoundaryPrompt { next, started: now });
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(NOTICE_DURATION).await;
                let _ = this.update(cx, |app, cx| {
                    if let Some(view) = app.comparison_views.iter_mut().find(|view| view.id == id)
                        && view.boundary.is_some_and(|prompt| prompt.started == now)
                    {
                        view.boundary = None;
                        cx.notify();
                    }
                });
            })
            .detach();
            cx.notify();
            return;
        }
        view.boundary = None;
        view.edge = None;
        let index = inward.unwrap_or_else(|| if next { index + 1 } else { index - 1 });
        view.hunk = index;
        if view.unified {
            if let Some(range) = comparison.unified_hunks.get(index) {
                // Unified presentation follows the same silent viewport contract as native panes.
                *comparison.navigation.borrow_mut() = [
                    Some(crate::editor::viewport::Locator::centered(range.start)),
                    None,
                    None,
                ];
            }
        } else {
            self.reveal_comparison_hunk(index, cx);
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The confirmation window is strict and changing direction cannot consume the old prompt.
    #[test]
    fn boundary_confirmation_requires_same_direction_before_expiry() {
        let started = Instant::now();
        let prompt = BoundaryPrompt {
            next: true,
            started,
        };
        assert!(prompt.confirms(true, started + Duration::from_millis(1999)));
        assert!(!prompt.confirms(false, started + Duration::from_millis(1)));
        assert!(!prompt.confirms(true, started + NOTICE_DURATION));
    }
}
