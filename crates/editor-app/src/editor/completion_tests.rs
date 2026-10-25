//! Exercise native completion actions and the painted documentation surfaces beside the editor.

use super::*;
use editor_core::Workspace;
use gpui_base::Root;
use gpui_base::input::{CompletionProvider, Rope};
use gpui_kit::gpui;
use gpui_kit::{TestAppContext, VisualTestContext};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionResponse, Hover, HoverContents, MarkupContent,
    MarkupKind,
};
use std::cell::RefCell;

/// Open an isolated document so completion tests never depend on installed servers.
fn completion_editor(
    cx: &mut TestAppContext,
) -> (tempfile::TempDir, Entity<EditorApp>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        theme::apply_theme(theme::builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sample.txt");
    std::fs::write(&path, "").unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, cx) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| EditorApp::new(workspace, Some(path), window, cx));
        *capture.borrow_mut() = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = slot.borrow_mut().take().unwrap();
    cx.simulate_resize(size(px(1000.), px(800.)));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    (directory, view, cx)
}

/// Moving past the viewport and back must keep the entire selected row visible.
#[gpui::test]
fn completion_keyboard_selection_stays_visible(cx: &mut TestAppContext) {
    let (_directory, view, cx) = completion_editor(cx);
    cx.update(|_, cx| {
        view.read(cx).editor.clone().update(cx, |state, cx| {
            state.present_completion_items(
                0,
                "",
                (0..24)
                    .map(|index| CompletionItem {
                        label: format!("candidate_{index:02}"),
                        ..Default::default()
                    })
                    .collect(),
                cx,
            );
        });
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    for (key, selected, selector) in [
        ("down", 23, "editor-completion-row-23"),
        ("up", 0, "editor-completion-row-0"),
    ] {
        for _ in 0..23 {
            cx.simulate_keystrokes(key);
        }
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(
            cx.update(|_, cx| view.read(cx).completion_popup.selection.get().1),
            selected
        );
        let panel = cx.debug_bounds("editor-completion-card").unwrap();
        let row = cx.debug_bounds(selector).unwrap();
        assert!(
            row.top() >= panel.top() && row.bottom() <= panel.bottom(),
            "selected completion {selected} must stay inside the panel: {row:?} / {panel:?}"
        );
    }
}

/// Record requests and widen the candidate set when the user shortens the prefix.
struct DeletionCompletions {
    requests: Rc<RefCell<Vec<String>>>,
    /// Tests can hold refresh responses to check cancellation and out-of-order delivery.
    pending: Option<Rc<RefCell<Vec<futures::channel::oneshot::Sender<CompletionResponse>>>>>,
}

impl CompletionProvider for DeletionCompletions {
    /// Ready responses make the key-to-refresh assertion deterministic.
    fn completions(
        &self,
        text: &Rope,
        _offset: usize,
        _trigger: CompletionContext,
        _window: &mut Window,
        cx: &mut App,
    ) -> gpui_kit::Task<anyhow::Result<CompletionResponse>> {
        let source = text.to_string();
        self.requests.borrow_mut().push(source.clone());
        if source != "pri"
            && let Some(pending) = &self.pending
        {
            let (sender, receiver) = futures::channel::oneshot::channel();
            pending.borrow_mut().push(sender);
            return cx.spawn(async move |_| Ok(receiver.await?));
        }
        let labels = if source == "pri" {
            vec!["print"]
        } else {
            vec!["print", "probe"]
        };
        gpui_kit::Task::ready(Ok(CompletionResponse::Array(
            labels
                .into_iter()
                .map(|label| CompletionItem {
                    label: label.into(),
                    ..Default::default()
                })
                .collect(),
        )))
    }

    /// Model the production provider's identifier trigger policy.
    fn is_completion_trigger(&self, _offset: usize, new_text: &str, _cx: &mut App) -> bool {
        !new_text.is_empty() && new_text.chars().all(char::is_alphanumeric)
    }
}

/// Backspace must retain the panel and fetch candidates for the shortened input.
#[gpui::test]
fn completion_backspace_refreshes_visible_candidates(cx: &mut TestAppContext) {
    let (_directory, view, cx) = completion_editor(cx);
    let requests = Rc::new(RefCell::new(Vec::new()));
    cx.update(|_, cx| {
        view.read(cx).editor.clone().update(cx, |state, _| {
            state.lsp_mut().completion_provider = Some(Rc::new(DeletionCompletions {
                requests: requests.clone(),
                pending: None,
            }));
        });
    });
    cx.simulate_input("pri");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("editor-completion-card").is_some());
    cx.simulate_keystrokes("backspace");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|_, cx| {
        let state = view.read(cx).editor.read(cx);
        assert_eq!(state.text().to_string(), "pr");
        assert!(
            state.completion_menu_state().open,
            "backspace must retain the panel"
        );
        assert_eq!(state.completion_menu_state().items.len(), 2);
    });
    assert_eq!(requests.borrow().last().map(String::as_str), Some("pr"));
}

