//! A source handover may reject an old page intent without changing the provider's metadata.
//! Only a new metadata read may recover; native selection and command gestures are never replayed.
use super::*;
use crate::ui::controls;
use gpui_base::Root;
use gpui_base::input::EditorState;
use gpui_kit::{TestAppContext, VisualTestContext};
use gpui_kit::{gpui, px, size};
use plugin_runtime::{
    Manager,
    plugin_protocol::api::{self as protocol, DocumentVersion},
};
use std::{cell::RefCell, time::Duration};

/// Keep the actual package owner alive; no test constructor bypasses capability or tree ownership.
struct Provider {
    manager: Rc<RefCell<Manager>>,
    resource: Arc<TreeResource>,
    _storage: tempfile::TempDir,
    _workspace: tempfile::TempDir,
}

impl Provider {
    /// The frozen database package has a documented build/hash and uses the public Manager ingress.
    fn new() -> Self {
        let storage = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        let package = crate::test_packages::package("database-tree-0.1.1");
        let mut manager = Manager::open(
            storage.path().into(),
            Environment {
                workspace: workspace.path().display().to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        crate::extensions::demand_plugin_fixture(
            &mut manager,
            "database-tree",
            plugin_runtime::DemandReason::View("presentation".into()),
        )
        .unwrap();
        let resource = manager.live["database-tree"]
            .tree_resources()
            .pop()
            .unwrap();
        let this = Self {
            manager: Rc::new(RefCell::new(manager)),
            resource,
            _storage: storage,
            _workspace: workspace,
        };
        for parent in [None, Some("main"), Some("users")] {
            this.page(parent);
        }
        this
    }

    /// Seed provider pages through the same owned request/event/reply chain as the native adapter.
    fn page(&self, parent: Option<&str>) {
        let request = self
            .resource
            .request(parent.map(str::to_owned), false)
            .unwrap()
            .unwrap();
        let mut manager = self.manager.borrow_mut();
        let revision = manager.live["database-tree"].views["presentation"].revision;
        manager
            .event(
                "database-tree",
                Some("presentation".into()),
                protocol::Notification::Ui(UiEvent {
                    revision,
                    node: "catalog-tree".into(),
                    action: Action::Tree(api::Event::Request(request)),
                }),
            )
            .unwrap();
    }

    /// Refresh cancels the old pages but preserves stable expanded IDs; only the root is reseeded.
    fn refresh(&self) {
        self.manager
            .borrow_mut()
            .invoke_command("database-tree", "refresh-database", serde_json::Value::Null)
            .unwrap();
        self.page(None);
    }
}

/// The same native text entity and owner callback admit exactly one complete source version.
struct Mounted {
    view: Entity<PluginView>,
    source: Entity<EditorState>,
    version: DocumentVersion,
    current_revision: Rc<Cell<u64>>,
    events: Rc<RefCell<Vec<UiEvent>>>,
}

/// Embed the package's unchanged tree node in a native source action so every source guard is real.
fn document(version: &DocumentVersion, resource: &TreeResource, disabled: bool) -> Document {
    let mut document = Document::new(
        ui::Node::new(
            "source",
            Kind::EditorWithActions {
                document: version.clone(),
                left: vec![ui::EditorAction {
                    offset: 0,
                    content: Box::new(
                        ui::Node::new(
                            "catalog-tree",
                            Kind::Tree {
                                handle: resource.handle().clone(),
                            },
                        )
                        .width(240.)
                        .height(280.)
                        .disabled(disabled),
                    ),
                }],
                right: Vec::new(),
                sync_group: None,
            },
        )
        .width(600.)
        .height(320.),
    )
    .revision(1);
    document.source = Some(version.clone());
    document.editor_layout = true;
    document.validate().unwrap();
    document
}

/// Mount a headless Base Root and the production PluginView, forwarding original envelopes unchanged.
fn mount<'a>(
    cx: &'a mut TestAppContext,
    provider: &Provider,
) -> (Mounted, &'a mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::typography::init(cx);
        crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let version = DocumentVersion {
        id: "source:tree-refresh".into(),
        path: "tree-refresh.txt".into(),
        revision: 7,
    };
    let current_revision = Rc::new(Cell::new(version.revision));
    let events = Rc::new(RefCell::new(Vec::new()));
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let initial_version = version.clone();
    let revision = current_revision.clone();
    let output = events.clone();
    let manager = provider.manager.clone();
    let resource = provider.resource.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let source = cx.new(|cx| EditorState::new(window, cx).language("text"));
        source.update(cx, |source, cx| {
            source.set_value("source\n".repeat(30), window, cx)
        });
        let view = cx.new(|cx| {
            let mut view = PluginView::new(
                "database-tree".into(),
                document(&initial_version, &resource, false),
                Environment::default(),
                move |event, _| {
                    output.borrow_mut().push(event.clone());
                    manager
                        .borrow_mut()
                        .event(
                            "database-tree",
                            Some("presentation".into()),
                            protocol::Notification::Ui(event),
                        )
                        .unwrap();
                },
                window,
                cx,
            );
            view.set_code_sync_source(Some((initial_version.clone(), source.clone())), cx);
            let editor = source.clone();
            view.native_editor = Some(Rc::new(move |_, actions, _, _, cx| {
                let body = controls::window_editor_body(&editor, None, cx);
                Some(if let Some(actions) = actions {
                    controls::with_projected_editor_actions(body, &editor, actions, None)
                } else {
                    body
                })
            }));
            view.action_source_current = Some(Rc::new(move |target, _| {
                target.id == initial_version.id
                    && target.path == initial_version.path
                    && target.revision == revision.get()
            }));
            view.update_trees(&[resource], cx);
            view
        });
        *capture.borrow_mut() = Some((view.clone(), source));
        Root::new(view, window, cx)
    });
    visual.simulate_resize(size(px(640.), px(360.)));
    draw(visual);
    let (view, source) = slot.borrow_mut().take().unwrap();
    (
        Mounted {
            view,
            source,
            version,
            current_revision,
            events,
        },
        visual,
    )
}

