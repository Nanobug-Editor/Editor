//! Native observations locate painted Base controls; activation uses ordinary Window input dispatch.
use super::*;
use gpui_base::test_support::{ElementSnapshot, snapshots};
use gpui_kit::{ElementId, KeyDownEvent, KeyUpEvent, Keystroke, Role, ScrollDelta};

/// Match the native identity while permitting source-action gesture incarnations to change.
pub(super) fn named(node: &ElementSnapshot, id: &str) -> bool {
    node.path().iter().any(|part| matches!(part, ElementId::Name(name) if name.as_ref() == id || name.starts_with(&format!("{id}-action-"))))
}

pub(super) fn button(window: &Window, id: &str) -> anyhow::Result<ElementSnapshot> {
    let native = format!("plugin-ui-{id}");
    snapshots(window)
        .into_iter()
        .find(|node| node.role() == Some(Role::Button) && node.visible() && named(node, &native))
        .with_context(|| format!("native button {id} not painted"))
}

/// Containers may retain two equally named files; scope selection by their real native tree ancestry.
pub(super) fn tree_item(
    window: &Window,
    tree: &str,
    label: &str,
) -> anyhow::Result<ElementSnapshot> {
    let scope = format!("plugin-tree-{tree}");
    snapshots(window)
        .into_iter()
        .find(|node| {
            node.role() == Some(Role::TreeItem)
                && node.visible()
                && node.label() == Some(label)
                && named(node, &scope)
        })
        .with_context(|| format!("native {tree} tree item {label} not painted"))
}

pub(super) fn tree(window: &Window, id: &str) -> anyhow::Result<ElementSnapshot> {
    let scope = format!("plugin-tree-{id}");
    snapshots(window)
        .into_iter()
        .find(|node| node.role() == Some(Role::Tree) && node.visible() && named(node, &scope))
        .with_context(|| format!("native {id} tree not painted"))
}

pub(super) fn field(window: &Window, id: &str) -> anyhow::Result<ElementSnapshot> {
    let scope = format!("plugin-ui-{id}-wrapper");
    snapshots(window)
        .into_iter()
        .find(|node| node.role() == Some(Role::TextInput) && node.visible() && named(node, &scope))
        .with_context(|| format!("native input {id} not painted"))
}

/// Readonly/document frames expose the original native value and source identity through Base's public snapshot.
pub(super) fn source(window: &Window, label: &str) -> anyhow::Result<ElementSnapshot> {
    snapshots(window)
        .into_iter()
        .find(|node| {
            matches!(node.role(), Some(Role::TextInput | Role::Document))
                && node.visible()
                && node.label() == Some(label)
        })
        .with_context(|| format!("native source {label} not painted"))
}

/// Actual bounds, visibility and disabled state are checked before any pointer event is dispatched.
pub(super) fn click(
    window: &mut Window,
    node: &ElementSnapshot,
    cx: &mut App,
) -> anyhow::Result<()> {
    ensure!(
        node.visible() && node.disabled() != Some(true),
        "native target unavailable: {node:?}"
    );
    let bounds = node.bounds();
    ensure!(
        bounds.size.width > px(0.) && bounds.size.height > px(0.),
        "native target has no geometry"
    );
    point_click(window, bounds.center(), cx)
}

/// A public geometry projection also serves code panes and the guest-drawn history, which have no Base button.
pub(super) fn point_click(
    window: &mut Window,
    position: Point<Pixels>,
    cx: &mut App,
) -> anyhow::Result<()> {
    point_press_count(window, position, 1, cx)
}

/// Dispatch a native double-click sequence at the real accessible row, including its first press.
pub(super) fn double_click(
    window: &mut Window,
    item: &ElementSnapshot,
    cx: &mut App,
) -> anyhow::Result<()> {
    point_press_count(window, item.bounds().center(), 1, cx)?;
    point_press_count(window, item.bounds().center(), 2, cx)
}

