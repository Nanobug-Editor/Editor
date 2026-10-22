//! Compose the existing native editor, popovers and input gestures without replacing its session.
use crate::*;
use editor_ui::aligned_editor::{AlignmentGroup, AlignmentNativeFactory};
use editor_ui::context_menu::NativeMenu;
use editor_ui::overlay::WindowExt as _;

/// One menu definition serves ordinary and projected input; only its physical anchor changes.
type SourceMenuBuilder = Rc<dyn Fn(NativeMenu, &mut Window, &mut App) -> NativeMenu>;
/// Local menu presentation can be observed by interaction tests without adding a plugin API.
type SourceMenuPresenter = Rc<dyn Fn(NativeMenu, Point<Pixels>, &mut Window, &mut App)>;

#[cfg(test)]
mod tests;

impl EditorApp {
    /// Native search and code-action panels require an opaque independent input presentation.
    /// Group owners use this signal for an explicit whole-group pause, never a one-pane fallback.
    pub(crate) fn alignment_native_overlays_open(&self, cx: &App) -> bool {
        let state = self.editor.read(cx);
        state.search_session().open || state.code_action_menu_state().open
    }

    /// Borrow the current native text entity for either a fallback body or an authorized layout node.
    pub(crate) fn render_native_editor(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        self.render_native_editor_body(None, window, cx)
    }

    /// Retains this source's commands, semantic input and overlays around a measured aligned pane.
    /// Only the atomic native input is composed by the group; SourceFrame is mounted exactly once.
    pub(crate) fn render_native_editor_aligned(
        &self,
        group: &AlignmentGroup,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        group.refresh_native_overlays(cx);
        self.render_native_editor_body(Some((group, index)), window, cx)
    }

