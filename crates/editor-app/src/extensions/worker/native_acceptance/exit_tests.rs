//! Add one native lifecycle layer at a time to locate TLS failures after ordinary libtest returns.
use gpui_base::input::EditorState;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Context, Focusable as _, IntoElement, Render,
    Role, Styled, WeakEntity, Window, WindowBounds, WindowOptions, div, px, rgb, size,
    test::TestWindowExt as _,
};
use std::{
    cell::{Cell, RefCell},
    fs,
    process::{Command, Stdio},
    rc::Rc,
    time::{Duration, Instant},
};

const CHILD: &str =
    "extensions::worker::native_acceptance::exit_tests::native_empty_window_shutdown_child";
const SLICE_ENV: &str = "NANOBUG_NATIVE_EXIT_SLICE";

/// Carry the pre-quit HWND observation out of native callbacks so diagnostics never unwind through Win32.
type NativeRetirement = Rc<RefCell<Option<Result<bool, String>>>>;

/// Each child starts a fresh libtest/WindowsPlatform, so adding Root or native input cannot
/// inherit another slice's thread-local cache or renderer state.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Slice {
    Empty,
    KitRoot,
    NativeEditor,
}

impl Slice {
    fn name(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::KitRoot => "kit-root",
            Self::NativeEditor => "native-editor",
        }
    }

    /// The exact child remains directly runnable without private environment configuration.
    fn from_environment() -> Self {
        match std::env::var(SLICE_ENV).as_deref() {
            Ok("empty") | Err(_) => Self::Empty,
            Ok("kit-root") => Self::KitRoot,
            Ok("native-editor") => Self::NativeEditor,
            Ok(other) => panic!("unknown native exit slice: {other}"),
        }
    }
}

/// A TLS destructor may panic after libtest's test function returns. Only the ordinary child
/// process exit proves successful shutdown; a completed frame or an Application receipt cannot.
#[test]
#[ignore = "requires a real Windows desktop/D3D11; launches a bounded empty native child"]
fn native_empty_window_exits_without_tls_failure() {
    run_child(Slice::Empty);
}

/// Stop at the first failing layer. Root uses the production public Kit window constructor;
/// native input then adds the local EditorState frame, actual pointer/text and normal retirement.
#[test]
#[ignore = "requires a real Windows desktop/D3D11; launches ordered Root and native-input children"]
fn native_initialized_windows_exit_without_tls_failure() {
    for slice in [Slice::KitRoot, Slice::NativeEditor] {
        run_child(slice);
    }
}

/// Capture the final process exit rather than accepting receipts printed before TLS destruction.
fn run_child(slice: Slice) {
    let directory = tempfile::tempdir().unwrap();
    let log_path = directory.path().join("native-empty-window.log");
    let log = fs::File::create(&log_path).unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            CHILD,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .current_dir(directory.path())
        .env(SLICE_ENV, slice.name())
        .env("RUST_BACKTRACE", "full")
        .env("RUST_MIN_STACK", "8388608")
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            // Kill only this test's Child, then wait before its private files are released.
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "native {} child exceeded 30 seconds:\n{}",
                slice.name(),
                fs::read_to_string(&log_path).unwrap()
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let diagnostic = fs::read_to_string(log_path).unwrap();
    assert!(
        status.success(),
        "native {} shutdown failed with {status}:\n{diagnostic}",
        slice.name(),
    );
    for receipt in [
        "native exit: real frame delivered",
        "native exit: window entity dropped",
        "native exit: HWND destroyed before quit Ok(true)",
        "native exit: Application.run returned",
    ] {
        assert!(
            diagnostic.contains(receipt),
            "missing {receipt}:\n{diagnostic}"
        );
    }
    if slice == Slice::NativeEditor {
        for receipt in [
            "native exit: real native text received",
            "native exit: editor entity released",
        ] {
            assert!(
                diagnostic.contains(receipt),
                "missing {receipt}:\n{diagnostic}"
            );
        }
    }
}

