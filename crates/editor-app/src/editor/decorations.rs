//! Current owned marker coordinates share the native editor's incremental edit and layout seams.
//! No document text, prior revisions, undo entries, parser or independent highlighter is retained.
use crate::*;
use gpui_base::input::{
    InputEdit, RangeDecoration, RangeDecorationCollection, RangeDecorationStyle, Rope,
};
use gpui_kit::{Half, SharedString};
use plugin_runtime::{
    DecorationResource,
    plugin_protocol::{api, decorations as marks},
};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};

/// One item's current location becomes None when a replacement removes its entire meaning.
struct Tracked {
    marker: marks::Marker,
    range: Option<api::TextRange>,
}
/// Each source has its own Base paint collection; disposing one source never clears another.
struct Collection {
    resource: Arc<DecorationResource>,
    markers: Vec<Tracked>,
    icons: BTreeMap<String, Arc<[u8]>>,
    paint: RangeDecorationCollection,
    /// Base `set` always notifies. Cache only the current projection to avoid a render/notify loop.
    painted: Vec<RangeDecoration>,
}
/// Only bounded current geometry is shared with the one editor highlighter.
#[derive(Default)]
pub(crate) struct Markers {
    collections: Vec<Collection>,
}
pub(crate) type SharedMarkers = Rc<RefCell<Markers>>;
impl Markers {
    /// Strict native boundaries are checked before publication; Base normalization never hides invalid input.
    pub(crate) fn validate(
        &self,
        resource: &DecorationResource,
        markers: &[marks::Marker],
        text: &Rope,
    ) -> Result<(), api::Failure> {
        let other = self
            .collections
            .iter()
            .filter(|collection| {
                collection.resource.handle() != resource.handle() && collection.resource.is_live()
            })
            .map(|collection| collection.markers.len())
            .sum::<usize>();
        if other + markers.len() > 4096 {
            return Err(api::Failure::new(
                api::ErrorCode::LimitExceeded,
                "Document decoration budget exceeds 4096 markers",
            ));
        }
        for marker in markers {
            let (start, end) = match marker.anchor {
                marks::Anchor::Point { offset, .. } => (offset, offset),
                marks::Anchor::Range { range } => (range.start, range.end),
            };
            if end > text.len() || !text.is_char_boundary(start) || !text.is_char_boundary(end) {
                return Err(api::Failure::new(
                    api::ErrorCode::InvalidRequest,
                    "Marker is outside text or splits UTF-8",
                ));
            }
        }
        Ok(())
    }
    /// Replacement updates only an identical resource incarnation, retaining no prior marker generation.
    pub(crate) fn replace(
        &mut self,
        resource: Arc<DecorationResource>,
        markers: Vec<marks::Marker>,
        icons: BTreeMap<String, Arc<[u8]>>,
        paint: Option<RangeDecorationCollection>,
    ) {
        let tracked = markers
            .into_iter()
            .map(|marker| {
                let range = match marker.anchor {
                    marks::Anchor::Point { offset, .. } => api::TextRange {
                        start: offset,
                        end: offset,
                    },
                    marks::Anchor::Range { range } => range,
                };
                Tracked {
                    marker,
                    range: Some(range),
                }
            })
            .collect();
        if let Some(collection) = self
            .collections
            .iter_mut()
            .find(|collection| collection.resource.handle() == resource.handle())
        {
            collection.markers = tracked;
            collection.icons = icons;
        } else if let Some(paint) = paint {
            self.collections.push(Collection {
                resource,
                markers: tracked,
                icons,
                paint,
                painted: Vec::new(),
            });
        }
    }
    pub(crate) fn contains(&self, resource: &DecorationResource) -> bool {
        self.collections.iter().any(|collection| {
            collection.resource.handle() == resource.handle() && collection.resource.is_live()
        })
    }
    pub(crate) fn positions(
        &self,
        resource: &DecorationResource,
    ) -> Result<Vec<marks::Position>, api::Failure> {
        let collection = self
            .collections
            .iter()
            .find(|collection| {
                collection.resource.handle() == resource.handle() && collection.resource.is_live()
            })
            .ok_or_else(|| {
                api::Failure::new(
                    api::ErrorCode::InvalidHandle,
                    "Decoration collection is no longer attached",
                )
            })?;
        Ok(collection
            .markers
            .iter()
            .map(|tracked| marks::Position {
                id: tracked.marker.id.clone(),
                range: tracked.range,
            })
            .collect())
    }
    /// The actual native InputEdit feeds every marker once, including all entries in update_batch.
    pub(crate) fn edit(&mut self, edit: &InputEdit) {
        if edit.start_byte == edit.old_end_byte && edit.start_byte == edit.new_end_byte {
            return;
        }
        for collection in &mut self.collections {
            if !collection.resource.is_live() {
                continue;
            }
            for tracked in &mut collection.markers {
                let Some(range) = tracked.range else {
                    continue;
                };
                tracked.range = match tracked.marker.anchor {
                    marks::Anchor::Point { affinity, .. } => {
                        let offset = transform(range.start, affinity, edit);
                        Some(api::TextRange {
                            start: offset,
                            end: offset,
                        })
                    }
                    marks::Anchor::Range { .. }
                        if edit.start_byte < edit.old_end_byte
                            && edit.start_byte <= range.start
                            && edit.old_end_byte >= range.end =>
                    {
                        None
                    }
                    marks::Anchor::Range { .. } => {
                        let start = transform(range.start, marks::Affinity::Right, edit);
                        let end = transform(range.end, marks::Affinity::Left, edit);
                        (start < end).then_some(api::TextRange { start, end })
                    }
                };
            }
        }
    }
    /// Bind the resulting version after the native Change subscription; release only revoked source layers.
    pub(crate) fn synchronize(&mut self, document: Option<&api::DocumentVersion>, cx: &mut App) {
        self.collections.retain(|collection| {
            if collection.resource.is_live() {
                true
            } else {
                collection.paint.dispose(cx);
                false
            }
        });
        for collection in &mut self.collections {
            if let Some(document) = document {
                collection.resource.bind(document.clone());
            }
            let colors: Vec<_> = collection
                .markers
                .iter()
                .filter_map(|tracked| {
                    let range = tracked.range.filter(|range| range.start < range.end)?;
                    Some(
                        RangeDecoration::new(range.start..range.end)
                            .with_style(RangeDecorationStyle::Fill)
                            .with_color(color(tracked.marker.style, cx).opacity(0.16)),
                    )
                })
                .collect();
            if colors != collection.painted {
                collection.paint.set(colors.clone(), cx);
                collection.painted = colors;
            }
        }
    }
    /// A closed document seals shared runtime resources even when a retained native entity still exists.
    pub(crate) fn revoke(&self) {
        for collection in &self.collections {
            collection.resource.revoke();
        }
    }
    /// Projection borrows accepted current anchors and immutable icons; it never samples disk text.
    fn presentation(
        &self,
        dark: bool,
        locale: &str,
    ) -> Vec<(
        api::TextRange,
        String,
        Option<Arc<[u8]>>,
        marks::Style,
        String,
    )> {
        self.collections
            .iter()
            .filter(|collection| collection.resource.is_live())
            .flat_map(|collection| {
                collection.markers.iter().filter_map(move |tracked| {
                    let range = tracked.range?;
                    let icon = tracked
                        .marker
                        .icon
                        .as_ref()
                        .and_then(|icon| collection.icons.get(icon.path(dark)))
                        .cloned();
                    Some((
                        range,
                        tracked.marker.label.for_locale(locale).to_string(),
                        icon,
                        tracked.marker.style,
                        format!(
                            "{}-{}-{}",
                            collection.resource.handle().instance,
                            collection.resource.handle().resource,
                            tracked.marker.id
                        ),
                    ))
                })
            })
            .collect()
    }
}
/// Insertions at either edge obey affinity. Interior replacement collapses points to the selected edge.
fn transform(point: usize, affinity: marks::Affinity, edit: &InputEdit) -> usize {
    if point < edit.start_byte {
        point
    } else if point > edit.old_end_byte {
        point
            .saturating_sub(edit.old_end_byte)
            .saturating_add(edit.new_end_byte)
    } else if affinity == marks::Affinity::Right {
        edit.new_end_byte
    } else {
        edit.start_byte
    }
}
fn color(style: marks::Style, cx: &App) -> gpui_kit::Hsla {
    match style {
        marks::Style::Accent => cx.theme().accent,
        marks::Style::Success => cx.theme().highlight_theme.style.status.success(cx),
        marks::Style::Warning => cx.theme().highlight_theme.style.status.warning(cx),
        marks::Style::Error => cx.theme().highlight_theme.style.status.error(cx),
    }
}
impl EditorApp {
    /// Gutter elements use public native range geometry after prepaint, retaining horizontal-scroll position.
    pub(crate) fn render_editor_markers(&self, cx: &Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        gpui_kit::canvas(
            move |_, window, cx| {
                let current = owner
                    .read_with(cx, |app, cx| {
                        let index = app.active_text_tab_index()?;
                        let markers = app.tabs[index].text.as_ref()?.highlighter.markers();
                        let editor = app.editor.read(cx);
                        let viewport = editor.input_bounds();
                        let visible_rows = editor.visible_row_range()?;
                        let items = markers
                            .borrow()
                            .presentation(cx.theme().is_dark(), &rust_i18n::locale());
                        Some((
                            viewport,
                            items
                                .into_iter()
                                .filter_map(|(range, label, artwork, style, key)| {
                                    // Native geometry can clamp an offset above the viewport to its
                                    // first laid-out row. Check the native buffer-row range before
                                    // creating an element, so offscreen marks have neither a hit
                                    // region nor an accessibility node.
                                    let source_row =
                                        editor.text().offset_to_position(range.start).line as usize;
                                    if !visible_rows.contains(&source_row) {
                                        return None;
                                    }
                                    let row =
                                        editor.range_to_bounds(&(range.start..range.start))?;
                                    (row.bottom() > viewport.top() && row.top() < viewport.bottom())
                                        .then_some((row, label, artwork, style, key))
                                })
                                .collect::<Vec<_>>(),
                        ))
                    })
                    .ok()
                    .flatten();
                let clip = current.as_ref().map(|(viewport, _)| *viewport);
                let mut elements = Vec::new();
                if let Some((viewport, items)) = current {
                    for (row, label, artwork, style, key) in items {
                        let position = point(
                            viewport.left() + px(2.),
                            row.top() + (row.size.height - px(12.)).half(),
                        );
                        let mut element = div()
                            .id(SharedString::from(format!("decoration-{key}")))
                            .debug_selector(move || format!("decoration-{key}").into())
                            .role(gpui_kit::Role::Label)
                            .aria_label(label.clone())
                            .aria_value(label.clone())
                            .size(px(12.))
                            .text_color(color(style, cx))
                            .child(
                                artwork
                                    .map(|artwork| {
                                        Icon::default()
                                            .data(&artwork)
                                            .size(px(12.))
                                            .into_any_element()
                                    })
                                    .unwrap_or_else(|| {
                                        div()
                                            .size(px(8.))
                                            .rounded(px(4.))
                                            .bg(color(style, cx))
                                            .into_any_element()
                                    }),
                            )
                            .tooltip(move |window, cx| {
                                crate::ui::controls::Tooltip::new(label.clone()).build(window, cx)
                            })
                            .into_any_element();
                        // Native scroll geometry may leave a partial row at either edge. Clip both
                        // its hit region and painting to the actual editor, not a copied layout.
                        window.with_content_mask(
                            Some(gpui_kit::ContentMask { bounds: viewport }),
                            |window| {
                                element.prepaint_as_root(
                                    position,
                                    size(px(12.), px(12.)).into(),
                                    window,
                                    cx,
                                );
                            },
                        );
                        elements.push(element);
                    }
                }
                (clip, elements)
            },
            |_, (clip, mut elements), window, cx| {
                window.with_content_mask(
                    clip.map(|bounds| gpui_kit::ContentMask { bounds }),
                    |window| {
                        for element in &mut elements {
                            element.paint(window, cx);
                        }
                    },
                );
            },
        )
        .absolute()
        .size_full()
    }
    /// Range hover uses native laid-out rows and bounded current metadata, with no competing language task.
    pub(crate) fn plugin_marker_hover(
        &self,
        position: Point<Pixels>,
        cx: &App,
    ) -> Option<(std::ops::Range<usize>, lsp_types::Hover)> {
        let index = self.active_text_tab_index()?;
        let editor = self.editor.read(cx);
        if !editor.input_bounds().contains(&position) {
            return None;
        }
        let markers = self.tabs[index].text.as_ref()?.highlighter.markers();
        let presentations = markers
            .borrow()
            .presentation(cx.theme().is_dark(), &rust_i18n::locale());
        let mut labels = Vec::new();
        let mut result = None;
        for (range, label, _, _, _) in presentations {
            if range.start == range.end {
                continue;
            }
            let first = editor.text().offset_to_position(range.start).line as usize;
            let last = editor.text().offset_to_position(range.end).line as usize;
            let visible = editor.visible_row_range()?;
            for row in first.max(visible.start)..=last.min(visible.end.saturating_sub(1)) {
                let start = range.start.max(editor.text().line_start_offset(row));
                let end = range.end.min(editor.text().line_end_offset(row));
                if start < end
                    && editor
                        .range_to_bounds(&(start..end))
                        .is_some_and(|bounds| bounds.contains(&position))
                {
                    result.get_or_insert(start..end);
                    labels.push(label.clone());
                    break;
                }
            }
        }
        Some((
            result?,
            lsp_types::Hover {
                contents: lsp_types::HoverContents::Scalar(lsp_types::MarkedString::String(
                    labels.join("\n"),
                )),
                range: None,
            },
        ))
    }
}
