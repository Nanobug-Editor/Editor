//! Native input, read-only history, one-step Undo and create-only recovery share the real editor.

use super::*;

pub(super) async fn exercise(h: &Native, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.frame(cx).await?;
    let dpi = h.window.update(cx, |_, window, _| {
        windows::DpiRequest::for_window(window, h.case.percent)
    })??;
    dpi.apply();
    h.frame(cx).await?;
    let editor = cx.update(|cx| h.app.read(cx).editor.clone());
    let path = cx.update(|cx| h.app.read(cx).tabs[0].path().to_owned());
    h.window.update(cx, |_, window, cx| {
        editor.focus_handle(cx).focus(window, cx);
        editor.update(cx, |editor, cx| {
            editor.replace_text_in_range(None, "saved ", window, cx)
        });
    })?;
    h.until(
        "native text change delivered",
        |app, _| app.tabs[0].is_dirty(),
        cx,
    )
    .await?;
    h.window.update(cx, |_, window, cx| {
        editor_ui::testing::press_key(window, "ctrl-s", cx)
    })?;
    h.until("native save", |app, _| !app.tabs[0].is_dirty(), cx)
        .await?;
    let disk_before_restore = fs::read(&path)?;
    h.window.update(cx, |_, window, cx| {
        editor.update(cx, |editor, cx| {
            editor.replace_text_in_range(None, "unsaved ", window, cx)
        });
    })?;
    h.frame(cx).await?;
    let before_restore = cx.update(|cx| editor.read(cx).value().to_string());
    entrypoints::exercise(h, &path, cx).await?;
    h.window.update(cx, |_, window, cx| {
        h.app.update(cx, |app, cx| {
            app.open_file_history(path.clone(), window, cx)
        })
    })?;
    h.until(
        "two saved versions",
        |app, _| {
            app.history
                .panel
                .as_ref()
                .is_some_and(|panel| panel.rows.len() == 2)
        },
        cx,
    )
    .await?;
    h.click("history-item-1", cx).await?;
    h.until(
        "read-only history preview",
        |app, _| {
            app.history
                .panel
                .as_ref()
                .is_some_and(|panel| panel.preview.is_some())
        },
        cx,
    )
    .await?;
    let preview = cx.update(|cx| {
        h.app
            .read(cx)
            .history
            .panel
            .as_ref()
            .unwrap()
            .preview
            .clone()
            .unwrap()
    });
    ensure!(
        !cx.update(|cx| preview.read(cx).is_editable()),
        "historical preview is writable"
    );
    h.click("history-compare", cx).await?;
    comparison_visible(h, &before_restore, cx).await?;
    h.capture("comparison.png", cx)?;
    h.click("history-restore", cx).await?;
    h.until(
        "restore applied to original editor",
        |app, _| app.history.panel.is_none(),
        cx,
    )
    .await?;
    ensure!(
        fs::read(&path)? == disk_before_restore,
        "restore wrote the disk before Save"
    );
    h.window.update(cx, |_, window, cx| {
        editor_ui::testing::press_key(window, "ctrl-z", cx)
    })?;
    h.frame(cx).await?;
    ensure!(
        cx.update(|cx| editor.read(cx).value().as_ref() == before_restore),
        "native Undo lost unsaved content"
    );
    h.window.update(cx, |_, window, cx| {
        editor_ui::testing::press_key(window, "ctrl-y", cx)
    })?;
    h.frame(cx).await?;
    ensure!(
        cx.update(|cx| editor.read(cx).value().as_ref() == "原始内容\r\nsecond line\nthird line\n"),
        "native Redo failed"
    );
    h.evidence.borrow_mut()["observations"]["native_save_restore_undo_redo_disk_unchanged"] =
        true.into();
    ime(h, &editor, cx).await?;
    h.window.update(cx, |_, window, cx| {
        editor_ui::testing::press_key(window, "ctrl-s", cx)
    })?;
    h.until(
        "restored document saved",
        |app, _| !app.tabs[0].is_dirty(),
        cx,
    )
    .await?;
    deleted(h, &path, cx).await?;
    timings::save_latency(h, cx).await?;
    timings::diff_preparation(h, cx).await?;
    large_preview(h, cx).await?;
    metadata(h, cx).await?;
    h.window.update(cx, |_, window, cx| {
        h.app
            .update(cx, |app, cx| app.open_workspace_history(window, cx))
    })?;
    h.until(
        "workspace metadata",
        |app, _| {
            app.history
                .panel
                .as_ref()
                .is_some_and(|panel| !panel.rows.is_empty())
        },
        cx,
    )
    .await?;
    h.click("history-clear-all", cx).await?;
    h.frame(cx).await?;
    h.capture("clear-confirmation.png", cx)?;
    h.click("history-clear-cancel", cx).await?;
    h.click("history-clear-all", cx).await?;
    h.click("history-clear-confirm", cx).await?;
    h.until(
        "confirmed history clear",
        |app, _| {
            app.history
                .panel
                .as_ref()
                .is_some_and(|panel| panel.rows.is_empty())
        },
        cx,
    )
    .await?;
    ensure!(
        fs::read(&path)?.starts_with("原始内容".as_bytes()),
        "history clear modified project file"
    );
    h.evidence.borrow_mut()["observations"]["clear_preview_cancel_confirm"] = true.into();
    h.click("history-close", cx).await?;
    settings(h, cx).await?;
    Ok(())
}

