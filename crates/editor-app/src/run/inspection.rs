//! Explicit-thread inspection retains only bounded readonly pages for one session and pause.
use super::*;
use editor_core::{InspectionError, PauseScope, StackFrame};
use plugin_protocol::debug::{Threads, Variables};
use plugin_runtime::plugin_protocol;
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// Immutable response variants are decoded at the actor boundary, then joined to their own request.
#[derive(Clone, Debug)]
pub(crate) enum Answer {
    Threads(Threads),
    Frames(Vec<StackFrame>),
    Variables(Variables),
}

/// One visible thread from the selected session's current pause; selection requests are revalidated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreadRow {
    pub id: u32,
    pub label: String,
    pub selected: bool,
}

/// Paths distinguish two visible occurrences of a cyclic object without recursively expanding both.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VariableAction {
    Toggle { path: Vec<usize>, reference: u32 },
    More { reference: u32, start: u32 },
}

/// This is provider inspection data, never a document text buffer or an independent target session.
#[derive(Clone, Debug)]
pub(super) struct State {
    session: String,
    epoch: u64,
    scope: PauseScope,
    threads: Option<Threads>,
    selected: Option<u32>,
    frames: Option<Vec<StackFrame>>,
    frame: Option<u32>,
    pages: BTreeMap<(u32, u32), Variables>,
    expanded: BTreeSet<(u32, Vec<usize>)>,
    pub(super) failed: bool,
}
impl State {
    fn new(session: String, epoch: u64, scope: PauseScope) -> Self {
        Self {
            session,
            epoch,
            scope,
            threads: None,
            selected: None,
            frames: None,
            frame: None,
            pages: BTreeMap::new(),
            expanded: BTreeSet::new(),
            failed: false,
        }
    }
    fn select_thread(&mut self, thread: u32) -> Result<(), InspectionError> {
        if !self
            .threads
            .as_ref()
            .is_some_and(|threads| threads.threads.iter().any(|value| value.id == thread))
        {
            return Err(InspectionError::Provider(
                "Unknown thread in this pause".into(),
            ));
        }
        if self.selected != Some(thread) {
            self.selected = Some(thread);
            self.frames = None;
            self.frame = None;
            self.pages.clear();
            self.expanded.clear();
        }
        self.failed = false;
        Ok(())
    }
    /// Resolve a visible path against already admitted pages; no arbitrary reference is accepted.
    fn node(&self, frame: u32, path: &[usize]) -> Option<&plugin_protocol::debug::Variable> {
        let (first, rest) = path.split_first()?;
        let mut node = self.pages.get(&(frame, 0))?.variables.get(*first)?;
        for index in rest {
            node = self
                .pages
                .get(&(frame, node.reference))?
                .variables
                .get(*index)?;
        }
        Some(node)
    }
    fn rows(&self) -> Vec<DebugVariableRow> {
        let mut rows = Vec::new();
        if let Some(frame) = self.frame {
            self.append_rows(frame, 0, &mut Vec::new(), &mut rows);
        }
        rows
    }
    /// A depth/row budget also bounds cyclic structures; each level requires a separate user action.
    fn append_rows(
        &self,
        frame: u32,
        reference: u32,
        path: &mut Vec<usize>,
        rows: &mut Vec<DebugVariableRow>,
    ) {
        if path.len() >= 32 || rows.len() >= 4096 {
            return;
        }
        let Some(page) = self.pages.get(&(frame, reference)) else {
            return;
        };
        for (index, node) in page.variables.iter().enumerate() {
            if rows.len() >= 4096 {
                break;
            }
            path.push(index);
            let expanded = self.expanded.contains(&(frame, path.clone()));
            let prefix = if node.reference == 0 {
                ""
            } else if expanded {
                "▾ "
            } else {
                "▸ "
            };
            rows.push(DebugVariableRow {
                selector: format!(
                    "debug-variable-{}",
                    path.iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join("-")
                ),
                label: format!("{prefix}{} = {}", node.name, node.value),
                depth: path.len() - 1,
                action: (node.reference != 0).then(|| VariableAction::Toggle {
                    path: path.clone(),
                    reference: node.reference,
                }),
            });
            if expanded {
                self.append_rows(frame, node.reference, path, rows);
            }
            path.pop();
        }
        if page.more {
            rows.push(DebugVariableRow {
                selector: format!("debug-variable-more-{reference}"),
                label: t!("run.debug_load_more").to_string(),
                depth: path.len(),
                action: Some(VariableAction::More {
                    reference,
                    start: page.variables.len() as u32,
                }),
            });
        }
    }
}

