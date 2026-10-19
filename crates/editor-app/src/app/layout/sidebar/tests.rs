//! Structural migration preserves measured widths and nested peer layouts without a second live tree.
use super::*;

/// Build persisted splits exactly as Base serializes them, including vertical peer preferences.
fn split(axis: Axis, children: Vec<PanelState>, sizes: &[f32]) -> PanelState {
    PanelState {
        panel_name: "StackPanel".into(),
        children,
        info: PanelInfo::stack(sizes.iter().copied().map(px).collect(), axis),
    }
}

/// Legacy left peers become one fixed sidebar while unrelated regions remain byte-for-byte equivalent.
#[test]
fn sidebar_migration_retains_width_peers_and_other_regions() {
    let sidebar = split(
        Axis::Vertical,
        vec![PanelState::new("Explorer"), PanelState::new("Outline")],
        &[420., 180.],
    );
    let right = DockState::new(
        PanelState::new("HostMessages"),
        DockPlacement::Right,
        px(319.),
        false,
    );
    let mut layout = DockAreaState {
        center: split(
            Axis::Horizontal,
            vec![sidebar.clone(), PanelState::new("Editor")],
            &[287., 674.],
        ),
        right_dock: Some(right.clone()),
        bottom_dock: Some(DockState::new(
            PanelState::new("NativeTerminal"),
            DockPlacement::Bottom,
            px(201.),
            true,
        )),
        ..Default::default()
    };
    let bottom = layout.bottom_dock.clone();
    assert!(separate_left(&mut layout, px(280.)));
    assert_eq!(layout.center, PanelState::new("Editor"));
    let left = layout.left_dock.as_ref().unwrap();
    assert_eq!(left.panel(), &sidebar);
    assert_eq!(left.size(), px(287.));
    assert_eq!(layout.right_dock, Some(right));
    assert_eq!(layout.bottom_dock, bottom);
    let migrated = layout.clone();
    assert!(!separate_left(&mut layout, px(280.)));
    assert_eq!(layout, migrated);
}

/// A customized outer column keeps its order and width alongside the migrated shared sidebar.
#[test]
fn sidebar_migration_retains_custom_outer_column_and_ignores_empty_dock() {
    for occupied in [false, true] {
        let outer = if occupied {
            PanelState::new("CustomLeft")
        } else {
            PanelState {
                panel_name: "TabPanel".into(),
                info: PanelInfo::tabs(0),
                ..Default::default()
            }
        };
        let mut layout = DockAreaState {
            center: split(
                Axis::Horizontal,
                vec![PanelState::new("Explorer"), PanelState::new("Editor")],
                &[271., 760.],
            ),
            left_dock: Some(DockState::new(
                outer.clone(),
                DockPlacement::Left,
                px(233.),
                true,
            )),
            ..Default::default()
        };
        assert!(separate_left(&mut layout, px(280.)));
        let left = layout.left_dock.unwrap();
        assert_eq!(left.size(), px(if occupied { 504. } else { 271. }));
        if occupied {
            assert_eq!(
                left.panel().children,
                vec![outer, PanelState::new("Explorer")]
            );
        } else {
            assert_eq!(left.panel(), &PanelState::new("Explorer"));
        }
    }
}

/// Joining a closed old column preserves effective visibility and avoids reserving its hidden width.
#[test]
fn sidebar_migration_keeps_closed_outer_panels_hidden() {
    let mut session = SessionState::for_workspace(std::path::Path::new("C:/sidebar-fixture"));
    // The synthetic center-side tree predates editor-local rows and needs the legacy conversion.
    session.plugin_sidebar_layout_version = 1;
    session.messages_visible = true;
    session
        .plugin_panel_visibility
        .insert("fixture/left".into(), true);
    session.dock_layout = Some(DockAreaState {
        center: split(
            Axis::Horizontal,
            vec![PanelState::new("Explorer"), PanelState::new("Editor")],
            &[271., 760.],
        ),
        left_dock: Some(DockState::new(
            split(
                Axis::Vertical,
                vec![
                    PanelState::new("HostMessages"),
                    PanelState::new("plugin:fixture/left"),
                ],
                &[200., 300.],
            ),
            DockPlacement::Left,
            px(233.),
            false,
        )),
        ..Default::default()
    });
    EditorApp::prepare_sidebar_visibility(&mut session);
    assert!(!session.messages_visible);
    assert_eq!(session.plugin_panel_visibility["fixture/left"], false);
    assert!(session.explorer_visible);
    let layout = session.dock_layout.as_mut().unwrap();
    assert!(separate_left(layout, px(280.)));
    assert_eq!(layout.left_dock.as_ref().unwrap().size(), px(271.));
}

/// All historical nested left columns must leave Center, retaining their outer-to-inner order.
#[test]
fn sidebar_migration_promotes_nested_left_columns() {
    let mut layout = DockAreaState {
        center: split(
            Axis::Horizontal,
            vec![
                PanelState::new("Explorer"),
                split(
                    Axis::Horizontal,
                    vec![PanelState::new("CustomLeft"), PanelState::new("Editor")],
                    &[190., 700.],
                ),
            ],
            &[280., 890.],
        ),
        ..Default::default()
    };
    assert!(separate_left(&mut layout, px(280.)));
    assert_eq!(layout.center, PanelState::new("Editor"));
    let left = layout.left_dock.unwrap();
    assert_eq!(left.size(), px(470.));
    assert_eq!(
        left.panel().children,
        vec![PanelState::new("Explorer"), PanelState::new("CustomLeft")]
    );
}
