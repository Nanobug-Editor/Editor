//! Incremental plugin parsers and bounded injection layers; Base remains the only editable text owner.
use super::registry::{Entry, LanguageRegistry};
use super::source::{Text, read};
use gpui_base::input::{HighlightStyleResolver, Rope};
use gpui_kit::{HighlightStyle, SharedString};
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::{ControlFlow, Range},
    time::{Duration, Instant},
};
use tree_sitter::{
    InputEdit, ParseOptions, Parser, Query, QueryCursor, QueryCursorOptions,
    StreamingIterator as _, Tree,
};

/// Parser state for a single plugin grammar and one immutable source-coordinate region.
struct Layer {
    /// Actual injection depth decides capture precedence independently of sibling ordering.
    depth: usize,
    /// Qualified provider identity, never a display language fallback.
    language: String,
    /// Publication incarnation that created this parser.
    generation: u64,
    /// Isolated WASM parser retained across edits.
    parser: Parser,
    /// Highlight query from the same validated package.
    query: Query,
    /// Optional static injection query from that package.
    injections: Option<Query>,
    /// Allowed qualified injection targets.
    allowed: Vec<SharedString>,
    /// Current parsed tree, with incremental edits applied before reparsing.
    tree: Option<Tree>,
    /// Included physical ranges; empty means the complete source.
    ranges: Vec<tree_sitter::Range>,
}
impl Layer {
    /// Build parser/query state only from an explicitly registered plugin factory.
    fn new(language: &str, entry: Entry) -> Option<Self> {
        if entry.config.highlights.is_empty() {
            return None;
        }
        let (mut parser, grammar) = entry.factory?().ok()?;
        parser.set_language(&grammar).ok()?;
        let query = Query::new(&grammar, &entry.config.highlights).ok()?;
        let injections = (!entry.config.injections.is_empty())
            .then(|| Query::new(&grammar, &entry.config.injections).ok())
            .flatten();
        Some(Self {
            depth: 0,
            language: language.into(),
            generation: entry.generation,
            parser,
            query,
            injections,
            allowed: entry.config.injection_languages,
            tree: None,
            ranges: Vec::new(),
        })
    }
    /// Parse an immutable UTF-8 snapshot, respecting the entire update's deadline.
    fn parse(
        &mut self,
        source: &Rope,
        edit: Option<&InputEdit>,
        deadline: Option<Instant>,
    ) -> bool {
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return false;
        }
        if let Some(edit) = edit {
            if let Some(tree) = &mut self.tree {
                tree.edit(edit);
            }
        }
        let mut progress = |_: &tree_sitter::ParseState| {
            if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        };
        let result = self.parser.parse_with_options(
            &mut |offset, _| read(source, offset),
            // Without an edit, callers supplied a whole snapshot. Reusing an unedited tree
            // would let Tree-sitter return the previous source's diagnostics unchanged.
            edit.and(self.tree.as_ref()),
            Some(ParseOptions::new().progress_callback(&mut progress)),
        );
        if let Some(tree) = result {
            self.tree = Some(tree);
            true
        } else {
            false
        }
    }
}

/// Native highlighter over immutable snapshots; all mutations and Undo remain in DocumentSession.
pub(crate) struct SyntaxHighlighter {
    /// Qualified selected grammar or inert text identity.
    language: SharedString,
    /// Current primary incremental parser, absent for plain text.
    root: Option<Layer>,
    /// Retained injection parser stores, bounded independently of source size.
    layers: Vec<Layer>,
    /// Read-only source snapshot used by query predicates and UTF-8 clipping.
    source: Rope,
}
impl SyntaxHighlighter {
    /// Create an inert reader without loading WASM or compiling queries on the UI thread.
    pub(crate) fn deferred(language: &str) -> Self {
        Self {
            language: language.to_owned().into(),
            root: None,
            layers: Vec::new(),
            source: Rope::new(),
        }
    }

