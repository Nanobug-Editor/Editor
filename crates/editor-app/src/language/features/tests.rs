//! Native Base frames and keyboard events verify the shared direct/LSP presentation contract.
use super::*;
use gpui_base::Root;
use gpui_kit::gpui;
use gpui_kit::{
    AppContext as _, ElementId, Entity, KeyDownEvent, KeyUpEvent, Keystroke, ParentElement as _,
    Subscription, TestAppContext,
};
use plugin_runtime::plugin_protocol::api::{ResourceIdentity, TextPosition};
use std::{cell::RefCell, rc::Rc};

/// Keep the actual rendered entity and its event subscription in one native test view.
struct View {
    features: Entity<LanguageFeatures>,
    _events: Subscription,
}

impl Render for View {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(self.features.clone())
    }
}

/// Every role uses the same real native view, original source identity and event subscription.
struct NativeFixture<'a> {
    visual: &'a mut gpui_kit::VisualTestContext,
    features: Entity<LanguageFeatures>,
    cancellation: RequestCancellation,
    events: Rc<RefCell<Vec<LanguageFeatureEvent>>>,
}

/// Mount a real Base Root without any alternate text state or provider-specific UI adapter.
fn mount(cx: &mut TestAppContext, feature: direct::Feature) -> NativeFixture<'_> {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::apply_theme(crate::builtin_theme(false), cx);
    });
    let cancellation = RequestCancellation::default();
    let events = Rc::new(RefCell::new(Vec::new()));
    let capture = events.clone();
    let mut features = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            let entity = cx.new(|cx| {
                LanguageFeatures::new(
                    "arbitrary-provider".into(),
                    feature,
                    DocumentVersion {
                        id: "native-source".into(),
                        path: "中文.novel".into(),
                        revision: 9,
                    },
                    cancellation.clone(),
                    window,
                    cx,
                )
            });
            features = Some(entity.clone());
            let subscription =
                cx.subscribe(&entity, move |_, _, event: &LanguageFeatureEvent, _| {
                    capture.borrow_mut().push(event.clone());
                });
            View {
                features: entity,
                _events: subscription,
            }
        });
        Root::new(view, window, cx)
    });
    NativeFixture {
        visual,
        features: features.unwrap(),
        cancellation,
        events,
    }
}

/// A named, visible Base reference row emits exactly the keyboard-selected plain location.
#[gpui::test]
fn native_language_features_references_named_keyboard_navigation(cx: &mut TestAppContext) {
    let NativeFixture {
        visual,
        features,
        cancellation,
        events,
    } = mount(cx, direct::Feature::References);
    visual.update(|_, cx| {
        features.update(cx, |view, cx| {
            view.complete(
                Ok(direct::Result::References {
                    locations: vec![
                        direct::Location {
                            resource: ResourceIdentity::Local {
                                path: "中文.novel".into(),
                            },
                            position: TextPosition {
                                line: 0,
                                character: 2,
                            },
                            label: None,
                        },
                        direct::Location {
                            resource: ResourceIdentity::Local {
                                path: "中文.novel".into(),
                            },
                            position: TextPosition {
                                line: 1,
                                character: 0,
                            },
                            label: None,
                        },
                    ],
                }),
                cx,
            );
        })
    });
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    let bounds = visual
        .debug_bounds("language-feature-row-0")
        .expect("The real reference row must be painted");
    assert!(bounds.size.width > gpui_kit::px(0.));
    visual.update(|window, _| {
        let nodes = gpui_base::test_support::snapshots(window);
        let row = nodes
            .iter()
            .find(|node| {
                node.path().iter().any(|part| {
            matches!(part, ElementId::Name(name) if name.as_str() == "language-feature-row-0")
        })
            })
            .expect("Base must publish the reference accessibility node");
        assert_eq!(row.role(), Some(gpui_kit::Role::Button));
        assert_eq!(row.label(), Some("中文.novel:1:3"));
        assert!(row.visible());
        assert_eq!(
            row.focused(),
            Some(true),
            "The first reply focuses its native row"
        );
    });
    visual.simulate_keystrokes("down");
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
        let nodes = gpui_base::test_support::snapshots(window);
        let selected = nodes
            .iter()
            .find(|node| {
                node.path().iter().any(|part| {
            matches!(part, ElementId::Name(name) if name.as_str() == "language-feature-row-1")
        })
            })
            .unwrap();
        assert_eq!(
            selected.focused(),
            Some(true),
            "Arrow selection follows native focus"
        );
    });
    // The shortcut helper emits KeyDown only; native Base buttons activate on a complete key press.
    let keystroke = Keystroke::parse("enter").unwrap();
    visual.simulate_event(KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    visual.simulate_event(KeyUpEvent { keystroke });
    visual.run_until_parked();
    let selections = events.borrow();
    let selected: Vec<_> = selections
        .iter()
        .filter_map(|event| match event {
            LanguageFeatureEvent::Navigate(location) => Some(location),
            _ => None,
        })
        .collect();
    assert_eq!(selected.len(), 1, "Keyboard Enter selects one proposal");
    assert_eq!(selected[0].position.line, 1);
    assert!(
        cancellation.is_cancelled(),
        "Closing presentation ends this request lifetime"
    );
}