/// Complete queued native defers without inventing another disclosure, selection or command.
fn draw(visual: &mut VisualTestContext) {
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    visual.run_until_parked();
}

/// Drive the production 150ms refresh task; virtual time never reaches the provider's 15s deadline.
fn pulse(visual: &mut VisualTestContext) {
    visual.executor().advance_clock(Duration::from_millis(150));
    draw(visual);
}

/// No pending-page slot is allocated when original source or active-node admission is withdrawn.
fn assert_no_pending(provider: &Provider, mounted: &Mounted, revision: u64) {
    let snapshot = provider.resource.snapshot();
    assert_eq!(snapshot.revision, revision);
    assert!(snapshot.branches.values().all(|branch| !branch.loading));
    assert!(mounted.events.borrow().is_empty());
}

/// A rejected old intent cannot make expanded metadata depend on another user gesture or refresh.
#[gpui::test]
#[ignore = "frozen database-tree Package/WASM and GPUI source handover; run explicitly with --ignored"]
fn native_tree_refresh_recovers_metadata_after_deferred_source_handover(cx: &mut TestAppContext) {
    let provider = Provider::new();
    let (mounted, visual) = mount(cx, &provider);
    let leaf = visual
        .debug_bounds("plugin-tree-row-user-0")
        .unwrap()
        .center();
    visual.simulate_click(leaf, Default::default());
    draw(visual);
    mounted.events.borrow_mut().clear();
    provider.refresh();
    let revision = provider.resource.snapshot().revision;
    let mut next = mounted.version.clone();
    next.revision += 1;
    visual.update(|_, cx| {
        mounted.view.update(cx, |view, cx| {
            // Reconciliation queues the expanded child intent in the old source incarnation.
            view.update_trees(&[provider.resource.clone()], cx);
            // The native owner advances before the guest's replacement scene is published.
            // Both the old gesture and the old Document's exact source must fail admission.
            mounted.current_revision.set(next.revision);
            view.set_code_sync_source(Some((next.clone(), mounted.source.clone())), cx);
        });
    });
    draw(visual);
    assert_no_pending(&provider, &mounted, revision);
    assert!(provider.resource.snapshot().expanded.contains("users"));

    // A subsequent guest publication may recover immediately; only the old turn had no authority.
    visual.update(|window, cx| {
        mounted.view.update(cx, |view, cx| {
            view.update_document(
                document(&next, &provider.resource, false),
                Environment::default(),
                window,
                cx,
            );
            view.update_trees(&[provider.resource.clone()], cx);
        });
    });
    // Three real pulses cover main, users and the final native projection without any new input.
    for _ in 0..3 {
        pulse(visual);
    }
    assert!(
        visual.debug_bounds("plugin-tree-row-user-0").is_some(),
        "the current source must recover its expanded provider pages after the old intent is rejected"
    );
    visual.update(|window, _| {
        let rows = gpui_base::test_support::snapshots(window);
        assert!(
            rows.iter()
                .any(|row| row.label() == Some("User 000") && row.selected() == Some(true))
        );
    });
    assert!(
        mounted
            .events
            .borrow()
            .iter()
            .all(|event| { matches!(&event.action, Action::Tree(api::Event::Request(_))) }),
        "metadata recovery must not replay Selected or Invoke business events"
    );
}

/// Retrying metadata must retain the existing source and disabled-node gates before allocating pages.
#[gpui::test]
#[ignore = "frozen database-tree Package/WASM and GPUI source guards; run explicitly with --ignored"]
fn native_tree_refresh_does_not_allocate_for_withdrawn_or_disabled_source(cx: &mut TestAppContext) {
    let provider = Provider::new();
    let (mounted, visual) = mount(cx, &provider);
    provider.refresh();
    mounted.events.borrow_mut().clear();
    let revision = provider.resource.snapshot().revision;
    visual.update(|_, cx| {
        mounted.view.update(cx, |view, cx| {
            view.set_code_sync_source(None, cx);
            view.update_trees(&[provider.resource.clone()], cx);
        });
    });
    pulse(visual);
    assert_no_pending(&provider, &mounted, revision);
    visual.update(|window, cx| {
        mounted.view.update(cx, |view, cx| {
            view.set_code_sync_source(Some((mounted.version.clone(), mounted.source.clone())), cx);
            view.update_document(
                document(&mounted.version, &provider.resource, true),
                Environment::default(),
                window,
                cx,
            );
            view.update_trees(&[provider.resource.clone()], cx);
        });
    });
    pulse(visual);
    assert_no_pending(&provider, &mounted, revision);
}