impl RunControls {
    /// All access is filtered through the selected real session's current pause identity.
    fn tree_context(&self) -> Option<(String, String, u64, PauseScope)> {
        if !self.selected_debug_capabilities().thread_tree
            || !matches!(
                self.debug_state(),
                editor_core::DebugSessionState::Paused { .. }
            )
        {
            return None;
        }
        Some((
            self.debug_sessions.current()?.0.to_owned(),
            self.debug_provider_session()?,
            self.debug_pause_epoch()?,
            self.debug_pause_scope()?,
        ))
    }
    fn tree(&self) -> Option<&State> {
        let (config, session, epoch, scope) = self.tree_context()?;
        self.inspections.get(&config).filter(|state| {
            state.session == session && state.epoch == epoch && state.scope == scope
        })
    }
    fn tree_mut(&mut self) -> Option<&mut State> {
        let (config, session, epoch, scope) = self.tree_context()?;
        let state = self
            .inspections
            .entry(config)
            .or_insert_with(|| State::new(session.clone(), epoch, scope));
        if state.session != session || state.epoch != epoch || state.scope != scope {
            *state = State::new(session, epoch, scope);
        }
        Some(state)
    }
    pub(super) fn tree_frames(&self) -> &[StackFrame] {
        self.tree()
            .and_then(|state| state.frames.as_deref())
            .unwrap_or_default()
    }
    pub(super) fn tree_selected_frame(&self) -> Option<u32> {
        self.tree()?.frame
    }
    pub(super) fn tree_location(&self) -> Option<(&str, u32)> {
        let state = self.tree()?;
        let frame = state
            .frames
            .as_ref()?
            .iter()
            .find(|frame| Some(frame.id) == state.frame)?;
        Some((&frame.source, frame.line))
    }
    pub(super) fn select_tree_frame(&mut self, frame: u32) -> Result<(), InspectionError> {
        let state = self.tree_mut().ok_or(InspectionError::NotPaused)?;
        if !state
            .frames
            .as_ref()
            .is_some_and(|frames| frames.iter().any(|value| value.id == frame))
        {
            return Err(InspectionError::NoSuchFrame { frame });
        }
        state.frame = Some(frame);
        state.failed = false;
        Ok(())
    }
    /// Changing the selected thread clears its projected stack and never reuses another thread's IDs.
    pub(crate) fn select_debug_thread(&mut self, thread: u32) -> Result<(), InspectionError> {
        self.tree_mut()
            .ok_or(InspectionError::NotPaused)?
            .select_thread(thread)
    }
    pub(super) fn tree_thread_rows(&self) -> Vec<ThreadRow> {
        self.tree()
            .and_then(|state| {
                state.threads.as_ref().map(|threads| {
                    threads
                        .threads
                        .iter()
                        .map(|thread| ThreadRow {
                            id: thread.id,
                            label: thread.name.clone(),
                            selected: Some(thread.id) == state.selected,
                        })
                        .collect()
                })
            })
            .unwrap_or_default()
    }
    pub(super) fn tree_variable_rows(&self) -> Vec<DebugVariableRow> {
        self.tree().map(State::rows).unwrap_or_default()
    }

