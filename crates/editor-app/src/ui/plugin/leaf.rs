//! Native leaf controls draw without retaining the recursive container/editor renderer on the stack.
use super::*;
use editor_ui::scroll::ScrollableElement as _;
use editor_ui::{Button, Input, Tooltip};
use gpui_base::{Checkbox, CheckboxState, Progress, Radio, RadioGroup};
use gpui_kit::{
    AnyElement, InteractiveElement, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, StyledImage as _, div, prelude::FluentBuilder as _, px, relative,
};
use plugin_runtime::plugin_protocol::ui::Node;

/// Plain plugin text uses the same element for painting and native accessibility metadata.
fn text_leaf(native_id: SharedString, text: &str) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(native_id)
        .role(gpui_kit::Role::Label)
        .aria_label(text.to_owned())
        // AccessKit's Windows Label Name is sourced from value, while label serves other consumers.
        .aria_value(text.to_owned())
        .child(text.to_owned())
}

impl PluginView {
    /// Concrete controls share the current scene gate, local appearance and Base interaction behavior.
    pub(super) fn leaf_node(
        &mut self,
        node: &Node,
        disabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = self.colors(node.theme_role(), cx);
        let id = node.id.clone();
        let native_id = SharedString::from(format!("plugin-ui-{}", node.id));
        let gesture = self.action_gesture(&node.id);
        match &node.kind {
            Kind::Tree { .. } => self
                .trees
                .get(&node.id)
                .map(|view| view.clone().into_any_element())
                .unwrap_or_else(|| div().into_any_element()),
            Kind::SideTabs(_) => self.collections[&node.id].clone().into_any_element(),
            Kind::Canvas(_) => self.canvases[&node.id].clone().into_any_element(),
            Kind::FileImage { alt, sizing } => {
                self.render_file_image(&node.id, alt, *sizing, node.viewport.clone(), cx)
            }
            Kind::Text { text } => text_leaf(native_id, text).into_any_element(),
            Kind::Image { source, alt } => {
                // Direct retained pixels bypass ambient file/URL loading. Declared dimensions choose
                // the pixel area's ratio; a missing dimension follows the intrinsic bitmap ratio.
                let photo = self.photos.get(&node.id).filter(|photo| {
                    self.document
                        .source
                        .as_ref()
                        .is_some_and(|source| photo.resource.source == *source)
                        && source == &photo.resource.uri
                });
                if let Some(photo) = photo
                    && let Ok(Some(bitmap)) = &photo.decoded
                {
                    if node.viewport.is_some() {
                        return self.render_visual_bitmap(
                            &node.id,
                            bitmap.clone(),
                            ui::ImageSizing::OriginalContain,
                            node.viewport.clone(),
                            cx,
                        );
                    }
                    let image_id = format!("plugin-image-{}", node.id);
                    let pixels_id = format!("plugin-image-pixels-{}", node.id);
                    let intrinsic_ratio = bitmap.width as f32 / bitmap.height as f32;
                    let width = node.layout.width.unwrap_or_else(|| {
                        node.layout
                            .height
                            .map_or(bitmap.width as f32, |height| height * intrinsic_ratio)
                    });
                    let ratio = match (node.layout.width, node.layout.height) {
                        (Some(width), Some(height)) if width > 0. && height > 0. => width / height,
                        _ => intrinsic_ratio,
                    };
                    let zero_extent = width == 0. || node.layout.height == Some(0.);
                    let fit = if node.layout.width.is_some() && node.layout.height.is_some() {
                        gpui_kit::ObjectFit::Fill
                    } else {
                        gpui_kit::ObjectFit::Contain
                    };
                    div()
                        .id(SharedString::from(image_id.clone()))
                        .debug_selector(move || image_id.clone())
                        // Alternative text names the image for accessibility, not a visible caption.
                        .role(gpui_kit::Role::Image)
                        .aria_label(alt.clone())
                        .flex()
                        .flex_col()
                        .w_full()
                        // Bound the complete element as well as pixels. Otherwise a height-only
                        // image can inherit a wider flex parent's intrinsic aspect-ratio height.
                        .max_w(px(width))
                        .min_w_0()
                        .child(
                            div()
                                .debug_selector(move || pixels_id.clone())
                                .flex_shrink_0()
                                .w_full()
                                .max_w(px(width))
                                .when(!zero_extent, |view| view.aspect_ratio(ratio))
                                // Zero is a valid public dimension; never construct a zero/NaN ratio.
                                .when(zero_extent, |view| {
                                    view.h(px(node.layout.height.unwrap_or(0.)))
                                })
                                .child(
                                    gpui_kit::img(bitmap.image.clone())
                                        .size_full()
                                        .object_fit(fit),
                                ),
                        )
                        .into_any_element()
                } else {
                    let key = super::bitmap::status_key(
                        photo.and_then(|photo| photo.decoded.as_ref().err()),
                    );
                    let label = rust_i18n::t!(key, locale = self.environment.locale.as_str());
                    let status_id = format!("plugin-image-status-{}-{key}", node.id);
                    div()
                        .debug_selector(move || status_id.clone())
                        .w_full()
                        .border_1()
                        .border_color(colors.border)
                        .rounded(px(4.))
                        .p_2()
                        // A pending/failed resource reserves its declared image area without borrowing pixels.
                        .when_some(node.layout.height, |view, height| view.h(px(height)))
                        .child(if alt.is_empty() {
                            label.to_string()
                        } else {
                            format!("{alt} · {label}")
                        })
                        .into_any_element()
                }
            }
            Kind::RichText { html } => {
                let owner = cx.entity().downgrade();
                let revision = self.document.revision;
                let link_events = self.document.link_events;
                editor_ui::rich_text_view(
                    SharedString::from(format!("{native_id}-scene-{revision}")),
                    html.clone(),
                    px(self
                        .content_font(node.theme_role(), false)
                        .size_px
                        .unwrap_or(14.)),
                    editor_ui::RichTextColors {
                        foreground: colors.foreground,
                        muted_foreground: colors.muted_foreground,
                        code_background: colors.code_background,
                        inline_code_background: colors.inline_code_background,
                        border: colors.border,
                        link: colors.accent,
                    },
                    self.document.content_styles.get(node.theme_role()),
                    window.rem_size(),
                    cx,
                )
                // Base resolves real link hit targets and selection gestures. No URI is opened here;
                // a negotiated event reaches the guest and then the versioned native effect boundary.
                .on_link_click(move |uri, event, window, cx| {
                    if link_events {
                        let _ = owner.update(cx, |this, cx| {
                            if this.action_gesture_current(&id, gesture)
                                && this.accept_link_click(&id, revision, event)
                            {
                                this.emit_version_in(
                                    &id,
                                    revision,
                                    Action::Link {
                                        uri: uri.to_string(),
                                    },
                                    window,
                                    cx,
                                );
                            }
                        });
                    }
                })
                .into_any_element()
            }
            Kind::CodeBlock { text, .. } => {
                // Each literal line keeps its whitespace; the host never parses guest code as markup.
                let font = self.content_font(node.theme_role(), true);
                let preset = self
                    .document
                    .content_styles
                    .get(node.theme_role())
                    .and_then(|style| style.code.as_ref());
                let font_size =
                    font.size_px.unwrap_or(14.) * preset.map_or(1., |style| style.scale);
                div()
                    .id(native_id.clone())
                    .w_full()
                    .overflow_x_scrollbar()
                    .bg(colors.code_background)
                    .border(px(preset.map_or(1., |style| style.border)))
                    .border_color(colors.border)
                    .rounded(px(preset.map_or(4., |style| style.radius)))
                    .p(px(preset.map_or(12., |style| style.padding)))
                    .font_family(font.family.unwrap_or_else(|| "Consolas".into()))
                    .text_size(px(font_size))
                    .when_some(preset, |view, style| {
                        view.line_height(relative(style.line_height))
                    })
                    .child(div().flex().flex_col().children({
                        let mut start = 0;
                        text.split_inclusive('\n')
                            .enumerate()
                            .map(|(index, raw)| {
                                // CRLF consumes two source bytes; splitting offsets must retain both of them.
                                let line = raw
                                    .strip_suffix('\n')
                                    .map(|line| line.strip_suffix('\r').unwrap_or(line))
                                    .unwrap_or(raw);
                                let content = self.code_line(&node.id, index, line, start, cx);
                                start += raw.len();
                                content
                            })
                            .collect::<Vec<_>>()
                    }))
                    .into_any_element()
            }
            Kind::Button { label } => self
                .button_appearance(
                    Button::new(action_gestures::native_id(native_id.clone(), gesture))
                        .when_some(
                            self.action_gestures
                                .focus
                                .get(&action_gestures::FocusTarget::Button(node.id.clone())),
                            |button, focus| button.track_focus(focus),
                        )
                        .debug_selector({
                            // Keep native form actions addressable after splitting the leaf renderer.
                            let control = format!("plugin-button-{}", node.id);
                            move || control.clone()
                        })
                        .accessibility_label(label.clone())
                        .when(self.editor_action_nodes.contains(&node.id), |button| {
                            // Three native rail actions fit without padding expanding the row;
                            // disabled siblings keep the same geometry and accessible controls.
                            button.small().compact().w(px(24.)).min_w_0().px_0()
                        })
                        .when(node.button_icon.is_none(), |button| {
                            button.label(label.clone())
                        })
                        .when_some(node.button_icon.as_ref(), |button, svg| {
                            // The negotiated, validated icon retains Base focus and Click behavior.
                            button
                                .icon(editor_ui::Icon::default().data(svg.as_bytes()))
                                .small()
                                .compact()
                        })
                        .when_some(node.tooltip.clone(), |button, tooltip| {
                            button.accessibility_label(tooltip.clone()).tooltip(tooltip)
                        })
                        // A waiting latest draft blocks activations, not editing authority.
                        .disabled(disabled || self.form_activation_blocked())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if this.action_gesture_current(&id, gesture) {
                                this.emit_in(&id, Action::Click, window, cx);
                            }
                        })),
                    node,
                    cx,
                )
                .into_any_element(),
            Kind::Input(_) => self
                .font(
                    Input::new(&self.inputs[&node.id].state)
                        .bg(colors.background)
                        .text_color(colors.foreground)
                        .border_color(colors.border),
                    node.theme_role(),
                )
                .into_any_element(),
            // Multiline form fields share the stable state and revision rules of single-line input.
            Kind::Textarea(_) => self
                .font(
                    div()
                        .w_full()
                        .when(node.layout.height.is_some(), |view| view.h_full())
                        .child(
                            editor_ui::Textarea::new(&self.textareas[&node.id].state).when_some(
                                node.layout.height,
                                |view, height| {
                                    view.height(px((height - node.layout.padding * 2.).max(0.)))
                                },
                            ),
                        ),
                    node.theme_role(),
                )
                .into_any_element(),
            Kind::Checkbox { label, checked } => {
                let owner = cx.entity().downgrade();
                let revision = self.document.revision;
                // Scene-bound ordinary controls keep their existing IDs; source actions also
                // revoke presses on native source changes and withdrawal without losing focus.
                let gesture_id =
                    action_gestures::native_id(format!("{native_id}-scene-{revision}"), gesture);
                // Marker-only controls keep their prose outside the checkbox. The ordinary
                // localized hint supplies an accessible name without duplicating visible text.
                let accessible_label = if label.is_empty() {
                    node.tooltip.as_deref().unwrap_or(label).to_owned()
                } else {
                    label.clone()
                };
                let marker_id = format!("plugin-checkbox-marker-{}", node.id);
                let marker = div()
                    // Expose the visible hit target separately from a stretched list-row wrapper.
                    .debug_selector(move || marker_id.clone())
                    .size(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_1()
                    .border_color(colors.border)
                    .rounded(px(3.))
                    .bg(if *checked {
                        colors.accent
                    } else {
                        colors.background
                    })
                    .text_color(colors.accent_foreground)
                    .when(*checked, |v| v.child("✓"));
                self.font(
                    Checkbox::new(gesture_id)
                        .track_focus(&self.checkbox_focus[&node.id])
                        .checked(*checked)
                        .disabled(disabled)
                        .accessibility_label(accessible_label)
                        // Base owns traversal and activation; the editor supplies its focus appearance.
                        .border_1()
                        .border_color(colors.background)
                        .rounded(px(4.))
                        .focus_visible(|style| style.border_color(colors.accent))
                        .when_some(node.tooltip.clone(), |checkbox, tooltip| {
                            checkbox.tooltip(move |window, cx| {
                                Tooltip::new(tooltip.clone()).build(window, cx)
                            })
                        })
                        .flex()
                        .items_center()
                        .gap_2()
                        .on_change(move |state, _, window, cx| {
                            let _ = owner.update(cx, |this, cx| {
                                if this.action_gesture_current(&id, gesture) {
                                    this.emit_version_in(
                                        &id,
                                        revision,
                                        Action::Toggle(state == CheckboxState::Checked),
                                        window,
                                        cx,
                                    );
                                }
                            });
                        })
                        .child(marker)
                        .child(label.clone()),
                    node.theme_role(),
                )
                .into_any_element()
            }
            Kind::Choice { options, selected } => {
                let mut group = RadioGroup::new(native_id.clone()).flex().flex_col().gap_2();
                for (index, option) in options.iter().enumerate() {
                    let owner = cx.entity().downgrade();
                    let checked = selected.as_ref() == Some(&option.id);
                    let node_id = node.id.clone();
                    let option_id = option.id.clone();
                    let marker = div()
                        .size(px(14.))
                        .rounded_full()
                        .border_1()
                        .border_color(colors.border)
                        .bg(if checked {
                            colors.accent
                        } else {
                            colors.background
                        });
                    group = group.child(
                        Radio::new(action_gestures::native_id(
                            format!("{}-option-{index}", node.id),
                            gesture,
                        ))
                        .when_some(
                            self.action_gestures
                                .focus
                                .get(&action_gestures::FocusTarget::Choice(
                                    node.id.clone(),
                                    option.id.clone(),
                                )),
                            |radio, focus| radio.track_focus(focus),
                        )
                        .checked(checked)
                        .disabled(disabled || option.disabled)
                        .accessibility_label(option.label.clone())
                        .set_position(index + 1, options.len())
                        .flex()
                        .items_center()
                        .gap_2()
                        .on_change(move |_, _, window, cx| {
                            let _ = owner.update(cx, |this, cx| {
                                if this.action_gesture_current(&node_id, gesture) {
                                    this.emit_in(
                                        &node_id,
                                        Action::Select(option_id.clone()),
                                        window,
                                        cx,
                                    );
                                }
                            });
                        })
                        .child(marker)
                        .child(option.label.clone()),
                    );
                }
                group.into_any_element()
            }
            Kind::List { items } => div()
                .flex()
                .flex_col()
                .gap_1()
                .children(items.iter().map(|text| div().py_1().child(text.clone())))
                .into_any_element(),
            Kind::Table { headers, rows } => {
                let mut table = div()
                    .flex()
                    .flex_col()
                    .border_1()
                    .border_color(colors.border);
                for (index, row) in std::iter::once(headers).chain(rows).enumerate() {
                    table = table.child(
                        div()
                            .flex()
                            .when(index == 0, |v| v.bg(colors.active))
                            .border_b_1()
                            .border_color(colors.border)
                            .children(
                                row.iter()
                                    .map(|cell| div().flex_1().min_w_0().p_2().child(cell.clone())),
                            ),
                    );
                }
                table.into_any_element()
            }
            Kind::Separator => div()
                .w_full()
                .h(px(1.))
                .bg(colors.border)
                .into_any_element(),
            Kind::Progress { label, value } => Progress::new(native_id.clone())
                .value(*value)
                .accessibility_label(label.clone())
                .h(px(8.))
                .w_full()
                .rounded(px(4.))
                .bg(colors.background)
                .border_1()
                .border_color(colors.border)
                .child(
                    div()
                        .h_full()
                        .w(relative(*value / 100.))
                        .bg(colors.accent)
                        .rounded(px(4.)),
                )
                .into_any_element(),
            Kind::Spacer => div().min_h(px(8.)).into_any_element(),
            _ => unreachable!("recursive geometry is dispatched before leaf controls"),
        }
    }
}

#[cfg(test)]
mod text_tests {
    use super::*;
    use gpui_kit::{Element, Role};

    /// Windows labels obtain their native Name from value; inspect the same element that paints the result.
    #[test]
    fn native_text_metadata_exposes_current_result_for_every_plugin() {
        for text in ["HTTP 200 · 17 bytes\nfirst 世界😀", "Cancelled", "已取消"] {
            let element = text_leaf("plugin-ui-result".into(), text);
            assert_eq!(element.a11y_role(), Some(Role::Label));
            let mut node = gpui_kit::accesskit::Node::new(Role::Label);
            element.write_a11y_info(&mut node);
            assert_eq!(node.label(), Some(text));
            assert_eq!(node.value(), Some(text));
        }
    }
}
