//! Native execution declarations and typed process events; the API does not expose parser internals.
//!
//! `process` 1.8 adds `CloseStdin` for owned pipe processes. Queue all `Write` bytes first, then
//! close once to deliver EOF without terminating the child. Accepted writes drain in order before
//! the writer is dropped; later writes are rejected. Continue reading output until `Exited`, and
//! release the handle through normal resource cleanup. PTY processes reject this operation because
//! their interactive terminal input is not a separately owned pipe. The negotiated process capability,
//! execution permission and live instance ownership are checked before changing the writer state.
use serde::{Deserialize, Serialize};

/// `tools.inspect 1.0` describes one approved tool distribution without running its executable.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolMetadata {
    /// Canonical executable selected from this package's declared service and trusted tool paths.
    pub program: String,
    /// Canonical distribution directory; descriptive text does not grant a filesystem handle.
    pub root: String,
    /// Requested UTF-8 metadata, at most 16 KiB per file and 64 KiB in total.
    pub files: std::collections::BTreeMap<String, String>,
}

/// A caller may inspect its approved service or an explicitly selected local tool executable.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ToolSource {
    /// Reuse an owned declaration, including its search paths or already prepared dependency lease.
    Service { service: String },
    /// Resolve a bare tool name or absolute executable without starting it or granting a file handle.
    Program { program: String },
}

#[cfg(feature = "guest")]
/// Inspect `source`'s distribution, moving `root_levels` (0–1) above its executable directory.
/// `files` are bounded relative metadata paths. Requires tools.inspect and process.exec, plus
/// declared service consent when applicable; readonly language hooks never start a process.
pub fn inspect_tool(
    source: ToolSource,
    root_levels: u8,
    files: Vec<String>,
) -> Result<ToolMetadata, crate::api::Failure> {
    match crate::api::guest::request(crate::api::Operation::Process {
        operation: Operation::InspectTool {
            source,
            root_levels,
            files,
        },
    })? {
        crate::api::Value::ToolMetadata(value) => Ok(value),
        _ => Err(crate::api::Failure::new(
            crate::api::ErrorCode::InvalidState,
            "Unexpected tool metadata result",
        )),
    }
}

/// Installation approves this exact program and argument vector. Calls supply only its map key.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Service {
    /// dependencies 1.2: defer acquisition until a provided method explicitly requires this service.
    /// StartService reads the prepared cache; downloads never run inside a WASM callback.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub on_demand: bool,
    /// Empty supports every platform; otherwise packaging selects only matching OS/architecture.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub platforms: Vec<String>,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Ordered absolute globs (or `${HOME}/...`) locate approved tool distributions before PATH.
    #[serde(default)]
    pub search_paths: Vec<String>,
    /// Optional argument vector checks a candidate's exit status before selecting it; never a shell.
    #[serde(default)]
    pub check_args: Vec<String>,
    /// Approved dependency preparation supplies the private executable instead of searching PATH.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation: Option<crate::dependencies::Plan>,
}

impl Service {
    /// Relative paths containing directories could silently select code from an untrusted project.
    pub fn valid(&self) -> bool {
        !self.program.is_empty()
            && valid_platforms(&self.platforms)
            && self.program.len() <= 4096
            && !self.program.contains('\0')
            && (std::path::Path::new(&self.program).is_absolute()
                || !self.program.contains(['/', '\\', ':']))
            && valid_args(&self.args)
            && valid_args(&self.check_args)
            && self.search_paths.len() <= 32
            && self.search_paths.iter().all(|pattern| {
                !pattern.is_empty()
                    && pattern.len() <= 4096
                    && !pattern.chars().any(char::is_control)
                    && !pattern.contains("**")
                    && !pattern.split(['/', '\\']).any(|part| part == "..")
                    && (pattern
                        .strip_prefix("${HOME}/")
                        .is_some_and(|tail| !tail.is_empty() && !tail.contains("${"))
                        || (std::path::Path::new(pattern).is_absolute() && !pattern.contains("${")))
            })
    }
}

/// OS/architecture selectors are bounded declaration data, never paths or command arguments.
pub(crate) fn valid_platforms(platforms: &[String]) -> bool {
    platforms.len() <= 16
        && platforms.iter().all(|value| {
            !value.is_empty()
                && value.len() <= 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
        })
}

/// Both startup and probe arguments share the same bounded, literal argv contract.
fn valid_args(args: &[String]) -> bool {
    args.len() <= 128
        && args
            .iter()
            .all(|arg| arg.len() <= 32768 && !arg.contains('\0'))
        && args.iter().map(String::len).sum::<usize>() <= 65536
}

/// Byte streams keep stdout and stderr separate; PTY output is explicitly merged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stream {
    Stdout,
    Stderr,
    Pty,
}

/// Transport choice is independent of executable identity and does not grant execution authority.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Transport {
    Stdio,
    Pty {
        columns: u16,
        rows: u16,
        /// process 1.3: Windows asks the byte-stream consumer for its existing cursor position.
        /// The caller must answer the VT query through Write; other platforms reject true.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        inherit_cursor: bool,
    },
}