/// Active overload parameters and inert documentation are visible in the actual native card.
#[gpui::test]
fn native_language_features_signature_card_preserves_active_parameter_and_plain_text(
    cx: &mut TestAppContext,
) {
    let NativeFixture {
        visual,
        features,
        cancellation,
        events,
    } = mount(cx, direct::Feature::SignatureHelp);
    let signature = direct::Signature {
        label: "f(🙂, 参数)".into(),
        documentation: Some("<b>inert 中文🙂</b>".into()),
        parameters: vec![
            direct::Parameter {
                label: "🙂".into(),
                documentation: Some("emoji docs".into()),
            },
            direct::Parameter {
                label: "参数".into(),
                documentation: None,
            },
        ],
        active_parameter: None,
    };
    visual.update(|_, cx| {
        features.update(cx, |view, cx| {
            view.complete(
                Ok(direct::Result::SignatureHelp {
                    help: Some(direct::SignatureHelp {
                        signatures: vec![
                            signature.clone(),
                            direct::Signature {
                                active_parameter: Some(0),
                                ..signature
                            },
                        ],
                        active_signature: Some(1),
                        active_parameter: Some(1),
                    }),
                }),
                cx,
            )
        })
    });
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    let card = visual
        .debug_bounds("language-signature-card-1")
        .expect("The real active overload must be painted");
    assert!(card.size.height > gpui_kit::px(0.));
    visual.update(|window, _| {
        let nodes = gpui_base::test_support::snapshots(window);
        let parameter = nodes.iter().find(|node| node.path().iter().any(|part| {
            matches!(part, ElementId::Name(name) if name.as_str() == "language-signature-parameter-1")
        })).expect("The active parameter must have a plain accessible name");
        assert_eq!(parameter.role(), Some(gpui_kit::Role::Label));
        assert_eq!(parameter.label(), Some(rust_i18n::t!("language_features.parameter", parameter = "🙂").as_ref()));
        let documentation = nodes.iter().find(|node| node.path().iter().any(|part| {
            matches!(part, ElementId::Name(name) if name.as_str() == "language-signature-documentation-1")
        })).expect("Documentation must remain visible plain text");
        assert_eq!(documentation.role(), Some(gpui_kit::Role::Label));
        assert_eq!(documentation.label(), Some("<b>inert 中文🙂</b>"));
        let close = nodes.iter().find(|node| node.path().iter().any(|part| {
            matches!(part, ElementId::Name(name) if name.as_str() == "language-features-close")
        })).unwrap();
        assert_eq!(close.role(), Some(gpui_kit::Role::Button));
        assert_eq!(close.label(), Some(rust_i18n::t!("language_features.close").as_ref()));
        assert!(close.visible());
    });
    assert!(
        visual.debug_bounds("language-features-choose").is_none(),
        "Readonly signatures expose no effect action"
    );
    visual.simulate_keystrokes("escape");
    visual.run_until_parked();
    assert!(cancellation.is_cancelled());
    assert!(matches!(
        events.borrow().as_slice(),
        [LanguageFeatureEvent::Dismissed]
    ));
}

/// Selecting a named Base action returns the original plain plan once; this view never executes it.
#[gpui::test]
fn native_language_features_code_action_returns_selected_complete_plan(cx: &mut TestAppContext) {
    let NativeFixture {
        visual,
        features,
        cancellation,
        events,
    } = mount(cx, direct::Feature::CodeActions);
    let document = visual.update(|_, cx| features.read(cx).document().clone());
    let action = |title: &str| Fix {
        title: title.into(),
        plan: plugin_runtime::plugin_protocol::workspace::Plan {
            title: title.into(),
            origin: Some(document.clone()),
            conditions: Vec::new(),
            changes: vec![
                plugin_runtime::plugin_protocol::workspace::Change::EditDocument {
                    document: document.clone(),
                    edits: vec![plugin_runtime::plugin_protocol::api::TextEdit {
                        range: plugin_runtime::plugin_protocol::api::TextRange {
                            start: 6,
                            end: 10,
                        },
                        text: "新🙂".into(),
                    }],
                    selection: plugin_runtime::plugin_protocol::api::TextRange {
                        start: 13,
                        end: 13,
                    },
                },
            ],
        },
    };
    visual.update(|_, cx| {
        features.update(cx, |view, cx| {
            view.complete(
                Ok(direct::Result::CodeActions {
                    actions: vec![action("First proposal"), action("修复 中文🙂")],
                }),
                cx,
            )
        })
    });
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        visual.debug_bounds("language-feature-row-0").is_some(),
        "The real action list must be painted"
    );
    visual.update(|window, _| {
        let nodes = gpui_base::test_support::snapshots(window);
        let row = nodes
            .iter()
            .find(|node| {
                node.path().iter().any(|part| {
                matches!(part, ElementId::Name(name) if name.as_str() == "language-feature-row-1")
            })
            })
            .expect("The fix proposal must have a native accessibility row");
        assert_eq!(row.role(), Some(gpui_kit::Role::Button));
        assert_eq!(row.label(), Some("修复 中文🙂"));
        assert!(row.visible());
    });
    visual.simulate_keystrokes("down");
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    // Match Base's complete native activation rather than invoking the view's private selection method.
    let keystroke = Keystroke::parse("enter").unwrap();
    visual.simulate_event(KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    visual.simulate_event(KeyUpEvent { keystroke });
    visual.run_until_parked();
    let events = events.borrow();
    let fixes = events
        .iter()
        .filter_map(|event| {
            if let LanguageFeatureEvent::Fix(fix) = event {
                Some(fix)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(fixes.len(), 1);
    assert_eq!(fixes[0].title, "修复 中文🙂");
    assert_eq!(fixes[0].plan.origin.as_ref(), Some(&document));
    assert_eq!(fixes[0].plan.changes.len(), 1);
    assert!(cancellation.is_cancelled());
}
