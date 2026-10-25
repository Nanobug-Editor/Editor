//! A separate Windows UI Automation client verifies native semantics without reading host maps.
use super::*;

/// Resolve only this actual test window; the client independently rechecks its process ownership.
pub(super) fn owned_handle(window: &Window) -> anyhow::Result<usize> {
    let raw = raw_window_handle::HasWindowHandle::window_handle(window)
        .map_err(|error| anyhow::anyhow!("UIA handle: {error:?}"))?;
    let raw_window_handle::RawWindowHandle::Win32(handle) = raw.as_raw() else {
        anyhow::bail!("UIA requires Windows");
    };
    Ok(handle.hwnd.get() as usize)
}
/// Run the OS client outside the UI thread so WM_GETOBJECT and native actions remain dispatchable.
pub(super) fn probe(handle: usize) -> anyhow::Result<serde_json::Value> {
    let output = std::process::Command::new(std::env::current_exe()?)
        .args([
            "native_canvas_semantics_os_client",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("NANOBUG_CANVAS_UIA_HWND", handle.to_string())
        .env("NANOBUG_CANVAS_UIA_OWNER", std::process::id().to_string())
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "OS UIA probe: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout)?;
    let result = stdout
        .lines()
        .find_map(|line| line.split_once("NANOBUG_UIA_RESULT=").map(|(_, json)| json))
        .ok_or_else(|| anyhow::anyhow!("UIA result missing: {stdout}"))?;
    let result = serde_json::from_str(result)?;
    eprintln!("actual Windows UIA result: {result}");
    Ok(result)
}
/// Observe native SetFocus/Invoke/SetValue through the same scoped CanvasInput stream as all input.
pub(super) fn verify(
    result: &serde_json::Value,
    events: &[ui::UiEvent],
    scale: f32,
) -> anyhow::Result<()> {
    use protocol::ui::{CanvasInputKind, CanvasSemanticAction};
    let items = result["items"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("UIA items missing"))?;
    let cell = items
        .iter()
        .find(|item| item["id"] == "canvas:60:cellA")
        .ok_or_else(|| anyhow::anyhow!("UIA A1 missing"))?;
    anyhow::ensure!(
        cell["name"] == "甲一"
            && cell["focus"] == true
            && (cell["w"].as_f64().unwrap() - 105. * f64::from(scale)).abs() <= 1.
            && (cell["h"].as_f64().unwrap() - 37.5 * f64::from(scale)).abs() <= 1.,
        "UIA must expose bilingual name, real focus and once-projected clipped bounds: {cell}"
    );
    let inputs = events
        .iter()
        .filter_map(|event| match &event.action {
            Action::Canvas(CanvasEvent::Input { input }) => Some(input),
            _ => None,
        })
        .collect::<Vec<_>>();
    for action in [
        CanvasSemanticAction::SetValue,
        CanvasSemanticAction::Activate,
    ] {
        let (target, semantic_id) = if action == CanvasSemanticAction::SetValue {
            ("b", "cellB")
        } else {
            ("a", "cellA")
        };
        anyhow::ensure!(inputs.iter().any(|input|input.identity==60&&input.target==target&&input.epoch==1&&matches!(&input.kind,
            CanvasInputKind::A11y{semantic,action:observed,value} if semantic==semantic_id&&*observed==action
                &&(action!=CanvasSemanticAction::SetValue||value.as_deref()==Some("中文 value")))),"missing real UIA action {action:?}: {inputs:?}");
    }
    Ok(())
}

/// This separate process remains an ordinary OS client, with no host state or GPUI object access.
#[test]
#[ignore = "subprocess-only Windows UI Automation client; parent supplies and verifies its own HWND"]
fn native_canvas_semantics_os_client() -> anyhow::Result<()> {
    use windows::{
        Win32::{
            Foundation::HWND,
            System::Com::{
                CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
                CoUninitialize,
            },
            UI::Accessibility::*,
        },
        core::BSTR,
    };
    let handle = std::env::var("NANOBUG_CANVAS_UIA_HWND")?.parse::<usize>()?;
    let owner = std::env::var("NANOBUG_CANVAS_UIA_OWNER")?.parse::<i32>()?;
    // SAFETY: this isolated MTA owns every COM interface and drops it before CoUninitialize.
    let result = unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        (|| -> anyhow::Result<serde_json::Value> {
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?;
            let root = automation.ElementFromHandle(HWND(handle as *mut std::ffi::c_void))?;
            anyhow::ensure!(
                root.CurrentProcessId()? == owner,
                "UIA native HWND owner changed"
            );
            let condition = automation.CreateTrueCondition()?;
            let mut items = BTreeMap::new();
            for _ in 0..30 {
                items.clear();
                let found = root.FindAll(TreeScope_Descendants, &condition)?;
                for index in 0..found.Length()? {
                    let node = found.GetElement(index)?;
                    let id = node.CurrentAutomationId()?.to_string();
                    if id.starts_with("canvas:60:") {
                        items.insert(id, node);
                    }
                }
                if items.contains_key("canvas:60:cellA") {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            anyhow::ensure!(
                items.len() == 4 && !items.contains_key("canvas:60:hidden"),
                "OS UIA visible forest: {:?}",
                items.keys()
            );
            let grid = items
                .get("canvas:60:grid")
                .ok_or_else(|| anyhow::anyhow!("UIA grid missing"))?;
            let description =
                BSTR::try_from(&grid.GetCurrentPropertyValue(UIA_FullDescriptionPropertyId)?)?
                    .to_string();
            anyhow::ensure!(
                description.contains("100000") && description.contains('4'),
                "UIA grid totals: {description}"
            );
            let cell = &items["canvas:60:cellA"];
            let aria =
                BSTR::try_from(&cell.GetCurrentPropertyValue(UIA_AriaPropertiesPropertyId)?)?
                    .to_string();
            anyhow::ensure!(
                aria.contains("rowindextext=1") && aria.contains("colindextext=1"),
                "UIA row/column properties: {aria}"
            );
            cell.SetFocus()?;
            std::thread::sleep(Duration::from_millis(150));
            anyhow::ensure!(
                cell.CurrentHasKeyboardFocus()?.as_bool(),
                "UIA focus did not reach native target"
            );
            let value: IUIAutomationValuePattern = cell.GetCurrentPatternAs(UIA_ValuePatternId)?;
            anyhow::ensure!(
                value.CurrentValue()?.to_string() == "Alpha",
                "UIA value missing"
            );
            anyhow::ensure!(
                value.CurrentIsReadOnly()?.as_bool(),
                "UIA readonly grid value must remain readonly"
            );
            let editor: IUIAutomationValuePattern =
                items["canvas:60:cellB"].GetCurrentPatternAs(UIA_ValuePatternId)?;
            anyhow::ensure!(
                !editor.CurrentIsReadOnly()?.as_bool()
                    && editor.CurrentValue()?.to_string() == "Beta",
                "UIA editable value missing"
            );
            editor.SetValue(&BSTR::from("中文 value"))?;
            let invoke: IUIAutomationInvokePattern =
                cell.GetCurrentPatternAs(UIA_InvokePatternId)?;
            invoke.Invoke()?;
            std::thread::sleep(Duration::from_millis(150));
            let mut output = Vec::new();
            for (id, item) in items {
                let rect = item.CurrentBoundingRectangle()?;
                output.push(serde_json::json!({"id":id,"name":item.CurrentName()?.to_string(),"focus":item.CurrentHasKeyboardFocus()?.as_bool(),
                    "x":rect.left,"y":rect.top,"w":rect.right-rect.left,"h":rect.bottom-rect.top}));
            }
            Ok(serde_json::json!({"items":output,"grid_description":description,"cell_aria":aria}))
        })()
    };
    // SAFETY: all interfaces in the closure have now been released on this initialized MTA.
    unsafe {
        CoUninitialize();
    }
    println!("NANOBUG_UIA_RESULT={}", result?);
    Ok(())
}