    /// Shares the ordinary source frame while allowing a geometry-owning inner input composition.
    fn render_native_editor_body(
        &self,
        alignment: Option<(&AlignmentGroup, usize)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        if self
            .plugin_window_mounts
            .contains_key(&format!("{:?}", self.editor.entity_id()))
        {
            return div()
                .size_full()
                .p_4()
                .child(t!("editor.detached_source").to_string())
                .into_any_element();
        }
        editor_ui::synchronize_editor_appearance(&self.editor, cx);
        if let Some(index) = self.active_text_tab_index() {
            self.tabs[index]
                .text
                .as_ref()
                .expect("text tab")
                .highlighter
                .markers()
                .borrow_mut()
                .synchronize(None, cx);
        }
        let style = component_styles(cx, ThemeComponent::Editor).base;
        // Modal surfaces own the window until dismissed; ordinary floating
        // panels can remain below the raised definition details layer.
        let hover_enabled = self.explorer_edit.is_none()
            && self.explorer_delete.is_none()
            && !window.has_active_dialog(cx);
        // The menu callback runs inside an editor update, so snapshot its state now.
        let (enabled, editable, has_definition, has_code_actions) = {
            let editor = self.editor.read(cx);
            let presentation = editor.presentation();
            (
                !presentation.is_disabled(),
                presentation.is_editable(),
                editor.lsp().definition_provider.is_some(),
                !editor.lsp().code_action_providers.is_empty()
                    || self.native_language_feature_available(
                        plugin_runtime::plugin_protocol::direct::Feature::CodeActions,
                        cx,
                    ),
            )
        };
        let app = cx.entity().downgrade();
        let plugin_target = self
            .active_path
            .as_deref()
            .map(|path| self.plugin_menu_target(path));
        let plugin_rows = plugin_target
            .as_ref()
            .map(|target| {
                self.plugin_menu_entries(
                    target,
                    &[
                        plugin_runtime::plugin_protocol::commands::Location::Editor,
                        plugin_runtime::plugin_protocol::commands::Location::Selection,
                    ],
                    cx,
                )
            })
            .unwrap_or_default();
        let language = self
            .active_path
            .as_deref()
            .map(|path| self.language_for_path(path))
            .unwrap_or_default();
        // A cold selected formatter is still callable: the action owns its normal demand wait.
        let can_format = editable
            && self
                .providers
                .formatters()
                .get(&language)
                .is_some_and(Option::is_some);
        let can_format_selection = can_format && !self.editor.read(cx).selected_range().is_empty();
        let can_rename = editable
            && self
                .active_path
                .as_deref()
                .and_then(|path| self.language_server_for_path(path))
                .is_some_and(|server| server.provides_editing());
        let custom_popovers = {
            let state = self.editor.read(cx);
            !state.search_session().open
                && !state.code_action_menu_state().open
                && (state.completion_menu_state().open
                    || state.hover_popover().is_some()
                    || state.diagnostic_popover().is_some())
        };
        let menu_builder: SourceMenuBuilder = Rc::new(move |menu, _, cx| {
            // Route the menu action through the same fresh LSP request as F12.
            let menu = menu
                .menu_with_disabled(
                    t!("editor.go_to_definition").to_string(),
                    !(enabled && has_definition),
                    Box::new(NavigateToDefinition),
                )
                .menu_with_disabled(
                    t!("editor.code_actions").to_string(),
                    !(editable && has_code_actions),
                    Box::new(gpui_base::input::ToggleCodeActions),
                )
                .separator()
                .menu_with_disabled(
                    t!("editor.format_document").to_string(),
                    !can_format,
                    Box::new(FormatDocument),
                )
                .menu_with_disabled(
                    t!("editor.format_selection").to_string(),
                    !can_format_selection,
                    Box::new(FormatSelection),
                )
                .menu_with_disabled(
                    t!("editor.rename_symbol").to_string(),
                    !can_rename,
                    Box::new(RenameSymbol),
                )
                .separator()
                // Cut and Copy validate the live selection when their actions run.
                .menu_with_disabled(
                    t!("editor.cut").to_string(),
                    !editable,
                    Box::new(gpui_base::input::Cut),
                )
                .menu_with_disabled(
                    t!("editor.copy").to_string(),
                    !enabled,
                    Box::new(gpui_base::input::Copy),
                )
                .menu_with_disabled(
                    t!("editor.paste").to_string(),
                    !(editable && cx.read_from_clipboard().is_some()),
                    Box::new(gpui_base::input::Paste),
                )
                .separator()
                .menu(
                    t!("editor.select_all").to_string(),
                    Box::new(gpui_base::input::SelectAll),
                );
            if let Some(target) = plugin_target.clone() {
                extensions::command_menus::append_editor(menu, plugin_rows.clone(), target)
            } else {
                menu
            }
        });
        let native_menu = menu_builder.clone();
        let aligned_native = alignment.is_some();
        // The group repeats only atomic native controls; SourceFrame and language overlays remain singular.
        let native: AlignmentNativeFactory = Rc::new(move |editor, _, cx| {
            // Match the ordinary SourceFrame switch: styled InputOverlays would replace the host's
            // completion action handler with one belonging to its hidden native completion menu.
            if custom_popovers {
                return editor_ui::native_editor_scrollbars(editor);
            }
            let menu = native_menu.clone();
            Editor::new(editor)
                // The aligned pane owns the single accessible source node. Its atomic authority
                // is full-height and must not publish a duplicate invisible input to screen readers.
                .when(aligned_native, |editor| {
                    editor.role(gpui_base::RoleOverride::Presentational)
                })
                // Preserve live edit restrictions when the local source frame renders.
                .readonly(!editable)
                .disabled(!enabled)
                .context_menu(move |base, window, cx| menu(base, window, cx))
                .bordered(false)
                .p_0()
                .size_full()
                .min_h_0()
                .bg(style.background.unwrap_or(cx.theme().background))
                .text_color(style.foreground.unwrap_or(cx.theme().foreground))
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(
                    style
                        .font_size_px
                        .map(px)
                        .unwrap_or(cx.theme().mono_font_size),
                )
                .into_any_element()
        });
        let alignment = alignment.filter(|(group, index)| {
            group.set_presentation(
                *index,
                &self.editor,
                Some(self.aligned_linked_input_factory(cx)),
                Some(native.clone()),
                cx,
            )
        });
        let mut aligned_body = alignment.and_then(|(group, index)| group.pane(index));
        let projection = alignment
            .map(|(group, index)| super::popovers::SourceProjection::new(group.clone(), index));
        let aligned = alignment.is_some();
        let clicked_source = self.editor.clone();
        // Preserve the native editor and all its popovers while a plugin adds a sibling preview.
        div()
            .debug_selector(|| "editor-source-pane".into())
            // Only the host-rendered native editor admits document shortcuts inside a guest layout.
            .key_context("NativeEditorSource")
            .on_action(cx.listener(Self::plugin_editor_menu_action))
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .relative()
            .when_some(
                self.render_editor_source_toolbar(window, cx),
                |source, toolbar| source.child(toolbar),
            )
            // Capture selection presses before the base editor collapses them.
            .when(hover_enabled, |view| {
                view.capture_any_mouse_down(cx.listener(Self::text_drag_press))
                    .capture_any_mouse_down(cx.listener(Self::linked_pointer_down))
                    .capture_key_down(cx.listener(Self::linked_navigation_key))
                    .capture_action(cx.listener(Self::linked_history_undo))
                    .capture_action(cx.listener(Self::linked_history_redo))
                    .capture_action(cx.listener(Self::text_drag_escape))
                    .capture_action(cx.listener(Self::cancel_rename_action))
                    // Keep Base's native shortcut/menu activation, then admit the same versioned
                    // proposal used by direct and standard LSP consumers before any text effect.
                    .capture_action(cx.listener(Self::native_code_actions))
                    .capture_action(cx.listener(Self::paste_plugin_images))
                    .capture_action(cx.listener(Self::linked_paste))
                    .capture_action(cx.listener(Self::linked_cut))
                    .capture_action(cx.listener(Self::linked_backspace))
                    .capture_action(cx.listener(Self::linked_delete))
                    .on_drag_move(cx.listener(Self::image_drag_move))
            })
            // Register drag listeners before the editor's own selection listeners.
            .child(self.render_text_drag_events(cx))
            .when(hover_enabled, |source| {
                source.child(self.render_image_drag_events(cx))
            })
            .on_mouse_move(cx.listener(Self::editor_pointer_move))
            .on_mouse_up(MouseButton::Middle, move |event, window, cx| {
                let position = event.position;
                let app = app.clone();
                let clicked_source = clicked_source.clone();
                cx.stop_propagation();
                window.defer(cx, move |window, cx| {
                    if !app
                        .read_with(cx, |app, _| app.editor == clicked_source)
                        .unwrap_or(false)
                    {
                        return;
                    }
                    // Reuse the editor's own hit testing to place the caret under the click.
                    // The root router has restored the physical pointer before this deferred
                    // gesture; redispatching the already projected native point would translate twice.
                    let dispatch_position = if aligned {
                        window.mouse_position()
                    } else {
                        position
                    };
                    let modifiers = Modifiers::default();
                    window.dispatch_event(
                        PlatformInput::MouseDown(MouseDownEvent {
                            button: MouseButton::Left,
                            position: dispatch_position,
                            modifiers,
                            click_count: 1,
                            first_mouse: false,
                        }),
                        cx,
                    );
                    window.dispatch_event(
                        PlatformInput::MouseUp(MouseUpEvent {
                            button: MouseButton::Left,
                            position: dispatch_position,
                            modifiers,
                            click_count: 1,
                        }),
                        cx,
                    );
                    // Aligned mouse forwarding is deferred too. Queue the semantic query after
                    // both forwarded events, so its caret and empty-result anchor match the click.
                    window.defer(cx, move |window, cx| {
                        let _ = app.update(cx, |app, cx| {
                            if app.editor == clicked_source {
                                app.request_definition(Some(dispatch_position), window, cx);
                            }
                        });
                    });
                });
            })
            .child(
                super::popovers::render(
                    &self.editor,
                    &self.completion_popup,
                    &self.definition_popup_focus,
                    style,
                    hover_enabled,
                    &mut aligned_body,
                    projection.as_ref(),
                    window,
                    cx,
                )
                .unwrap_or_else(|| {
                    aligned_body
                        .take()
                        .unwrap_or_else(|| native(&self.editor, window, cx))
                }),
            )
            // Ordinary sources retain Base's leading gutter, including during popup renderer switches.
            .when(!aligned, |source| {
                source.child(editor_ui::gutter_separator(&self.editor))
            })
            // Install after the atomic authority: its native handler otherwise opens a menu at
            // the synthetic source coordinate. Base still owns right-click selection semantics.
            // Ordinary SourceFrames replace the weak projected handler too, including while a
            // completion popup temporarily uses BaseEditor and would otherwise retain an old hook.
            .child(projected_context_menu(
                self.editor.clone(),
                menu_builder,
                alignment.map_or_else(
                    || Rc::new(|bounds, _: &App| Some(bounds)) as _,
                    |(group, index)| group.pointer_projector(index),
                ),
                Rc::new(|menu, anchor, window, cx| menu.show(anchor, window, cx)),
            ))
            // Paint the drop caret after the text, using the current editor layout.
            .child(self.render_text_drag_caret_projected(projection.clone(), cx))
            .child(self.render_editor_markers_projected(projection, cx))
            // The aligned pane installs the same linked delegate inside its geometry bridge.
            .children((!aligned).then(|| self.render_linked_input()).flatten())
            .children(self.render_rename_prompt(cx))
            .into_any_element()
    }
}

/// Reuse Base's deferred context-menu callback, projecting only its saved mouse anchor.
fn projected_context_menu(
    editor: Entity<EditorState>,
    builder: SourceMenuBuilder,
    projection: Rc<dyn Fn(Bounds<Pixels>, &App) -> Option<Bounds<Pixels>>>,
    present: SourceMenuPresenter,
) -> gpui_kit::AnyElement {
    gpui_kit::canvas(
        |_, _, _| (),
        move |_, _, _, cx| {
            editor.update(cx, |editor, _| {
                editor.on_context_menu(Rc::new(move |_, _, position, window, cx| {
                    let native = Bounds::new(position, size(px(0.), px(0.)));
                    if let Some(anchor) = projection(native, cx) {
                        present(
                            builder(NativeMenu::new(), window, cx),
                            anchor.origin,
                            window,
                            cx,
                        );
                    }
                }));
            });
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
}
