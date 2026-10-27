//! An independent COM client reads and edits the actual guest's native semantic tree.
use super::*;

/// Probe only the supplied live owned HWND; no host state or business API enters the client.
pub(in crate::ui::plugin::canvas::scene_native_tests) fn probe(
    handle: usize,
    target: &str,
    write: bool,
) -> anyhow::Result<serde_json::Value> {
    let output = std::process::Command::new(std::env::current_exe()?)
        .args([
            "native_canvas_editable_os_client",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("NANOBUG_CANVAS_UIA_HWND", handle.to_string())
        .env("NANOBUG_CANVAS_UIA_OWNER", std::process::id().to_string())
        .env("NANOBUG_CANVAS_UIA_TARGET", target)
        .env("NANOBUG_CANVAS_UIA_WRITE", if write { "1" } else { "0" })
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "actual guest OS UIA: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout)?;
    let json = stdout
        .lines()
        .find_map(|line| {
            line.split_once("NANOBUG_EDITABLE_UIA=")
                .map(|(_, json)| json)
        })
        .ok_or_else(|| anyhow::anyhow!("actual guest UIA result missing: {stdout}"))?;
    Ok(serde_json::from_str(json)?)
}
/// The separate OS process verifies native names, values, focus and bounded virtual metadata.
#[test]
#[ignore = "subprocess-only native UIA client; parent supplies its exact owned HWND"]
fn native_canvas_editable_os_client() -> anyhow::Result<()> {
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
    let target = std::env::var("NANOBUG_CANVAS_UIA_TARGET")?;
    let write = std::env::var("NANOBUG_CANVAS_UIA_WRITE")? == "1";
    // SAFETY: this MTA owns and drops every UIA interface before CoUninitialize.
    let result = unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        (|| -> anyhow::Result<serde_json::Value> {
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?;
            let root = automation.ElementFromHandle(HWND(handle as *mut std::ffi::c_void))?;
            anyhow::ensure!(
                root.CurrentProcessId()? == owner,
                "actual guest HWND ownership changed"
            );
            let mut items = BTreeMap::new();
            for _ in 0..30 {
                items.clear();
                let found =
                    root.FindAll(TreeScope_Descendants, &automation.CreateTrueCondition()?)?;
                for index in 0..found.Length()? {
                    let item = found.GetElement(index)?;
                    let id = item.CurrentAutomationId()?.to_string();
                    if id.starts_with("canvas:") {
                        items.insert(id, item);
                    }
                }
                if items.contains_key(&target) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            anyhow::ensure!(
                items.len() <= 8,
                "virtual logical rows expanded in native tree: {}",
                items.len()
            );
            let element = items.get(&target).ok_or_else(|| {
                anyhow::anyhow!(
                    "actual guest native target missing: {target}, {:?}",
                    items.keys()
                )
            })?;
            let name = element.CurrentName()?.to_string();
            anyhow::ensure!(
                !name.is_empty(),
                "actual guest native semantic name missing"
            );
            let value: IUIAutomationValuePattern =
                element.GetCurrentPatternAs(UIA_ValuePatternId)?;
            anyhow::ensure!(
                !value.CurrentIsReadOnly()?.as_bool(),
                "actual guest editable text advertised readonly"
            );
            let text = value.CurrentValue()?.to_string();
            if write {
                element.SetFocus()?;
                std::thread::sleep(Duration::from_millis(150));
            }
            let focused = element.CurrentHasKeyboardFocus()?.as_bool();
            if write {
                value.SetValue(&BSTR::from("辅助 value"))?;
            }
            let mut grid = String::new();
            for (id, item) in &items {
                if id.ends_with(":table") {
                    grid = BSTR::try_from(
                        &item.GetCurrentPropertyValue(UIA_FullDescriptionPropertyId)?,
                    )?
                    .to_string();
                }
            }
            let foreground = windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow()
                == HWND(handle as *mut std::ffi::c_void);
            // A global UIA focus query may race foreground changes; never read another process's text.
            let actual_focus = if foreground {
                automation.GetFocusedElement().ok().filter(|element| element.CurrentProcessId().ok()==Some(owner))
                    .map(|element| serde_json::json!({"id":element.CurrentAutomationId().map(|value|value.to_string()).unwrap_or_default(),"name":element.CurrentName().map(|value|value.to_string()).unwrap_or_default()}))
            } else {
                None
            };
            Ok(
                serde_json::json!({"name":name,"text":text,"focus":focused,"foreground":foreground,"actual_focus":actual_focus,"root_focus":root.CurrentHasKeyboardFocus()?.as_bool(),"items":items.len(),"grid":grid}),
            )
        })()
    };
    // SAFETY: no COM references escape the closure.
    unsafe {
        CoUninitialize();
    }
    println!("NANOBUG_EDITABLE_UIA={}", result?);
    Ok(())
}