/// Deliver one validated native press/release pair with the platform's click count.
fn point_press_count(
    window: &mut Window,
    position: Point<Pixels>,
    click_count: usize,
    cx: &mut App,
) -> anyhow::Result<()> {
    let viewport = Bounds::new(point(px(0.), px(0.)), window.viewport_size());
    ensure!(
        viewport.contains(&position),
        "native pointer {position:?} lies outside {viewport:?}"
    );
    window.activate_window();
    window.dispatch_event(
        PlatformInput::MouseDown(MouseDownEvent {
            position,
            button: MouseButton::Left,
            modifiers: Default::default(),
            click_count,
            first_mouse: false,
        }),
        cx,
    );
    window.dispatch_event(
        PlatformInput::MouseUp(MouseUpEvent {
            position,
            button: MouseButton::Left,
            modifiers: Default::default(),
            click_count,
        }),
        cx,
    );
    Ok(())
}

/// Send one bounded nontext history key pair through the production native input route.
/// The caller already awaited its current frame; drawing while borrowing this update would
/// delay the canvas's original deferred event. Text and IME continue through their existing helpers.
pub(super) fn history_control_key(window: &mut Window, key: &str, cx: &mut App) {
    assert!(matches!(key, "home" | "end" | "enter"));
    // Dynamic notation follows the same host OS as this window's input route.
    let key = Keystroke::parse_with_os(key, cx.platform_info().os)
        .expect("the fixed history control key is valid");
    window.dispatch_event(
        PlatformInput::KeyDown(KeyDownEvent {
            keystroke: key.clone(),
            is_held: false,
            prefer_character_input: false,
        }),
        cx,
    );
    window.dispatch_event(PlatformInput::KeyUp(KeyUpEvent { keystroke: key }), cx);
}

pub(super) fn wheel(window: &mut Window, position: Point<Pixels>, y: f32, cx: &mut App) {
    window.dispatch_event(
        PlatformInput::Scroll(ScrollEvent {
            // Wheel and trackpad fixtures retain pointer-device semantics.
            source: gpui_kit::ScrollSource::Pointer,
            position,
            delta: ScrollDelta::Pixels(point(px(0.), px(y))),
            ..Default::default()
        }),
        cx,
    );
}