    /// Determine the next missing description. Existing request coalescing retains the full key.
    pub(crate) fn next_tree_request(
        &mut self,
    ) -> Option<(DebugMethod, &'static str, Value, Option<u32>)> {
        let state = self.tree_mut()?;
        if state.failed {
            return None;
        }
        let mut args = json!({"session":state.session,"pause":state.epoch});
        if state.threads.is_none() {
            return Some((DebugMethod::Threads, "threads", args, None));
        }
        let thread = state.selected?;
        args["thread"] = json!(thread);
        if state.frames.is_none() {
            return Some((
                DebugMethod::ThreadFrames(thread),
                "thread_frames",
                args,
                None,
            ));
        }
        let frame = state.frame?;
        let reference = if !state.pages.contains_key(&(frame, 0)) {
            0
        } else {
            state
                .expanded
                .iter()
                .filter(|(owner, _)| *owner == frame)
                .filter_map(|(_, path)| state.node(frame, path))
                .map(|node| node.reference)
                .find(|reference| !state.pages.contains_key(&(frame, *reference)))?
        };
        args["frame"] = json!(frame);
        args["reference"] = json!(reference);
        args["start"] = json!(0);
        args["count"] = json!(100);
        Some((
            DebugMethod::Children {
                thread,
                frame,
                reference,
                start: 0,
            },
            "variable_children",
            args,
            Some(frame),
        ))
    }
    /// Toggle a single visible occurrence, or explicitly ask for the next bounded child page.
    pub(crate) fn act_debug_variable(
        &mut self,
        action: VariableAction,
    ) -> Result<Option<(DebugMethod, Value, u32)>, InspectionError> {
        let state = self.tree_mut().ok_or(InspectionError::NotPaused)?;
        let frame = state.frame.ok_or(InspectionError::NotPaused)?;
        state.failed = false;
        match action {
            VariableAction::Toggle { path, reference } => {
                if path.len() >= 32
                    || state
                        .node(frame, &path)
                        .is_none_or(|node| node.reference != reference)
                {
                    return Err(InspectionError::StalePause);
                }
                let key = (frame, path);
                if !state.expanded.remove(&key) {
                    state.expanded.insert(key);
                }
                Ok(None)
            }
            VariableAction::More { reference, start } => {
                if !state
                    .pages
                    .get(&(frame, reference))
                    .is_some_and(|page| page.more && page.variables.len() == start as usize)
                {
                    return Err(InspectionError::StalePause);
                }
                let thread = state.selected.ok_or(InspectionError::NotPaused)?;
                Ok(Some((
                    DebugMethod::Children {
                        thread,
                        frame,
                        reference,
                        start,
                    },
                    json!({"session":state.session,"pause":state.epoch,"thread":thread,"frame":frame,"reference":reference,"start":start,"count":100}),
                    frame,
                )))
            }
        }
    }

    /// Admit only the request's original configuration, pause, selected thread and selected frame.
    pub(crate) fn apply_tree_answer(
        &mut self,
        request: u64,
        answer: Answer,
    ) -> Result<(), InspectionError> {
        let index = self
            .debug_requests
            .iter()
            .position(|pending| pending.id == request)
            .ok_or(InspectionError::NoSession)?;
        let pending = self.debug_requests.remove(index);
        if pending.config.as_deref() != self.debug_sessions.selected() {
            return Err(InspectionError::WrongSession);
        }
        let state = self.tree_mut().ok_or(InspectionError::NotPaused)?;
        if state.scope != pending.scope {
            return Err(InspectionError::StalePause);
        }
        match (pending.method, answer) {
            (DebugMethod::Threads, Answer::Threads(threads)) => {
                state.selected = threads
                    .threads
                    .iter()
                    .find(|thread| thread.id == threads.selected)
                    .or_else(|| threads.threads.first())
                    .map(|thread| thread.id);
                state.threads = Some(threads);
            }
            (DebugMethod::ThreadFrames(thread), Answer::Frames(frames))
                if state.selected == Some(thread) =>
            {
                state.frame = frames.first().map(|frame| frame.id);
                state.frames = Some(frames);
                state.pages.clear();
                state.expanded.clear();
            }
            (
                DebugMethod::Children {
                    thread,
                    frame,
                    reference,
                    start,
                },
                Answer::Variables(mut page),
            ) if state.selected == Some(thread) && state.frame == Some(frame) => {
                let count: usize = state.pages.values().map(|page| page.variables.len()).sum();
                let bytes: usize = state
                    .pages
                    .values()
                    .flat_map(|page| &page.variables)
                    .map(|node| node.name.len() + node.value.len())
                    .sum();
                if state.pages.len() >= 128
                    || count + page.variables.len() > 4096
                    || bytes
                        + page
                            .variables
                            .iter()
                            .map(|node| node.name.len() + node.value.len())
                            .sum::<usize>()
                        > 4 * 1024 * 1024
                {
                    state.failed = true;
                    return Err(InspectionError::Provider(
                        t!("run.debug_inspection_limit").to_string(),
                    ));
                }
                if start == 0 {
                    state.pages.insert((frame, reference), page);
                } else {
                    let previous = state
                        .pages
                        .get_mut(&(frame, reference))
                        .filter(|page| page.variables.len() == start as usize)
                        .ok_or(InspectionError::StalePause)?;
                    previous.variables.append(&mut page.variables);
                    previous.more = page.more;
                }
            }
            _ => return Err(InspectionError::StalePause),
        }
        Ok(())
    }
}
