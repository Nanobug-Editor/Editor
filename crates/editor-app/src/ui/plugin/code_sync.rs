//! Optional code-pane groups share semantic scroll anchors, never text, selection or editor state.
use super::*;
use crate::editor::viewport::{self as native_viewport, Anchor, Locator, Step};
use gpui_base::input::{EditorState, RopeExt as _};
use gpui_kit::{
    AnyElement, EntityId, InteractiveElement, ParentElement, Pixels, Point, Styled, div,
};
use plugin_runtime::plugin_protocol::api::DocumentVersion;
use std::ops::Range;

mod alignment;
#[cfg(test)]
mod alignment_tests;
mod authority;
mod mapping;
#[cfg(test)]
mod tests;

/// Each scene rebuild admits at most one existing 4M-cell comparison budget across all groups.
const MAX_CELLS: usize = 4_000_000;
const MAX_SAMPLE_ATTEMPTS: u8 = 48;
type Changes = Vec<[Range<usize>; 2]>;
type Scroll = (Entity<EditorState>, Point<Pixels>);

/// Only real native input can replace the driving pane; programmatic offsets never claim it.
#[derive(Default)]
struct Driver {
    pane: Cell<Option<usize>>,
    sequence: Cell<u64>,
}
impl Driver {
    fn claim(&self, pane: usize) {
        self.pane.set(Some(pane));
        self.sequence.set(self.sequence.get().wrapping_add(1));
    }
}

/// Text changes and native incarnation changes invalidate immutable maps independently of paint.
#[derive(Clone, PartialEq, Eq)]
struct Stamp {
    node: String,
    entity: Option<EntityId>,
    generation: u64,
    native: Option<DocumentVersion>,
}
struct Pane {
    stamp: Stamp,
    editor: Option<Entity<EditorState>>,
}

/// Mapping uses a common pivot, so three panes need two bounded diffs instead of all directed pairs.
struct Group {
    panes: Vec<Pane>,
    maps: Option<Vec<Changes>>,
    driver: Rc<Driver>,
    sequence: u64,
    anchor: Option<Anchor>,
    pending: Vec<Option<Locator>>,
    sample_attempts: u8,
    geometry_failed: bool,
    /// Optional zero-byte presentation; legacy 1.0 groups retain semantic scroll synchronization.
    alignment: Option<crate::ui::controls::aligned_editor::AlignmentGroup>,
    alignment_requested: bool,
}

impl Group {
    /// Failure is a visible independent-scroll fallback, never a guessed display-row mapping.
    fn new(panes: Vec<Pane>, budget: &mut usize, cx: &App) -> Self {
        let maps = (|| {
            let text = panes
                .iter()
                .map(|pane| {
                    let editor = pane.editor.as_ref().ok_or(())?;
                    let value = editor.read(cx);
                    if value.text().len() > 1024 * 1024 {
                        return Err(());
                    }
                    Ok(value.value().to_string())
                })
                .collect::<Result<Vec<_>, ()>>()?;
            let lines = text
                .iter()
                .map(|text| text.split_inclusive('\n').take(2001).count())
                .collect::<Vec<_>>();
            if lines.iter().any(|lines| *lines > 2000) {
                return Err(());
            }
            let cost = lines
                .iter()
                .skip(1)
                .map(|count| lines[0] * count)
                .sum::<usize>();
            if cost > *budget {
                return Err(());
            }
            *budget -= cost;
            text.iter()
                .skip(1)
                .map(|right| crate::editor::comparison::changed_byte_ranges(&text[0], right))
                .collect::<Result<Vec<_>, _>>()
        })()
        .ok();
        let pending = (0..panes.len()).map(|_| None).collect();
        Self {
            panes,
            maps,
            driver: Rc::new(Driver::default()),
            sequence: 0,
            anchor: None,
            pending,
            sample_attempts: 0,
            geometry_failed: false,
            alignment: None,
            alignment_requested: false,
        }
    }

    /// Existing byte maps are valid only while every native entity and content generation matches.
    fn same_panes(&self, panes: &[Pane]) -> bool {
        self.panes.len() == panes.len()
            && self
                .panes
                .iter()
                .zip(panes)
                .all(|(a, b)| a.stamp == b.stamp)
    }

    /// A driver offset maps through the common source, retaining exact unchanged byte intervals.
    fn target_offset(&self, driver: usize, target: usize, offset: usize) -> usize {
        let maps = self.maps.as_ref().expect("only admitted groups locate");
        let pivot = if driver == 0 {
            offset
        } else {
            mapping::offset(offset, 1, &maps[driver - 1])
        };
        if target == 0 {
            pivot
        } else {
            mapping::offset(pivot, 0, &maps[target - 1])
        }
    }