/// Lifecycle completion follows all output. Termination never implies rollback of native side effects.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Update {
    Output { stream: Stream, bytes: Vec<u8> },
    Exited { code: u32 },
    Terminated,
}

/// process 1.5 separates a supported normal-exit request from an explicit forceful tree termination.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitMode {
    #[default]
    Graceful,
    Force,
}

/// Start is separated from handle operations so service callers cannot override executable fields.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// process 1.9: obtain this instance's immutable package and private native-data paths.
    /// Requires process.exec and the named declared service's grant. These descriptive paths
    /// confer no WASI/file handles; native execution retains its existing OS authority and lifetime.
    NativeContext {
        service: String,
    },
    /// tools.inspect 1.0: read bounded metadata beside an owned declared service's executable.
    /// Does not probe, start or install anything; missing files and escaping paths fail explicitly.
    InspectTool {
        source: ToolSource,
        root_levels: u8,
        files: Vec<String>,
    },
    /// process 1.7: present this caller's existing PTY in the host's native terminal.
    /// Requires process.exec and ui.panels, including the original service caller's grants.
    /// This creates no process and transfers no ownership. The host routes input/resize to this
    /// exact resource, and an authenticated invocation may place it in its existing task tab.
    PresentTerminal {
        handle: crate::api::ResourceHandle,
        title: String,
    },
    /// process 1.7: append at most 16 KiB of explicitly decoded text to a resource's read-only view.
    /// Requires the same ownership, process.exec and ui.panels grants as PresentTerminal. Native
    /// stdout is never mirrored for this view; a protocol consumer decides what text is meaningful.
    TerminalOutput {
        handle: crate::api::ResourceHandle,
        title: String,
        bytes: Vec<u8>,
    },
    /// process 1.6: resolve a bare executable or absolute path without starting it.
    /// Requires process.exec and the active/trusted instance. This is observation only;
    /// Execute still checks permissions and resolves the path again.
    Resolve {
        program: String,
    },
    StartService {
        service: String,
    },
    Execute {
        program: String,
        args: Vec<String>,
        transport: Transport,
        /// process 1.2: an explicit absolute native directory under the already granted execution authority.
        /// Omission preserves the instance's workspace/private-data default; this grants no WASI access.
        #[serde(default)]
        cwd: Option<String>,
        /// process 1.4: caller-supplied environment applied over the environment the child inherits.
        ///
        /// The host neither reads nor logs these values. They belong to the program a caller asked for,
        /// so they cannot widen another instance's environment or reach a process it already started.
        #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
        env: std::collections::BTreeMap<String, String>,
    },
    Write {
        handle: crate::api::ResourceHandle,
        bytes: Vec<u8>,
    },
    /// process 1.8: close only the owned stdio input after all accepted writes have drained.
    /// Idempotent while the process remains live. Subsequent writes fail; stdout/stderr and the
    /// exit notification remain observable. PTYs reject this operation without sending control bytes.
    CloseStdin {
        handle: crate::api::ResourceHandle,
    },
    Resize {
        handle: crate::api::ResourceHandle,
        columns: u16,
        rows: u16,
    },
    /// process 1.5: request exit while retaining the handle until actual completion is observed.
    ///
    /// Graceful PTY exit delivers a console interrupt. Stdio has no universal exit protocol and
    /// returns `UnsupportedOperation`. Neither mode acknowledges the program's completed exit.
    RequestExit {
        handle: crate::api::ResourceHandle,
        #[serde(default)]
        mode: ExitMode,
    },
    Terminate {
        handle: crate::api::ResourceHandle,
    },
}

/// Paths for configuring an approved native service, scoped to one active package incarnation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeContext {
    /// Immutable installed resources belonging to this package version.
    pub assets: String,
    /// Host-owned private data directory; plugins must keep project metadata outside the workspace.
    pub data: String,
}

/// Resolve owned native paths without creating processes, downloading tools or granting file access.
/// Returns permission/capability/state errors when the instance or named service is unavailable.
#[cfg(target_arch = "wasm32")]
pub fn native_context(service: &str) -> Result<NativeContext, crate::api::Failure> {
    match crate::api::guest::request(crate::api::Operation::Process {
        operation: Operation::NativeContext {
            service: service.into(),
        },
    })? {
        crate::api::Value::NativeContext(context) => Ok(context),
        _ => Err(crate::api::Failure::new(
            crate::api::ErrorCode::InvalidRequest,
            "Expected native service context",
        )),
    }
}

/// Omitted/default options retain compatibility with older strict process decoders.
#[cfg(test)]
mod tests {
    use super::Transport;

    #[test]
    fn default_pty_omits_cursor_extension_and_opt_in_is_explicit() {
        let old = serde_json::json!({"kind":"pty", "columns":80, "rows":24});
        let value: Transport = serde_json::from_value(old.clone()).unwrap();
        assert!(matches!(
            value,
            Transport::Pty {
                inherit_cursor: false,
                ..
            }
        ));
        assert_eq!(serde_json::to_value(value).unwrap(), old);
        let requested = Transport::Pty {
            columns: 80,
            rows: 24,
            inherit_cursor: true,
        };
        assert_eq!(
            serde_json::to_value(requested).unwrap()["inherit_cursor"],
            true
        );
    }
}