/// The forward Delete binding must refresh using the same native editing path.
#[gpui::test]
fn completion_forward_delete_refreshes_candidates(cx: &mut TestAppContext) {
    let (_directory, view, cx) = completion_editor(cx);
    let requests = Rc::new(RefCell::new(Vec::new()));
    cx.update(|_, cx| {
        view.read(cx).editor.clone().update(cx, |state, _| {
            state.lsp_mut().completion_provider = Some(Rc::new(DeletionCompletions {
                requests: requests.clone(),
                pending: None,
            }));
        });
    });
    cx.simulate_input("pri");
    cx.run_until_parked();
    cx.update(|window, cx| {
        view.read(cx).editor.clone().update(cx, |state, cx| {
            let items = state.completion_menu_state().items.clone();
            state.set_cursor_position(lsp_types::Position::new(0, 2), window, cx);
            state.present_completion_items(0, "pr", items, cx);
        });
        window.draw(cx).clear(cx);
    });
    cx.simulate_keystrokes("delete");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|_, cx| {
        let state = view.read(cx).editor.read(cx);
        assert_eq!(state.text().to_string(), "pr");
        assert!(state.completion_menu_state().open);
        assert_eq!(state.completion_menu_state().items.len(), 2);
    });
    assert_eq!(requests.borrow().last().map(String::as_str), Some("pr"));
}

/// Pending edits cannot be inserted, and an older response cannot replace a newer list.
#[gpui::test]
fn completion_deletion_refresh_keeps_panel_and_discards_stale_responses(cx: &mut TestAppContext) {
    let (_directory, view, cx) = completion_editor(cx);
    let requests = Rc::new(RefCell::new(Vec::new()));
    let pending = Rc::new(RefCell::new(Vec::new()));
    cx.update(|_, cx| {
        view.read(cx).editor.clone().update(cx, |state, _| {
            state.lsp_mut().completion_provider = Some(Rc::new(DeletionCompletions {
                requests: requests.clone(),
                pending: Some(pending.clone()),
            }));
        });
    });
    cx.simulate_input("pri");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    // simulate_input types one character at a time; earlier typing requests were cancelled.
    pending.borrow_mut().clear();
    cx.simulate_keystrokes("backspace");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let row = cx.debug_bounds("editor-completion-row-0").unwrap();
    assert!(cx.debug_bounds("editor-completion-card").is_some());
    cx.simulate_keystrokes("enter");
    cx.simulate_click(row.center(), Default::default());
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(view.read(cx).editor.read(cx).text().to_string(), "pr");
    });

    cx.simulate_keystrokes("backspace");
    cx.run_until_parked();
    assert_eq!(pending.borrow().len(), 2);
    pending
        .borrow_mut()
        .pop()
        .unwrap()
        .send(CompletionResponse::Array(vec![CompletionItem {
            label: "push".into(),
            ..Default::default()
        }]))
        .unwrap();
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    pending
        .borrow_mut()
        .pop()
        .unwrap()
        .send(CompletionResponse::Array(vec![CompletionItem {
            label: "outdated".into(),
            ..Default::default()
        }]))
        .unwrap();
    cx.run_until_parked();
    cx.update(|_, cx| {
        let state = view.read(cx).editor.read(cx);
        assert_eq!(state.text().to_string(), "p");
        assert_eq!(state.completion_menu_state().items[0].label, "push");
    });

    // Even deleting the entire prefix must request candidates; Escape cancels that request.
    cx.simulate_keystrokes("backspace");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("editor-completion-card").is_some());
    assert_eq!(requests.borrow().last().map(String::as_str), Some(""));
    cx.simulate_keystrokes("escape");
    pending
        .borrow_mut()
        .pop()
        .unwrap()
        .send(CompletionResponse::Array(vec![CompletionItem {
            label: "cancelled".into(),
            ..Default::default()
        }]))
        .unwrap();
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("editor-completion-card").is_none());
}

/// Supply unfiltered candidates through the production matcher and replacement range logic.
struct FuzzyCompletions;

impl CompletionProvider for FuzzyCompletions {
    /// Match against the exact byte cursor, including any non-ASCII text before it.
    fn completions(
        &self,
        text: &Rope,
        offset: usize,
        _trigger: CompletionContext,
        _window: &mut Window,
        _cx: &mut App,
    ) -> gpui_kit::Task<anyhow::Result<CompletionResponse>> {
        let prefix = crate::language::completion::identifier_prefix(&text.to_string(), offset);
        gpui_kit::Task::ready(Ok(crate::language::completion::rank_completions(
            CompletionResponse::Array(vec![CompletionItem {
                label: "println".into(),
                ..Default::default()
            }]),
            &prefix,
            text.offset_to_position(offset - prefix.len()),
            text.offset_to_position(offset),
        )))
    }