/// Read the real mounted/worker revisions for a click receipt without exposing a production API.
fn scene_revisions(harness: &Harness, cx: &App) -> serde_json::Value {
    let worker = harness
        .worker
        .state
        .lock()
        .unwrap()
        .views
        .iter()
        .map(|(key, scene)| (key.clone(), scene.revision))
        .collect::<std::collections::BTreeMap<_, _>>();
    let app = harness.app.read(cx);
    let panels = app
        .plugin_panels
        .iter()
        .map(|(key, panel)| {
            let panel = panel.read(cx);
            let scene = panel.current_document();
            let declared_source = scene
                .as_ref()
                .and_then(|scene| scene.active_native_editor());
            let current_source = declared_source.and_then(|expected| {
                (0..app.tabs.len())
                    .filter_map(|index| app.plugin_document_version(index).ok())
                    .find(|actual| actual.id == expected.id)
            });
            serde_json::json!({
                "panel": key,
                "host_revision": scene.as_ref().map(|scene| scene.revision),
                "worker_revision": worker.get(key),
                "declared_native_source": declared_source,
                "current_native_source": current_source
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!(panels)
}

impl Harness {
    /// Wait for the current publication before clicking the actual native button face. The final
    /// idle scene is not an operation receipt; callers must await their observable business effect.
    pub async fn click_button(
        &self,
        target: AnyWindowHandle,
        id: &str,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        self.settle(cx).await?;
        let mut previous = None;
        self.until(
            target,
            id,
            |window, cx| {
                let native = button(window, id).ok();
                let current = self.node_current(target, id, cx);
                let focus = native.as_ref().and_then(|node| node.focused());
                let bounds = native.as_ref().map(|node| node.bounds());
                let geometry = bounds.is_some_and(|bounds| {
                    bounds.size.width > px(0.)
                        && bounds.size.height > px(0.)
                        && Bounds::new(point(px(0.), px(0.)), window.viewport_size())
                            .contains(&bounds.center())
                });
                let disabled = native.as_ref().and_then(|node| node.disabled());
                let facts = serde_json::json!({
                    "id": id, "native_painted": native.is_some(),
                    "native_visible": native.as_ref().is_some_and(|node| node.visible()),
                    "native_disabled_metadata": disabled, "native_focus_metadata": focus,
                    "native_geometry_available": geometry,
                    "native_bounds": bounds.map(|bounds| format!("{bounds:?}")),
                    "guest_host_current": current
                });
                if previous.as_ref() != Some(&facts) {
                    let mut receipt = facts.clone();
                    receipt["scenes"] = scene_revisions(self, cx);
                    self.checkpoint(&format!("native_button_{id}_wait"), receipt)?;
                    previous = Some(facts.clone());
                }
                // Base Button 0.7 binds track_focus only while enabled. Observed::paint maps that
                // handle to Some(false) when unfocused; disabled controls have no binding (None).
                // The native form gate can disable a button whose guest Node remains enabled.
                if !current || !geometry || disabled == Some(true) || focus.is_none() {
                    return Ok(false);
                }
                let mut receipt = facts;
                receipt["scenes"] = scene_revisions(self, cx);
                self.checkpoint(&format!("native_button_{id}_before_click"), receipt)?;
                // Readiness, current geometry and dispatch share this one Window update. This
                // predicate succeeds after its only gesture and never replays an unaccepted click.
                click(window, native.as_ref().unwrap(), cx)?;
                window.draw(cx).clear(cx);
                let after = button(window, id).ok();
                self.checkpoint(
                    &format!("native_button_{id}_after_click"),
                    serde_json::json!({
                        "id": id, "native_painted": after.is_some(),
                        "native_focus_metadata": after.as_ref().and_then(|node| node.focused()),
                        "window_active": window.is_window_active(),
                        "scenes": scene_revisions(self, cx)
                    }),
                )?;
                Ok(true)
            },
            cx,
        )
        .await?;
        self.settle(cx).await
    }

    /// Expand with native double-click activation, keeping the normal Right-key fallback and focus.
    pub async fn expand(
        &self,
        target: AnyWindowHandle,
        tree: &str,
        label: &str,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        self.until(
            target,
            label,
            |window, cx| {
                let Ok(item) = tree_item(window, tree, label) else {
                    return Ok(false);
                };
                if item.disabled() == Some(true) {
                    return Ok(false);
                }
                if item.expanded() != Some(true) {
                    // Commit directories explicitly use single-click disclosure; a double click
                    // would toggle them twice and focus a different lazy row before the fallback.
                    if tree == "staging" {
                        click(window, &item, cx)?;
                    } else {
                        double_click(window, &item, cx)?;
                    }
                }
                Ok(true)
            },
            cx,
        )
        .await?;
        // Selection is one guest event and may publish a new UI scene. Finish that observable
        // step before pressing Right against its current native callbacks, as for other actions.
        self.settle(cx).await?;
        // Lazy pages can invalidate while Git is idle. Locate and dispatch in the same native
        // update, waiting for a missing row without reissuing any already dispatched input.
        self.until(
            target,
            label,
            |window, cx| {
                let Ok(item) = tree_item(window, tree, label) else {
                    return Ok(false);
                };
                if item.disabled() == Some(true) {
                    return Ok(false);
                }
                if item.expanded() != Some(true) {
                    editor_ui::testing::press_key(window, "right", cx);
                }
                Ok(true)
            },
            cx,
        )
        .await?;
        self.until(
            target,
            label,
            |window, _| {
                Ok(tree_item(window, tree, label).is_ok_and(|item| item.expanded() == Some(true)))
            },
            cx,
        )
        .await
    }

    pub async fn select_file(
        &self,
        target: AnyWindowHandle,
        tree: &str,
        file: &str,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        self.settle(cx).await?;
        self.until(
            target,
            file,
            |window, cx| {
                let Ok(item) = tree_item(window, tree, file) else {
                    return Ok(false);
                };
                if item.disabled() == Some(true) {
                    return Ok(false);
                }
                click(window, &item, cx)?;
                Ok(true)
            },
            cx,
        )
        .await?;
        self.settle(cx).await
    }
}