/// Background diff and native layout must both finish before comparison counts as visible evidence.
async fn comparison_visible(h: &Native, current: &str, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        h.frame(cx).await?;
        let (panes, glyphs) = h.window.update(cx, |_, window, cx| {
            (
                [t!("history.historical"), t!("history.current")]
                    .map(|label| input::source(window, label.as_ref()).ok()),
                crate::editor::history::tests::comparison_painted_ranges(
                    h.app.read(cx),
                    [0..3, 0..1],
                    cx,
                ),
            )
        })?;
        if let ([Some(left), Some(right)], Some(glyphs)) = (&panes, glyphs) {
            ensure!(
                left.role() == Some(Role::Document),
                "history comparison is writable"
            );
            ensure!(
                right.role() == Some(Role::TextInput),
                "current comparison is readonly"
            );
            ensure!(
                left.value() == Some("原始内容\r\nsecond line\nthird line\n")
                    && right.value() == Some(current),
                "comparison does not expose the selected version and current buffer"
            );
            let bounds = [left.bounds(), right.bounds()];
            if bounds
                .iter()
                .all(|bounds| bounds.size.width > px(100.) && bounds.size.height > px(100.))
                && bounds.iter().zip(glyphs).all(|(pane, glyph)| {
                    pane.intersects(&glyph)
                        && glyph.size.width > px(0.)
                        && glyph.size.height > px(0.)
                })
            {
                h.evidence.borrow_mut()["observations"]["comparison_visible_sources"] = serde_json::json!({
                    "historical_readonly": true, "current_writable": true,
                    "values_match_sources": true, "bounds": format!("{bounds:?}"),
                    "painted_glyphs": format!("{glyphs:?}")
                });
                return Ok(());
            }
        }
        ensure!(
            Instant::now() < deadline,
            "native comparison sources did not become visible: {panes:?}; glyphs: {glyphs:?}"
        );
    }
}

/// A real fifty-row native page follows the same stable cursors and literal filter as the store.
async fn metadata(h: &Native, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let (worker, root) = cx.update(|cx| {
        let app = h.app.read(cx);
        (
            app.history.worker.clone().unwrap(),
            app.workspace.root().to_owned(),
        )
    });
    worker
        .call(0, move |history| {
            let path = root.join("history-pagination.txt");
            let now = crate::editor::history::now_ms();
            for index in 0..113 {
                history.capture(editor_history::Capture {
                    file: None,
                    path: path.clone(),
                    workspace: Some(root.clone()),
                    contents: format!("version {index}").into_bytes(),
                    reason: editor_history::Reason::Saved,
                    at_ms: now,
                })?;
            }
            fs::write(path, "version 112")?;
            Ok(())
        })
        .await?;
    let started = Instant::now();
    h.window.update(cx, |_, window, cx| {
        h.app
            .update(cx, |app, cx| app.open_workspace_history(window, cx))
    })?;
    h.until(
        "first fifty metadata rows",
        |app, _| {
            app.history
                .panel
                .as_ref()
                .is_some_and(|panel| panel.rows.len() == 50)
        },
        cx,
    )
    .await?;
    let first_visible_ms = started.elapsed().as_secs_f64() * 1000.;
    for (filter, expected) in [("no-such-history-path", 0), ("pagination", 50)] {
        h.window.update(cx, |_, window, cx| {
            let field = snapshots(window)
                .into_iter()
                .find(|node| {
                    node.visible()
                        && node.role() == Some(Role::TextInput)
                        && node.label() == Some(t!("history.filter").as_ref())
                })
                .context("native history filter not painted")?;
            input::click(window, &field, cx)
        })??;
        h.frame(cx).await?;
        h.window.update(cx, |_, window, cx| {
            editor_ui::testing::press_key(window, "ctrl-a", cx);
            editor_ui::testing::input_text(window, filter, cx);
        })?;
        h.until(
            "filtered metadata rows",
            |app, _| {
                app.history
                    .panel
                    .as_ref()
                    .is_some_and(|panel| panel.rows.len() == expected)
            },
            cx,
        )
        .await?;
    }
    h.click("history-next", cx).await?;
    h.until(
        "second fifty metadata rows",
        |app, _| {
            app.history
                .panel
                .as_ref()
                .is_some_and(|panel| panel.rows.len() == 50)
        },
        cx,
    )
    .await?;
    h.click("history-next", cx).await?;
    h.until(
        "last thirteen metadata rows",
        |app, _| {
            app.history
                .panel
                .as_ref()
                .is_some_and(|panel| panel.rows.len() == 13)
        },
        cx,
    )
    .await?;
    h.capture("filtered-third-page.png", cx)?;
    h.evidence.borrow_mut()["observations"]["native_filter_cursor_pages"] =
        serde_json::json!({"rows":113,"pages":[50,50,13],"first_visible_ms":first_visible_ms});
    Ok(())
}