    fn unavailable(&self, cx: &App) -> bool {
        if self.alignment_requested {
            use crate::ui::controls::aligned_editor::AlignmentStatus;
            self.alignment
                .as_ref()
                .is_none_or(|group| matches!(group.status(cx), AlignmentStatus::Unavailable(_)))
        } else {
            self.maps.is_none() || self.geometry_failed
        }
    }
}

/// The only native editor reference is an existing host session supplied by its owning surface.
pub(super) struct CodeSync {
    source: Option<(DocumentVersion, Entity<EditorState>)>,
    groups: BTreeMap<String, Group>,
    generation: u64,
    /// Shared across live aligned groups, including rebuilds caused by native editing and IME.
    alignment_budget: Rc<Cell<usize>>,
    alignment_warning: Option<&'static str>,
}

impl Default for CodeSync {
    fn default() -> Self {
        Self {
            source: None,
            groups: BTreeMap::new(),
            generation: 0,
            alignment_budget: Rc::new(Cell::new(MAX_CELLS)),
            alignment_warning: None,
        }
    }
}

impl CodeSync {
    /// Dropping groups drops pending locators and old gesture handles before their next frame.
    pub(super) fn cancel(&mut self, cx: &mut App) {
        self.generation = self.generation.wrapping_add(1);
        for group in self.groups.values() {
            if let Some(alignment) = &group.alignment {
                alignment.retire(cx);
            }
        }
        self.groups.clear();
    }

    /// A stale guest scene loses actions immediately, while the original editable draft retains
    /// its geometry and focus until its owner actually unmounts or changes the native source.
    pub(super) fn cancel_gestures(&self) {
        for group in self.groups.values() {
            if let Some(alignment) = &group.alignment {
                for region in alignment.regions() {
                    region.request_cancel();
                }
            }
        }
    }
}

/// Walk only the displayed branch; inactive tabs and disabled parents cannot drive another pane.
fn active_nodes<'a>(node: &'a ui::Node, out: &mut Vec<&'a ui::Node>) {
    if node.disabled {
        return;
    }
    out.push(node);
    match &node.kind {
        Kind::Column { children } | Kind::Row { children } => {
            for child in children {
                active_nodes(child, out);
            }
        }
        Kind::Scroll { content } => active_nodes(content, out),
        Kind::Tabs { tabs, selected } => {
            if let Some(tab) = tabs.iter().find(|tab| tab.id == *selected) {
                active_nodes(&tab.content, out);
            }
        }
        Kind::EditorWithActions { left, right, .. } => {
            for action in left.iter().chain(right) {
                active_nodes(&action.content, out);
            }
        }
        _ => {}
    }
}

impl PluginView {
    /// Paint uses the source version prepared by its owner, which may still be updating while
    /// opening a window. Event dispatch separately rechecks the owner's authoritative version.
    pub(super) fn code_source_paint_current(&self, document: &DocumentVersion) -> bool {
        self.code_sync
            .source
            .as_ref()
            .is_some_and(|(source, _)| source == document)
    }

    /// Bind the already owned native session. Clearing/changing this binding revokes old locations;
    /// snapshots remain ordinary immutable native views and cannot acquire document edit authority.
    pub(crate) fn set_code_sync_source(
        &mut self,
        source: Option<(DocumentVersion, Entity<EditorState>)>,
        cx: &mut Context<Self>,
    ) {
        let identity = |source: &Option<(DocumentVersion, Entity<EditorState>)>| {
            source.as_ref().map(|(version, editor)| {
                (version.id.clone(), version.path.clone(), editor.entity_id())
            })
        };
        if identity(&self.code_sync.source) != identity(&source) {
            self.code_sync.cancel(cx);
            cx.notify();
        }
        self.code_sync.source = source;
    }

