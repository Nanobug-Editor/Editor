//! Native format actions and input use the same production EditorState as the visible source.
use super::*;
use std::io::Write as _;

impl Harness {
    /// Wait for installed Vue primary/format declarations, without manually initializing an LSP connection.
    pub(in super::super) async fn catalog(&self, cx: &mut AsyncApp) -> anyhow::Result<()> {
        self.wait(
            "Vue format package catalog",
            30,
            |app, _| {
                preparation(app, &self.actor)?;
                Ok(app
                    .providers
                    .language_servers()
                    .get("vue")
                    .is_some_and(Option::is_some)
                    && app
                        .providers
                        .formatters()
                        .get("vue")
                        .is_some_and(Option::is_some))
            },
            cx,
        )
        .await
    }

    /// Open a normal native session and replace its original text through the installed input handler.
    pub(in super::super) async fn dirty(
        &self,
        path: &Path,
        value: &str,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        self.main.update(cx, |_, window, cx| {
            window.activate_window();
            self.app
                .update(cx, |app, cx| app.open_file(path.to_owned(), window, cx));
            self.app.read(cx).editor.clone().update(cx, |editor, cx| {
                let length = editor.text().len();
                editor.set_selected_range(0..length, cx);
                editor.replace_text_in_range(None, value, window, cx);
                editor.focus(window, cx);
                cx.notify();
            });
            window.draw(cx).clear(cx);
        })?;
        self.wait(
            "native dirty formatting input",
            15,
            |app, cx| Ok(app.editor.read(cx).text().to_string() == value),
            cx,
        )
        .await
    }

    /// Set only the actual native byte selection/focus before dispatching a registered source action.
    pub(in super::super) fn select(
        &self,
        range: Range<usize>,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        self.main.update(cx, |_, window, cx| {
            self.app.read(cx).editor.clone().update(cx, |editor, cx| {
                editor.set_selected_range(range, cx);
                editor.focus(window, cx);
                cx.notify();
            });
            window.draw(cx).clear(cx);
        })?;
        Ok(())
    }

    /// Clear only old presentation feedback, then enter the public named action as a user does.
    pub(in super::super) fn dispatch(
        &self,
        name: &'static str,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        self.main
            .update(cx, |_, window, cx| -> anyhow::Result<()> {
                self.app.update(cx, |app, _| app.status.clear());
                let action = cx
                    .build_action(name, None)
                    .context("registered native formatting action")?;
                window.dispatch_action(action, cx);
                Ok(())
            })??;
        Ok(())
    }

    /// Observe the actual native postimage under the original 180-second cold-format observer budget.
    pub(in super::super) async fn changed(
        &self,
        before: &str,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<String> {
        self.wait(
            "installed formatter native output",
            180,
            |app, cx| {
                preparation(app, &self.actor)?;
                Ok(app.editor.read(cx).text().to_string() != before)
            },
            cx,
        )
        .await?;
        self.text(cx)
    }

    /// One ordinary native Undo must restore the dirty preimage, without consuming the user's paste.
    pub(in super::super) async fn undo(
        &self,
        original: &str,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        self.main.update(cx, |_, window, cx| {
            window.dispatch_action(Box::new(gpui_base::input::Undo), cx)
        })?;
        self.wait(
            "one native formatting Undo",
            15,
            |app, cx| Ok(app.editor.read(cx).text().to_string() == original),
            cx,
        )
        .await
    }

    /// Read only the original editor's text; the harness has no mutable analysis copy.
    pub(in super::super) fn text(&self, cx: &mut AsyncApp) -> anyhow::Result<String> {
        Ok(self.main.update(cx, |_, _, cx| {
            self.app.read(cx).editor.read(cx).text().to_string()
        })?)
    }

    /// An empty standard LSP edit list is explicit no-change feedback, never a whole-document fallback.
    pub(in super::super) async fn rejected(
        &self,
        original: &str,
        empty: bool,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        self.wait(
            "range no-applicable-change feedback",
            30,
            |app, cx| {
                preparation(app, &self.actor)?;
                ensure!(
                    app.editor.read(cx).text().to_string() == original,
                    "unsupported range formatted the whole SFC"
                );
                Ok(app.status
                    == if empty {
                        t!("editor.format_selection_empty").to_string()
                    } else {
                        t!("editor.format_no_changes").to_string()
                    })
            },
            cx,
        )
        .await?;
        ensure!(
            self.text(cx)? == original,
            "rejected range changed native text"
        );
        Ok(())
    }

    /// These fixtures remain alive through the scenario and checked actor shutdown.
    pub(in super::super) fn external(
        &self,
        path: &Path,
        suffix: &str,
        contents: &str,
    ) -> anyhow::Result<PathBuf> {
        fs::create_dir_all(path)?;
        let mut file = tempfile::Builder::new()
            .prefix("s3-format-external-")
            .suffix(suffix)
            .tempfile_in(path)?;
        file.write_all(contents.as_bytes())?;
        file.flush()?;
        let path = file.path().to_owned();
        self.external.borrow_mut().push(file);
        Ok(path)
    }

    /// Same-turn input uses the original 350ms observation window; this is not wire-ingress evidence.
    pub(in super::super) async fn settle(&self, cx: &mut AsyncApp) -> anyhow::Result<()> {
        let deadline = Instant::now() + Duration::from_millis(350);
        while Instant::now() < deadline {
            self.frame(cx)?;
            cx.background_executor()
                .timer(Duration::from_millis(10))
                .await;
        }
        Ok(())
    }
}