/// The actual settings dialog writes only the selected profile and keeps its controls usable at this DPI.
async fn settings(h: &Native, cx: &mut AsyncApp) -> anyhow::Result<()> {
    cx.update(|cx| {
        h.app.update(cx, |app, _| {
            app.settings_section = crate::app::SettingsSection::History
        })
    });
    h.click("open-settings", cx).await?;
    h.until(
        "native history settings window",
        |app, _| app.dialog_window.is_some(),
        cx,
    )
    .await?;
    let dialog: AnyWindowHandle = cx.update(|cx| h.app.read(cx).dialog_window.unwrap().into());
    h.frame_for(dialog, cx).await?;
    let dpi = dialog.update(cx, |_, window, _| {
        windows::DpiRequest::for_window(window, h.case.percent)
    })??;
    dpi.apply();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        h.frame_for(dialog, cx).await?;
        let ready = dialog.update(cx, |_, window, _| {
            snapshots(window).into_iter().any(|node| {
                node.visible()
                    && input::named(&node, "history-settings-apply")
                    && node.disabled() != Some(true)
            })
        })?;
        if ready {
            break;
        }
        ensure!(
            Instant::now() < deadline,
            "history settings did not finish loading"
        );
    }
    let screenshot = dialog.update(cx, |_, window, cx| {
        window.draw(cx).clear(cx);
        windows::capture(window, &h.output.join("settings.png"))
    })??;
    h.evidence.borrow_mut()["screenshots"]["settings.png"] = screenshot;
    dialog.update(cx, |_, window, cx| {
        let checkbox = snapshots(window)
            .into_iter()
            .find(|node| node.visible() && input::named(node, "history-enabled"))
            .context("history enabled checkbox missing")?;
        input::click(window, &checkbox, cx)
    })??;
    h.click_for(dialog, "history-settings-apply", cx).await?;
    let policy_path = cx.update(|cx| {
        h.app
            .read(cx)
            .history
            .paths
            .as_ref()
            .unwrap()
            .settings
            .clone()
    });
    loop {
        h.frame_for(dialog, cx).await?;
        let disabled = fs::read(&policy_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<editor_history::Policy>(&bytes).ok())
            .is_some_and(|policy| !policy.enabled);
        if disabled {
            break;
        }
        ensure!(
            Instant::now() < deadline,
            "native settings did not persist the profile policy"
        );
    }
    h.evidence.borrow_mut()["observations"]["native_settings_profile_toggle_persisted"] =
        true.into();
    let owned = dialog.update(cx, |_, window, _| windows::OwnedWindow::for_window(window))??;
    dialog.update(cx, |_, window, _| window.remove_window())?;
    windows::wait_for_retirement(&[owned], cx).await?;
    Ok(())
}

/// Public native composition callbacks verify preedit rendering/cancellation, not the OS candidate popup.
async fn ime(h: &Native, editor: &Entity<EditorState>, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let before = cx.update(|cx| editor.read(cx).value().to_string());
    h.window.update(cx, |_, window, cx| {
        editor.update(cx, |editor, cx| {
            editor.set_selected_range(0..0, cx);
            editor.replace_and_mark_text_in_range(None, "拼😀", Some(0..3), window, cx);
        })
    })?;
    h.frame(cx).await?;
    h.capture("ime-preedit.png", cx)?;
    h.window.update(cx, |_, window, cx| {
        editor.update(cx, |editor, cx| {
            editor.replace_and_mark_text_in_range(None, "", None, window, cx)
        })
    })?;
    h.frame(cx).await?;
    ensure!(
        cx.update(|cx| editor.read(cx).value().as_ref() == before),
        "IME cancellation changed committed text"
    );
    h.evidence.borrow_mut()["observations"]["native_ime_preedit_cancel"] = true.into();
    Ok(())
}