    /// Resolve active panes after native snapshot reconciliation. Pure maps are cached between
    /// unchanged paints, and changed scenes have an aggregate comparison allocation/CPU budget.
    pub(super) fn sync_code_groups(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.document.code_alignment
            && (!self.scene_current.get()
                || self.document.dialog.is_some()
                || self.document.menu.is_some())
        {
            self.code_sync.cancel(cx);
            return;
        }
        let mut nodes = Vec::new();
        active_nodes(&self.document.root, &mut nodes);
        let mut grouped = BTreeMap::<String, Vec<Pane>>::new();
        for node in nodes {
            let (group, editor, generation, native) = match &node.kind {
                Kind::ReadonlyCode {
                    sync_group: Some(group),
                    ..
                } => {
                    let snapshot = self.readonly_code.get(&node.id);
                    (
                        group,
                        snapshot.map(|snapshot| snapshot.editor.clone()),
                        snapshot.map_or(0, |snapshot| snapshot.generation),
                        None,
                    )
                }
                Kind::EditorWithActions {
                    document,
                    sync_group: Some(group),
                    ..
                } => {
                    let editor = self
                        .code_sync
                        .source
                        .as_ref()
                        .filter(|(source, _)| {
                            source == document
                                || (self.document.code_alignment
                                    && source.id == document.id
                                    && source.path == document.path
                                    && source.revision >= document.revision)
                        })
                        .map(|(_, editor)| editor.clone());
                    let revision = self
                        .code_sync
                        .source
                        .as_ref()
                        .map_or(document.revision, |(source, _)| source.revision);
                    (group, editor, revision, Some(document.clone()))
                }
                _ => continue,
            };
            let stamp = Stamp {
                node: node.id.clone(),
                entity: editor.as_ref().map(Entity::entity_id),
                generation,
                native,
            };
            grouped
                .entry(group.clone())
                .or_default()
                .push(Pane { stamp, editor });
        }
        if self.document.code_alignment {
            self.sync_aligned_code_groups(grouped, window, cx);
            return;
        }
        self.code_sync
            .groups
            .retain(|id, _| grouped.get(id).is_some_and(|panes| panes.len() >= 2));
        let mut budget = MAX_CELLS;
        for (id, panes) in grouped {
            if panes.len() < 2 {
                continue;
            }
            if self
                .code_sync
                .groups
                .get(&id)
                .is_some_and(|group| group.same_panes(&panes))
            {
                continue;
            }
            self.code_sync
                .groups
                .insert(id, Group::new(panes, &mut budget, cx));
        }
    }

