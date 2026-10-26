//! Replaced OLE registration must still deliver a standard GPUI ExternalPaths drop to existing consumers.
use super::*;
use gpui_kit::{
    Entity, ExternalPaths, InteractiveElement as _, ParentElement as _, Render, Styled as _, div,
};
use std::{
    path::PathBuf,
    sync::{Mutex, atomic::AtomicBool},
    time::{Duration, Instant},
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE,
};

/// Ordinary GPUI drop composition matches the existing editor/image and explorer consumer boundary.
pub(in crate::ui::plugin::canvas::scene_native_tests) struct Receiver {
    view: Entity<crate::ui::plugin::PluginView>,
    paths: Arc<Mutex<Vec<PathBuf>>>,
}
impl Receiver {
    pub(in crate::ui::plugin::canvas::scene_native_tests) fn new(
        view: Entity<crate::ui::plugin::PluginView>,
        paths: Arc<Mutex<Vec<PathBuf>>>,
    ) -> Self {
        Self { view, paths }
    }
}
impl Render for Receiver {
    fn render(
        &mut self,
        _: &mut gpui_kit::Window,
        _: &mut gpui_kit::Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        let paths = self.paths.clone();
        div().relative().size_full().child(self.view.clone()).child(
            div().absolute().inset_0().size_full().occlude().on_drop(
                move |external: &ExternalPaths, _, _| {
                    *paths.lock().unwrap() = external.paths().to_vec();
                },
            ),
        )
    }
}
struct File(PathBuf, Arc<AtomicUsize>);
impl drag::Provider for File {
    fn formats(&self) -> Vec<String> {
        vec![plugin_runtime::plugin_protocol::transfer::FILES_MIME.into()]
    }
    fn read(&self, _: &str) -> Result<drag::Data, drag::Error> {
        self.1.fetch_add(1, Ordering::AcqRel);
        Ok(drag::Data::Files(vec![self.0.clone()]))
    }
}
/// Exercise real DoDragDrop; no PlatformInput is injected by the test itself.
pub(in crate::ui::plugin::canvas::scene_native_tests) fn run(
    hwnd: usize,
    received: Arc<Mutex<Vec<PathBuf>>>,
) -> anyhow::Result<()> {
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("legacy-image.png");
    std::fs::write(&path, b"native external path")?;
    let reads = Arc::new(AtomicUsize::new(0));
    let provider = File(path.clone(), reads.clone());
    super::pointer(hwnd, 40, 40, MOUSEEVENTF_LEFTDOWN)?;
    let source = std::thread::spawn(move || {
        drag::start(
            hwnd as isize,
            Arc::new(provider),
            Arc::new(AtomicBool::new(true)),
        )
    });
    std::thread::sleep(Duration::from_millis(300));
    super::pointer(hwnd, 60, 45, MOUSEEVENTF_MOVE)?;
    std::thread::sleep(Duration::from_millis(200));
    super::pointer(hwnd, 60, 45, MOUSEEVENTF_LEFTUP)?;
    anyhow::ensure!(source.join().unwrap()? == drag::Outcome::Dropped);
    let deadline = Instant::now() + Duration::from_secs(3);
    while received.lock().unwrap().is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    anyhow::ensure!(
        *received.lock().unwrap() == vec![path],
        "legacy GPUI file consumer missed the real native drop (native reads: {})",
        reads.load(Ordering::Acquire)
    );
    anyhow::ensure!(reads.load(Ordering::Acquire) == 1);
    Ok(())
}
