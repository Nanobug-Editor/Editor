//! Explicit navigation validates terminal output as web URLs or existing local source locations.

use super::*;
use gpui_kit::{MouseButton, MouseDownEvent};

/// A safe navigation destination; terminals cannot request arbitrary URL scheme execution.
#[derive(Debug, PartialEq, Eq)]
enum Destination {
    /// Credential-free HTTP(S) URL delegated to the ordinary system browser.
    Web(String),
    /// Existing local file and one-based line/column, relative to the session's cwd when necessary.
    File(PathBuf, u32, u32),
}

impl TerminalPanel {
    /// Ctrl-click opens a destination before selection or mouse reporting; plain clicks retain policy.
    /// Native bounds reject clicks on toolbars, side tabs and other panes.
    pub(super) fn navigation_pointer(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Left || !event.modifiers.control || event.modifiers.shift {
            return;
        }
        let point = self.visible_sessions().into_iter().find_map(|id| {
            self.grids
                .get(&id)
                .and_then(|grid| grid.read(cx).local_position(event.position))
                .map(|point| (id, point))
        });
        let Some((id, (x, y))) = point.filter(|(_, (x, y))| *x >= 8. && *y >= 8.) else {
            return;
        };
        self.active = Some(id);
        self.grid = self.grids.get(&id).cloned();
        if let Some(geometry) = self.geometry.get(&id).copied() {
            self.width = geometry.width;
            self.height = geometry.height;
            self.cell_width = geometry.cell_width;
            self.cell_height = geometry.cell_height;
        }
        if let Some(index) = self.active_index() {
            self.link = self.sessions[index].engine.link_at(
                ((y - 8.) / self.cell_height) as usize,
                ((x - 8.) / self.cell_width) as usize,
            );
        }
        if self.open_link(window, cx) {
            window.prevent_default();
            cx.stop_propagation();
        }
    }

    /// Validate the selected candidate at the moment of the explicit user activation.
    pub(super) fn open_link(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(index) = self.active_index() else {
            return false;
        };
        let Some(link) = &self.link else {
            return false;
        };
        let Some(destination) = destination(
            &link.target,
            Path::new(&self.sessions[index].cwd),
            &self.workspace,
        ) else {
            return false;
        };
        match destination {
            Destination::Web(url) => cx.open_url(&url),
            Destination::File(path, line, column) => {
                let Some(uri) = url::Url::from_file_path(&path)
                    .ok()
                    .and_then(|url| url.as_str().parse::<lsp_types::Uri>().ok())
                else {
                    return false;
                };
                let position = lsp_types::Position::new(line - 1, column - 1);
                return self
                    .parent
                    .update(cx, |app, cx| {
                        app.open_definition_uri(
                            &uri,
                            Some(lsp_types::Range::new(position, position)),
                            window,
                            cx,
                        )
                    })
                    .unwrap_or(false);
            }
        }
        true
    }
}

/// Parse printed/OSC 8 `target` against `cwd`, falling back to `workspace` for relative compiler paths.
/// File metadata is checked before navigation, so prose, missing files and remote file URLs are ignored.
fn destination(target: &str, cwd: &Path, workspace: &Path) -> Option<Destination> {
    if target.len() > 8192 || target.chars().any(char::is_control) {
        return None;
    }
    let target = target.trim_matches(['\'', '"']);
    let (path, line, column) = location(target);
    if let Ok(url) = url::Url::parse(target) {
        match url.scheme() {
            "http" | "https"
                if url.host_str().is_some()
                    && url.username().is_empty()
                    && url.password().is_none() =>
            {
                return Some(Destination::Web(url.into()));
            }
            "file"
                if url
                    .host_str()
                    .is_none_or(|host| host.eq_ignore_ascii_case("localhost")) =>
            {
                let path = url.to_file_path().ok()?;
                return path.is_file().then(|| Destination::File(path, 1, 1));
            }
            "http" | "https" | "file" => return None,
            // URL treats Windows drive letters as a scheme; only that explicit form is a path.
            scheme if scheme.len() == 1 && target.as_bytes().get(1) == Some(&b':') => {}
            // A bare compiler filename followed by a numeric location is also parsed as a scheme.
            // It can proceed only through the existing local-file validation below.
            _ if path != target => {}
            _ => return None,
        }
    }
    let path = path.trim_matches(['\'', '"']);
    let path = if let Some(relative) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\"))
    {
        dirs::home_dir()?.join(relative)
    } else {
        PathBuf::from(path)
    };
    let candidates = if path.is_absolute() {
        vec![path]
    } else {
        vec![cwd.join(&path), workspace.join(path)]
    };
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .and_then(|path| path.canonicalize().ok())
        .map(|path| Destination::File(path, line, column))
}

/// Split compiler `path:line[:column]` and `path(line[,column])` without consuming a drive colon.
fn location(target: &str) -> (&str, u32, u32) {
    if let Some(value) = target.strip_suffix(')')
        && let Some((path, suffix)) = value.rsplit_once('(')
    {
        let mut parts = suffix.split(',');
        if let Some(line) = parts
            .next()
            .and_then(|value| value.trim().parse::<u32>().ok())
        {
            let column = parts
                .next()
                .and_then(|value| value.trim().parse::<u32>().ok())
                .unwrap_or(1);
            if parts.next().is_none() {
                return (path, line.max(1), column.max(1));
            }
        }
    }
    let target = target.trim_end_matches([':', ')']);
    if let Some((prefix, last)) = target.rsplit_once(':')
        && let Ok(last) = last.parse::<u32>()
    {
        if let Some((path, line)) = prefix.rsplit_once(':')
            && let Ok(line) = line.parse::<u32>()
        {
            return (path, line.max(1), last.max(1));
        }
        return (prefix, last.max(1), 1);
    }
    (target, 1, 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compiler locations preserve drives, spaces, zero protection and both suffix conventions.
    #[test]
    fn compiler_locations() {
        assert_eq!(
            location("C:\\work\\main.rs:12:3"),
            ("C:\\work\\main.rs", 12, 3)
        );
        assert_eq!(location("src/main.rs(12,3)"), ("src/main.rs", 12, 3));
        assert_eq!(location("src/main.rs:0"), ("src/main.rs", 1, 1));
        assert_eq!(location("Cargo.toml"), ("Cargo.toml", 1, 1));
    }

    /// Opening a link is limited to safe web URLs and real files relative to this session.
    #[test]
    fn navigation_targets_are_validated() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("含 空格.rs"), "a\nb").unwrap();
        std::fs::write(root.path().join("native-terminal.txt"), "a\nb").unwrap();
        for target in ["native-terminal.txt:2:1", "native-terminal.txt:2"] {
            assert!(matches!(
                destination(target, root.path(), root.path()),
                Some(Destination::File(_, 2, 1))
            ));
        }
        assert!(matches!(
            destination("\"含 空格.rs\":2:1", root.path(), root.path()),
            Some(Destination::File(_, 2, 1))
        ));
        for value in [
            "javascript:alert(1)",
            "https://name:password@example.com",
            "file://remote/path",
            "missing.rs:1",
            "https://example.com/\n",
        ] {
            assert_eq!(
                destination(value, root.path(), root.path()),
                None,
                "{value}"
            );
        }
        assert!(matches!(
            destination("https://example.com/docs", root.path(), root.path()),
            Some(Destination::Web(_))
        ));
    }
}