    /// Capture existing wheel/scrollbar/key input without consuming it or manufacturing a second
    /// editor. Group-local handles are replaced on scene retirement, rejecting retained callbacks.
    pub(super) fn code_sync_pane(&self, id: &str, source: AnyElement) -> AnyElement {
        let Some((group, index)) = self.code_sync.groups.values().find_map(|group| {
            group.maps.as_ref()?;
            group
                .panes
                .iter()
                .position(|pane| pane.stamp.node == id)
                .map(|index| (group, index))
        }) else {
            return source;
        };
        let driver = group.driver.clone();
        let keyboard = driver.clone();
        div()
            .relative()
            .flex()
            .size_full()
            .min_h_0()
            .min_w_0()
            .capture_key_down(move |_, _, _| keyboard.claim(index))
            .child(source)
            .child(
                gpui_kit::canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        let wheel = driver.clone();
                        window.on_mouse_event(
                            move |event: &gpui_kit::ScrollWheelEvent, phase, _, _| {
                                if phase.capture() && bounds.contains(&event.position) {
                                    wheel.claim(index);
                                }
                            },
                        );
                        let pointer = driver.clone();
                        window.on_mouse_event(
                            move |event: &gpui_kit::MouseDownEvent, phase, _, _| {
                                if phase.capture() && bounds.contains(&event.position) {
                                    pointer.claim(index);
                                }
                            },
                        );
                    },
                )
                .absolute()
                .inset_0()
                .size_full(),
            )
            .into_any_element()
    }

    /// The final paint probe runs after every source prepaint, regardless of pane/container order.
    /// Native hit testing needs the paint phase. Only authority reads and resulting scrolls are
    /// deferred until outer owners finish updating, including a parent opening this new window.
    pub(super) fn code_sync_probe(&self, cx: &Context<Self>) -> AnyElement {
        if self.code_sync.groups.is_empty() {
            return div().into_any_element();
        }
        let owner = cx.entity().downgrade();
        let generation = self.code_sync.generation;
        let revision = self.document.revision;
        let source = self
            .code_sync
            .source
            .as_ref()
            .map(|(source, _)| source.clone());
        let warning = self.code_alignment_warning(cx).or_else(|| {
            self.code_sync
                .groups
                .values()
                .any(|group| group.unavailable(cx))
                .then_some("editor.code_sync_unavailable")
        });
        use gpui_kit::prelude::FluentBuilder as _;
        div()
            .absolute()
            .inset_0()
            .size_full()
            .child(
                gpui_kit::canvas(
                    |_, _, _| (),
                    move |_, _, window, cx| {
                        let updates = owner
                            .update(cx, |view, cx| {
                                view.step_code_groups(generation, revision, window, cx)
                            })
                            .unwrap_or_default();
                        // GPUI associates animation requests with the current painted view.
                        // Queue the follow-up here; the deferred scroll still validates authority.
                        if !updates.is_empty() {
                            window.request_animation_frame();
                        }
                        window.defer(cx, move |_, cx| {
                            let allowed = owner
                                .update(cx, |view, cx| {
                                    view.code_scroll_authorized(generation, revision, &source, cx)
                                })
                                .unwrap_or(false);
                            if !allowed {
                                return;
                            }
                            for (editor, offset) in updates {
                                let current = owner
                                    .update(cx, |view, cx| {
                                        view.code_scroll_authorized(
                                            generation, revision, &source, cx,
                                        )
                                    })
                                    .unwrap_or(false);
                                if !current {
                                    break;
                                }
                                editor
                                    .update(cx, |editor, cx| editor.set_scroll_offset(offset, cx));
                            }
                        });
                    },
                )
                .absolute()
                .inset_0()
                .size_full(),
            )
            .when_some(warning, |view, warning| {
                view.child(
                    div()
                        .absolute()
                        .right_0()
                        .bottom_0()
                        .px_2()
                        .py_1()
                        .child(rust_i18n::t!(warning).to_string()),
                )
            })
            .into_any_element()
    }

    /// Prepare display-only scroll proposals. The deferred application checks exact authority.
    /// Each locator has its own 48-frame bound;
    /// actual native geometry handles wrapping/folds/clamps and never changes selection or focus.
    fn step_code_groups(
        &mut self,
        generation: u64,
        revision: u64,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Vec<Scroll> {
        if generation != self.code_sync.generation
            || revision != self.document.revision
            || !self.scene_current.get()
        {
            return Vec::new();
        }
        let mut updates = Vec::new();
        let warning = self.code_alignment_warning(cx);
        let mut notify = warning != self.code_sync.alignment_warning;
        self.code_sync.alignment_warning = warning;
        for group in self.code_sync.groups.values_mut() {
            if group.maps.is_none() {
                continue;
            }
            let Some(driver) = group
                .driver
                .pane
                .get()
                .filter(|driver| *driver < group.panes.len())
            else {
                continue;
            };
            let sequence = group.driver.sequence.get();
            if group.sequence != sequence {
                group.sequence = sequence;
                group.anchor = None;
                group.geometry_failed = false;
                group.sample_attempts = 0;
                group.pending.iter_mut().for_each(|pending| *pending = None);
            }
            if group.geometry_failed {
                continue;
            }
            let source = group.panes[driver]
                .editor
                .as_ref()
                .expect("mapped panes are available");
            let Some(anchor) = native_viewport::sample(source.read(cx), window, cx) else {
                group.sample_attempts += 1;
                if group.sample_attempts >= MAX_SAMPLE_ATTEMPTS {
                    group.geometry_failed = true;
                }
                notify = true;
                continue;
            };
            group.sample_attempts = 0;
            if group.anchor != Some(anchor) {
                for target in 0..group.panes.len() {
                    if target == driver {
                        continue;
                    }
                    let offset = group.target_offset(driver, target, anchor.offset);
                    let editor = group.panes[target].editor.as_ref().unwrap().read(cx);
                    let mut offset = offset.min(editor.text().len());
                    while !editor.text().is_char_boundary(offset) {
                        offset -= 1;
                    }
                    if offset > 0
                        && editor.text().char_at(offset - 1) == Some('\r')
                        && editor.text().char_at(offset) == Some('\n')
                    {
                        offset -= 1;
                    }
                    group.pending[target] = Some(Locator::new(offset, anchor.line_fraction));
                }
                group.anchor = Some(anchor);
            }
            let start = updates.len();
            for (pane, pending) in group.panes.iter().zip(&mut group.pending) {
                let Some(locator) = pending else {
                    continue;
                };
                let editor = pane.editor.as_ref().unwrap();
                match locator.step(editor.read(cx)) {
                    Step::Scroll(offset) => {
                        updates.push((editor.clone(), offset));
                        notify = true;
                    }
                    Step::Settled => {
                        *pending = None;
                    }
                    Step::Failed => {
                        group.geometry_failed = true;
                        notify = true;
                        break;
                    }
                }
            }
            if group.geometry_failed {
                updates.truncate(start);
                group.pending.iter_mut().for_each(|pending| *pending = None);
            }
        }
        if notify {
            cx.notify();
        }
        updates
    }
}