    /// Only an already prepared current grammar may enter the foreground parse budget.
    pub(crate) fn is_prepared(&self) -> bool {
        let current = LanguageRegistry::singleton().entry(&self.language);
        match current {
            None => self.root.is_none(),
            Some(entry) if entry.factory.is_none() || entry.config.highlights.is_empty() => true,
            Some(entry) => self
                .root
                .as_ref()
                .is_some_and(|root| root.generation == entry.generation),
        }
    }

    /// Prepare the selected grammar on a background worker before starting its parse deadline.
    pub(crate) fn prepare(&mut self) {
        let current = LanguageRegistry::singleton().entry(&self.language);
        if self.root.as_ref().map(|layer| layer.generation)
            != current.as_ref().map(|entry| entry.generation)
        {
            self.root = current.and_then(|entry| Layer::new(&self.language, entry));
            self.layers.clear();
        }
    }

    /// Unknown, disabled or failed plugin languages are inert and never select a native grammar.
    pub(crate) fn new(language: &str) -> Self {
        Self {
            language: language.to_owned().into(),
            root: LanguageRegistry::singleton()
                .entry(language)
                .and_then(|entry| Layer::new(language, entry)),
            layers: Vec::new(),
            source: Rope::new(),
        }
    }
    /// The selected qualified parser identity, retained even while unavailable.
    pub(crate) fn language(&self) -> &SharedString {
        &self.language
    }
    /// Borrow the primary tree for diagnostics and generic grammar fold candidates.
    pub(crate) fn tree(&self) -> Option<&Tree> {
        self.root.as_ref()?.tree.as_ref()
    }
    /// Incrementally parse `text`; false leaves resumable state for the caller's cancellation gate.
    /// `timeout` covers the primary parse and all nested layers together.
    pub(crate) fn update(
        &mut self,
        edit: Option<InputEdit>,
        text: &Rope,
        timeout: Option<Duration>,
    ) -> bool {
        let deadline = timeout.map(|timeout| Instant::now() + timeout);
        self.source = text.clone();
        let current = LanguageRegistry::singleton().entry(&self.language);
        if self.root.as_ref().map(|layer| layer.generation)
            != current.as_ref().map(|entry| entry.generation)
        {
            self.root = current.and_then(|entry| Layer::new(&self.language, entry));
            self.layers.clear();
        }
        let Some(root) = &mut self.root else {
            return true;
        };
        if !root.parse(&self.source, edit.as_ref(), deadline) {
            return false;
        }
        let Some(requests) = injection_requests(root, &self.source, deadline) else {
            return false;
        };
        let mut pending = requests
            .into_iter()
            .map(|request| (request, 1))
            .collect::<std::collections::VecDeque<_>>();
        let mut old = std::mem::take(&mut self.layers);
        let mut layers = Vec::new();
        let mut injected_bytes = 0usize;
        while let Some(((language, ranges), depth)) = pending.pop_front() {
            if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                return false;
            }
            if depth > 8 || layers.len() == 128 {
                continue;
            }
            injected_bytes = injected_bytes.saturating_add(
                ranges
                    .iter()
                    .map(|range| range.end_byte - range.start_byte)
                    .sum::<usize>(),
            );
            if injected_bytes > 4 * 1024 * 1024 {
                continue;
            }
            let Some(entry) = LanguageRegistry::singleton().entry(&language) else {
                continue;
            };
            let cached = old.iter().position(|layer| {
                layer.language == language
                    && layer.generation == entry.generation
                    && layer.ranges == ranges
            });
            let mut layer = if let Some(index) = cached {
                old.swap_remove(index)
            } else if timeout.is_some_and(|limit| limit <= Duration::from_millis(2)) {
                // Cold injection stores/queries cannot obey a foreground parse deadline.
                // Return to the adapter's background path before creating any of them.
                return false;
            } else if let Some(layer) = Layer::new(&language, entry) {
                layer
            } else {
                continue;
            };
            if layer.parser.set_included_ranges(&ranges).is_err() {
                continue;
            }
            layer.ranges = ranges;
            layer.depth = depth;
            if !layer.parse(&self.source, edit.as_ref(), deadline) {
                return false;
            }
            let Some(requests) = injection_requests(&layer, &self.source, deadline) else {
                return false;
            };
            pending.extend(requests.into_iter().map(|request| (request, depth + 1)));
            layers.push(layer);
        }
        self.layers = layers;
        true
    }

    /// Resolve non-overlapping UTF-8 style runs covering the requested source range.
    /// Deeper injected captures win, then shorter captures and later query patterns.
    pub(crate) fn styles(
        &self,
        range: &Range<usize>,
        resolver: &dyn HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        let end = range.end.min(self.source.len());
        let start = range.start.min(end);
        if start == end {
            return Vec::new();
        }
        let mut captures = Vec::new();
        for layer in self.root.iter().chain(self.layers.iter()) {
            let depth = layer.depth;
            let Some(tree) = &layer.tree else {
                continue;
            };
            let mut cursor = QueryCursor::new();
            cursor.set_byte_range(start..end);
            cursor.set_match_limit(65536);
            let mut matches = cursor.matches(&layer.query, tree.root_node(), Text(&self.source));
            while let Some(found) = matches.next() {
                for capture in found.captures {
                    let name = layer.query.capture_names()[capture.index as usize];
                    let Some(style) = resolver.style(name) else {
                        continue;
                    };
                    let bytes = capture.node.byte_range();
                    let from = bytes.start.max(start);
                    let to = bytes.end.min(end);
                    if from < to
                        && self.source.is_char_boundary(from)
                        && self.source.is_char_boundary(to)
                    {
                        captures.push((
                            from..to,
                            (
                                depth,
                                std::cmp::Reverse(bytes.len()),
                                found.pattern_index,
                                captures.len(),
                            ),
                            style,
                        ));
                    }
                    if captures.len() == 65536 {
                        break;
                    }
                }
                if captures.len() == 65536 {
                    break;
                }
            }
        }
        let mut events: BTreeMap<usize, Vec<(bool, usize)>> = BTreeMap::new();
        events.entry(start).or_default();
        events.entry(end).or_default();
        for (index, (range, _, _)) in captures.iter().enumerate() {
            events.entry(range.start).or_default().push((true, index));
            events.entry(range.end).or_default().push((false, index));
        }
        let mut active = BTreeSet::new();
        let mut result: Vec<(Range<usize>, HighlightStyle)> = Vec::new();
        let mut previous = start;
        for (offset, changes) in events {
            if previous < offset {
                let style = active
                    .last()
                    .map(|(_, index): &(_, usize)| captures[*index].2)
                    .unwrap_or_default();
                if let Some((range, old)) = result.last_mut() {
                    if *old == style && range.end == previous {
                        range.end = offset;
                    } else {
                        result.push((previous..offset, style));
                    }
                } else {
                    result.push((previous..offset, style));
                }
            }
            for (insert, index) in changes {
                let key = (captures[index].1, index);
                if insert {
                    active.insert(key);
                } else {
                    active.remove(&key);
                }
            }
            previous = offset;
        }
        result
    }
}

