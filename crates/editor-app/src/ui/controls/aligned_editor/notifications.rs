//! Bound the extra observer relay to notifications not already represented by the authority paint.
use super::*;
use gpui_kit::Point;
use std::cell::RefCell;

/// Public current state only; content identity remains the group's bounded immutable COW snapshot.
/// Native private state (blink, syntax, folds, IME underlines) is never cached or replaced here.
#[derive(PartialEq)]
pub(super) struct PaintedNative {
    selection: Range<usize>,
    cursor: usize,
    scroll: Point<Pixels>,
    input: Bounds<Pixels>,
    text: Option<Bounds<Pixels>>,
    caret: Option<(Bounds<Pixels>, Pixels)>,
    visible: Option<Range<usize>>,
    presentation: (bool, bool, bool),
    search_open: bool,
    completion: (bool, u64),
    code_actions: (bool, u64),
}

impl PaintedNative {
    pub fn read(editor: &EditorState) -> Self {
        let presentation = editor.presentation();
        let completion = editor.completion_menu_state();
        let actions = editor.code_action_menu_state();
        Self {
            selection: editor.selected_range(),
            cursor: editor.cursor(),
            scroll: editor.scroll_offset(),
            input: editor.input_bounds(),
            text: editor.text_bounds(),
            caret: editor.cursor_layout(),
            visible: editor.visible_row_range(),
            presentation: (
                presentation.is_readonly(),
                presentation.is_disabled(),
                presentation.is_loading(),
            ),
            search_open: editor.search_session().open,
            completion: (completion.open, completion.revision()),
            code_actions: (actions.open, actions.revision()),
        }
    }
}

#[derive(Default)]
struct Pending {
    serial: u64,
    painted: Option<PaintedNative>,
}

/// GPUI suppresses paint-time notifications for the drawing window, but another window that
/// merely reads this native entity can still enqueue its Notify effect. Relaying that effect
/// back to this PaneView would repaint all slices forever despite identical final geometry.
#[derive(Clone, Default)]
pub(super) struct AuthorityFence(Rc<RefCell<Pending>>);

impl AuthorityFence {
    pub fn completed(&self, painted: PaintedNative, cx: &mut App) {
        let serial = {
            let mut pending = self.0.borrow_mut();
            pending.serial = pending.serial.wrapping_add(1);
            pending.painted = Some(painted);
            pending.serial
        };
        let pending = self.0.clone();
        // The native paint enqueued Notify before this canvas. FIFO effect processing consumes
        // it before clearing this fence; no persistent equality filter can hide a later blink,
        // highlight/fold update, unmark, scroll or input notification with unchanged public fields.
        cx.defer(move |_| {
            let mut pending = pending.borrow_mut();
            if pending.serial == serial {
                pending.painted = None;
            }
        });
    }

    pub fn should_relay(&self, editor: &EditorState) -> bool {
        self.0
            .borrow()
            .painted
            .as_ref()
            .is_none_or(|painted| *painted != PaintedNative::read(editor))
    }
}