/// Start the production Application/WindowsPlatform from ordinary libtest. Only the selected
/// layer is enabled; no slice creates a Worker, EditorApp, workspace or plugin package.
#[test]
#[ignore = "spawn through the bounded native empty-window parent, or run exact with matching PDB"]
fn native_empty_window_shutdown_child() {
    let slice = Slice::from_environment();
    let frame_seen = Rc::new(Cell::new(false));
    let root_dropped = Rc::new(Cell::new(false));
    let editor_dropped = Rc::new(Cell::new(false));
    let native_retired: NativeRetirement = Rc::new(RefCell::new(None));
    let completed = frame_seen.clone();
    let released = root_dropped.clone();
    let editor_release = editor_dropped.clone();
    let native_observed = native_retired.clone();
    eprintln!("native exit: Application.run begin; slice={}", slice.name());
    let mut application = gpui_kit::application().with_quit_mode(gpui_kit::QuitMode::Explicit);
    if slice != Slice::Empty {
        application = application.with_assets(crate::AppAssets);
    }
    application.run(move |cx| {
        if slice != Slice::Empty {
            gpui_kit::init(cx);
            crate::ui::typography::init(cx);
            crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(true), cx);
            cx.set_reduce_motion(true);
        }
        let (handle, editor) = open_window(slice, released, editor_release, cx);
        handle
            .update(cx, |_, window, cx| {
                // Windows invokes queued next-frame callbacks before that frame's draw. Paint
                // now, as the full harness does, so a callback cannot retire an unpainted root.
                window.draw(cx).clear(cx);
                window.on_next_frame(move |window, cx| {
                    completed.set(true);
                    eprintln!("native exit: real frame delivered");
                    if let Some(editor) = editor {
                        native_input(handle, editor, native_observed, window, cx);
                    } else {
                        retire_window(handle, native_observed, cx);
                    }
                });
                window.refresh();
            })
            .unwrap();
        cx.activate(true);
    });
    eprintln!("native exit: Application.run returned");
    assert!(frame_seen.get(), "no real native frame was delivered");
    assert!(root_dropped.get(), "window still owns its root entity");
    let native_retired = native_retired
        .borrow_mut()
        .take()
        .expect("native HWND retirement was not observed");
    assert!(
        matches!(native_retired, Ok(true)),
        "original HWND was not destroyed before cx.quit(): {native_retired:?}"
    );
    if slice == Slice::NativeEditor {
        assert!(editor_dropped.get(), "native input still owns EditorState");
    }
}

/// Kit's public constructor supplies its real Base Root. Drop its returned content handle now,
/// leaving the native window as the sole owner before the ordinary shutdown path begins.
fn open_window(
    slice: Slice,
    released: Rc<Cell<bool>>,
    editor_released: Rc<Cell<bool>>,
    cx: &mut App,
) -> (AnyWindowHandle, Option<WeakEntity<EditorState>>) {
    let mut options = if slice == Slice::Empty {
        WindowOptions::default()
    } else {
        crate::ui::controls::window::TitleBar::window_options()
    };
    options.window_bounds = Some(WindowBounds::Windowed(Bounds::centered(
        None,
        size(px(640.), px(480.)),
        cx,
    )));
    options.show = true;
    options.focus = true;
    options.inactive_frame_interval = None;
    let slot = Rc::new(RefCell::new(None));
    let observed = slot.clone();
    let build = move |window: &mut Window, cx: &mut App| {
        window.set_window_title("Nanobug — Native shutdown regression");
        let editor = (slice == Slice::NativeEditor).then(|| {
            let editor = cx.new(|cx| {
                cx.on_release(move |_, _| {
                    editor_released.set(true);
                    eprintln!("native exit: editor entity released");
                })
                .detach();
                EditorState::new(window, cx).line_number(false)
            });
            editor.update(cx, |editor, cx| {
                editor.set_value("native shutdown\n", window, cx)
            });
            editor
        });
        *observed.borrow_mut() = editor.as_ref().map(|editor| editor.downgrade());
        cx.new(|_| ExitWindow { released, editor })
    };
    let handle = if slice == Slice::Empty {
        cx.open_window(options, build).unwrap().into()
    } else {
        let (handle, content) = gpui_kit::open_window(options, cx, build).unwrap();
        drop(content);
        handle
    };
    let editor = slot.borrow_mut().take();
    (handle, editor)
}