/// Extract validated static injections while preserving physical coordinates and child exclusions.
/// Unknown targets remain plain; combined patterns share a parser over normalized included ranges.
fn injection_requests(
    layer: &Layer,
    source: &Rope,
    deadline: Option<Instant>,
) -> Option<Vec<(String, Vec<tree_sitter::Range>)>> {
    let (Some(query), Some(tree)) = (&layer.injections, &layer.tree) else {
        return Some(Vec::new());
    };
    let mut groups: Vec<(usize, String, Vec<tree_sitter::Range>)> = Vec::new();
    let mut cursor = QueryCursor::new();
    cursor.set_match_limit(4096);
    let mut progress = |_: &tree_sitter::QueryCursorState| {
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    };
    let mut matches = cursor.matches_with_options(
        query,
        tree.root_node(),
        Text(source),
        QueryCursorOptions::new().progress_callback(&mut progress),
    );
    let mut captured_ranges = 0;
    let mut events = 0;
    while let Some(found) = matches.next() {
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return None;
        }
        events += 1;
        // Query match limits bound concurrent states, not total results. Bound accumulated work too.
        if events > 4096 {
            return Some(Vec::new());
        }
        let settings = query.property_settings(found.pattern_index);
        let Some(language) = settings
            .iter()
            .find(|property| property.key.as_ref() == "injection.language")
            .and_then(|property| property.value.as_deref())
        else {
            continue;
        };
        if !layer
            .allowed
            .iter()
            .any(|allowed| allowed.as_ref() == language)
        {
            continue;
        }
        let include_children = settings
            .iter()
            .any(|property| property.key.as_ref() == "injection.include-children");
        let combined = settings
            .iter()
            .any(|property| property.key.as_ref() == "injection.combined");
        let mut ranges = Vec::new();
        for capture in found
            .captures
            .iter()
            .filter(|capture| query.capture_names()[capture.index as usize] == "injection.content")
        {
            let node = capture.node;
            let mut pieces = Vec::new();
            let mut start = (node.start_byte(), node.start_position());
            if !include_children {
                let mut walker = node.walk();
                for child in node.children(&mut walker) {
                    if pieces.len() == 4096 {
                        return Some(Vec::new());
                    }
                    if start.0 < child.start_byte() {
                        pieces.push(tree_sitter::Range {
                            start_byte: start.0,
                            start_point: start.1,
                            end_byte: child.start_byte(),
                            end_point: child.start_position(),
                        });
                    }
                    start = (child.end_byte(), child.end_position());
                }
            }
            if start.0 < node.end_byte() {
                pieces.push(tree_sitter::Range {
                    start_byte: start.0,
                    start_point: start.1,
                    end_byte: node.end_byte(),
                    end_point: node.end_position(),
                });
            }
            for piece in pieces {
                if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    return None;
                }
                captured_ranges += layer.ranges.len().max(1);
                if captured_ranges > 4096 {
                    return Some(Vec::new());
                }
                if layer.ranges.is_empty() {
                    ranges.push(piece);
                } else {
                    for parent in &layer.ranges {
                        let from = piece.start_byte.max(parent.start_byte);
                        let to = piece.end_byte.min(parent.end_byte);
                        if from < to {
                            ranges.push(tree_sitter::Range {
                                start_byte: from,
                                start_point: if from == piece.start_byte {
                                    piece.start_point
                                } else {
                                    parent.start_point
                                },
                                end_byte: to,
                                end_point: if to == piece.end_byte {
                                    piece.end_point
                                } else {
                                    parent.end_point
                                },
                            });
                        }
                    }
                }
            }
        }
        if ranges.is_empty() {
            continue;
        }
        if combined {
            if let Some((_, _, existing)) = groups
                .iter_mut()
                .find(|(pattern, _, _)| *pattern == found.pattern_index)
            {
                existing.extend(ranges);
                continue;
            }
        }
        if groups.len() == 128 {
            break;
        }
        groups.push((found.pattern_index, language.into(), ranges));
    }
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        return None;
    }
    Some(
        groups
            .into_iter()
            .filter_map(|(_, language, mut ranges)| {
                ranges.sort_unstable_by_key(|range| range.start_byte);
                let mut normalized: Vec<tree_sitter::Range> = Vec::new();
                for range in ranges {
                    if let Some(previous) = normalized.last_mut() {
                        if range.start_byte <= previous.end_byte {
                            if range.end_byte > previous.end_byte {
                                previous.end_byte = range.end_byte;
                                previous.end_point = range.end_point;
                            }
                            continue;
                        }
                    }
                    normalized.push(range);
                }
                (!normalized.is_empty()).then_some((language, normalized))
            })
            .collect(),
    )
}