/// Delete is protected through the actual modal; its history recovers as dirty, never auto-saved content.
async fn deleted(h: &Native, path: &Path, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.window.update(cx, |_, window, cx| {
        h.app.update(cx, |app, cx| {
            app.close_tab(path.to_owned(), window, cx);
            app.start_explorer_delete(path.to_owned(), window, cx);
        })
    })?;
    h.click("explorer-delete-confirm-button", cx).await?;
    h.until(
        "protected file deletion",
        |app, _| app.explorer_delete.is_none() && !path.exists(),
        cx,
    )
    .await?;
    h.window.update(cx, |_, window, cx| {
        h.app
            .update(cx, |app, cx| app.open_workspace_history(window, cx))
    })?;
    h.click("history-category-1", cx).await?;
    h.until(
        "deleted file history",
        |app, _| {
            app.history
                .panel
                .as_ref()
                .is_some_and(|panel| !panel.rows.is_empty())
        },
        cx,
    )
    .await?;
    h.click("history-item-0", cx).await?;
    h.until(
        "deleted file preview",
        |app, _| {
            app.history
                .panel
                .as_ref()
                .is_some_and(|panel| panel.preview.is_some())
        },
        cx,
    )
    .await?;
    h.capture("deleted-preview.png", cx)?;
    h.click("history-restore", cx).await?;
    h.until(
        "deleted file pending Save",
        |app, _| app.history.panel.is_none() && app.tabs.iter().any(OpenTab::is_dirty),
        cx,
    )
    .await?;
    ensure!(!path.exists(), "deleted recovery auto-saved");
    h.capture("recovered-pending.png", cx)?;
    h.window.update(cx, |_, window, cx| {
        editor_ui::testing::press_key(window, "ctrl-s", cx)
    })?;
    h.until(
        "create-only recovery saved",
        |app, _| path.exists() && !app.tabs.iter().any(OpenTab::is_dirty),
        cx,
    )
    .await?;
    h.evidence.borrow_mut()["observations"]["protected_delete_dirty_restore_create_only_save"] =
        true.into();
    Ok(())
}

/// Include actual 8 MiB native preview/layout timing, separate from storage-only decode measurements.
async fn large_preview(h: &Native, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let root = cx.update(|cx| h.app.read(cx).workspace.root().to_owned());
    let path = root.join("large.txt");
    let bytes = vec![b'x'; 8 * 1024 * 1024];
    cx.background_executor()
        .spawn({
            let path = path.clone();
            async move { fs::write(path, bytes) }
        })
        .await?;
    let started = Instant::now();
    h.window.update(cx, |_, window, cx| {
        h.app.update(cx, |app, cx| {
            app.open_file(path.clone(), window, cx);
            app.open_file_history(path.clone(), window, cx);
        })
    })?;
    h.until(
        "8 MiB metadata visible",
        |app, _| {
            app.history
                .panel
                .as_ref()
                .is_some_and(|panel| !panel.rows.is_empty())
        },
        cx,
    )
    .await?;
    let list_ms = started.elapsed().as_secs_f64() * 1000.;
    let started = Instant::now();
    h.click("history-item-0", cx).await?;
    h.until(
        "8 MiB preview visible",
        |app, _| {
            app.history
                .panel
                .as_ref()
                .is_some_and(|panel| panel.preview.is_some())
        },
        cx,
    )
    .await?;
    let preview_ms = started.elapsed().as_secs_f64() * 1000.;
    h.capture("8mib-preview.png", cx)?;
    h.click("history-compare", cx).await?;
    h.frame(cx).await?;
    h.capture("8mib-diff-degraded.png", cx)?;
    h.frame(cx).await?;
    let started = Instant::now();
    let cancel_dispatch_ms = h.window.update(cx, |_, window, cx| {
        let node = snapshots(window)
            .into_iter()
            .find(|node| node.visible() && input::named(node, "history-close"))
            .context("history close control missing")?;
        let dispatch = Instant::now();
        input::click(window, &node, cx)?;
        ensure!(
            h.app.read(cx).history.panel.is_none(),
            "cancel did not retire history state"
        );
        Ok::<_, anyhow::Error>(dispatch.elapsed().as_secs_f64() * 1000.)
    })??;
    h.frame(cx).await?;
    h.evidence.borrow_mut()["observations"]["8mib_single_line"] = serde_json::json!({
        "list_visible_ms": list_ms, "preview_visible_ms": preview_ms,
        "close_ms": started.elapsed().as_secs_f64() * 1000., "cancel_dispatch_ms":cancel_dispatch_ms,
        "bytes": 8 * 1024 * 1024
    });
    Ok(())
}