    /// Let this fixture exercise a complete typed member expression in one input event.
    fn is_completion_trigger(&self, _offset: usize, new_text: &str, _cx: &mut App) -> bool {
        !new_text.is_empty()
    }
}

/// Accepting a fuzzy candidate replaces the abbreviation without damaging preceding UTF-8 text.
#[gpui::test]
fn completion_fuzzy_candidate_replaces_abbreviation(cx: &mut TestAppContext) {
    let (_directory, view, cx) = completion_editor(cx);
    cx.update(|_, cx| {
        view.read(cx).editor.clone().update(cx, |state, _| {
            state.lsp_mut().completion_provider = Some(Rc::new(FuzzyCompletions));
        });
    });
    cx.simulate_input("变量.pn");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("editor-completion-card").is_some());
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(
            view.read(cx).editor.read(cx).text().to_string(),
            "变量.println"
        );
    });
}

/// Compare opaque foreground/background colors in linear light, independent of card styling.
fn hover_text_contrast(foreground: Hsla, background: Hsla) -> f32 {
    let luminance = |color: Hsla| {
        let color = color.to_rgb();
        let linear = |channel: f32| {
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
    };
    let foreground = luminance(foreground);
    let background = luminance(background);
    (foreground.max(background) + 0.05) / (foreground.min(background) + 0.05)
}

/// Plain documentation and both code forms must stay readable when the live theme changes.
#[gpui::test]
fn hover_documentation_surface_contrasts_with_text_in_both_themes(cx: &mut TestAppContext) {
    let (_directory, view, cx) = completion_editor(cx);
    cx.simulate_input("message");
    cx.run_until_parked();
    for dark in [true, false] {
        cx.update(|_, cx| theme::apply_theme(theme::builtin_theme(dark), cx));
        for (form, markdown) in [
            (
                "prose",
                "(method) String.toUpperCase(): string\n\nConverts all alphabetic characters to uppercase.",
            ),
            (
                "inline code",
                "Call `message.toUpperCase()` to convert the value.",
            ),
            (
                "fenced code",
                "Method signature:\n\n```typescript\ntoUpperCase(): string\n```",
            ),
            (
                "structured documentation",
                "example::editor\n\n```rust\npub struct Alpha;\n```\n\n---\n\nsize = 0, align = 0x1, no Drop\n\nShows type details for the selected symbol. Use `Alpha` to inspect its documentation.\n\n支持中文说明，代码与正文保持清晰的层次。",
            ),
        ] {
            cx.update(|_, cx| {
                view.read(cx).editor.clone().update(cx, |state, cx| {
                    state.present_hover(
                        0..7,
                        Hover {
                            contents: HoverContents::Markup(MarkupContent {
                                kind: MarkupKind::Markdown,
                                value: markdown.into(),
                            }),
                            range: None,
                        },
                        cx,
                    );
                });
            });
            cx.run_until_parked();
            cx.update(|window, cx| window.draw(cx).clear(cx));
            let card = cx.debug_bounds("editor-definition-card").unwrap();
            let text = cx.debug_bounds("editor-definition-text").unwrap();
            assert!(text.size.width > px(0.) && text.size.height > px(0.));
            if form == "structured documentation" {
                assert!(
                    text.bottom() <= card.bottom() - px(8.),
                    "short wrapped documentation must fit before the popup's height limit: card={card:?}, text={text:?}"
                );
            }
            let (painted, expected_surface, foreground) = cx.update(|window, cx| {
                let bounds = card.scale(window.scale_factor());
                let painted = window
                    .painted_quads()
                    .into_iter()
                    // Paint snaps edges to physical pixels; a source/editor quad must not
                    // masquerade as this card merely because it covers the same center.
                    .filter(|quad| {
                        (quad.bounds.left().0 - bounds.left().0).abs() <= 1.
                            && (quad.bounds.right().0 - bounds.right().0).abs() <= 1.
                            && (quad.bounds.top().0 - bounds.top().0).abs() <= 1.
                            && (quad.bounds.bottom().0 - bounds.bottom().0).abs() <= 1.
                            && quad.content_mask.bounds.contains(&bounds.center())
                    })
                    .max_by_key(|quad| quad.order)
                    .and_then(|quad| quad.background.as_solid())
                    .expect("the visible hover card must paint its own solid surface");
                (painted, cx.theme().popover, cx.theme().foreground)
            });
            // Local Markdown explicitly uses the theme foreground. Test its required contrast
            // against the painted surface, rather than a second copy of the card's style logic.
            let contrast = hover_text_contrast(foreground, painted);
            assert!(
                contrast >= 4.5,
                "{form} hover text is unreadable in dark={dark}: contrast={contrast:.2}, foreground={foreground:?}, painted={painted:?}"
            );
            assert_eq!(
                painted, expected_surface,
                "{form} hover must follow the live popover surface in dark={dark}"
            );
        }
    }
}