/// The local InputBase receives one real pointer gesture and native input only after its next
/// painted frame proves focus. Weak captures never keep the editor alive through retirement;
/// `native_retirement` carries the eventual pre-quit HWND observation back to the test.
fn native_input(
    handle: AnyWindowHandle,
    editor: WeakEntity<EditorState>,
    native_retirement: NativeRetirement,
    window: &mut Window,
    cx: &mut App,
) {
    let source = gpui_base::test_support::snapshots(window)
        .into_iter()
        .find(|node| node.visible() && node.role() == Some(Role::TextInput))
        .expect("native editor frame was not painted");
    super::input::click(window, &source, cx).unwrap();
    window.on_next_frame(move |window, cx| {
        // Publish the newly focused editor's real platform input handler before delivering text.
        window.draw(cx).clear(cx);
        let native = editor
            .upgrade()
            .expect("editor disappeared before native input");
        assert!(native.read(cx).focus_handle(cx).is_focused(window));
        window.input("退出验证", cx);
        drop(native);
        window.on_next_frame(move |window, cx| {
            let native = editor
                .upgrade()
                .expect("editor disappeared after native input");
            assert!(native.read(cx).value().contains("退出验证"));
            eprintln!("native exit: real native text received");
            drop(native);
            window.blur(cx);
            retire_input_frames(handle, 2, native_retirement, window);
        });
        window.refresh();
    });
    window.refresh();
}

/// Paint `remaining` real post-blur frames for `handle`, then preserve the native retirement receipt.
fn retire_input_frames(
    handle: AnyWindowHandle,
    remaining: u8,
    native_retirement: NativeRetirement,
    window: &mut Window,
) {
    window.on_next_frame(move |window, cx| {
        // This callback precedes the platform draw, so explicitly paint both post-blur buffers
        // before the second callback is allowed to remove the native window.
        window.draw(cx).clear(cx);
        if remaining == 1 {
            retire_window(handle, native_retirement, cx);
        } else {
            retire_input_frames(handle, remaining - 1, native_retirement, window);
        }
    });
    window.refresh();
}

/// Remove `handle` after its frame callback, then await the original HWND's actual destruction.
/// Failures go into `native_retirement`; assertions run only after Application returns.
fn retire_window(handle: AnyWindowHandle, native_retirement: NativeRetirement, cx: &mut App) {
    cx.defer(move |cx| {
        let native = handle
            .update(cx, |_, window, _| {
                let native = super::windows::OwnedWindow::for_window(window);
                window.remove_window();
                native
            })
            .map_err(|error| format!("GPUI window removal failed: {error:#}"))
            .and_then(|native| native.map_err(|error| format!("{error:#}")));
        eprintln!("native exit: GPUI window removal {native:?}");
        cx.spawn(async move |cx| {
            let destroyed = match native {
                Ok(window) => super::windows::wait_for_retirement(&[window], cx)
                    .await
                    .map(|()| !window.is_alive())
                    .map_err(|error| format!("{error:#}")),
                Err(error) => Err(error),
            };
            eprintln!("native exit: HWND destroyed before quit {destroyed:?}");
            *native_retirement.borrow_mut() = Some(destroyed);
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
}

/// The plain container stays unchanged; only the native-editor slice mounts one original state.
struct ExitWindow {
    released: Rc<Cell<bool>>,
    editor: Option<gpui_kit::Entity<EditorState>>,
}

impl Render for ExitWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(editor) = &self.editor {
            crate::ui::controls::window_editor(editor, cx)
        } else {
            div().size_full().bg(rgb(0x101010)).into_any_element()
        }
    }
}

impl Drop for ExitWindow {
    fn drop(&mut self) {
        self.released.set(true);
        eprintln!("native exit: window entity dropped");
    }
}
