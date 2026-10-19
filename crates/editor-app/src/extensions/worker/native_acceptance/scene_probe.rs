//! Read-only actor boundaries explain a replaced native scene without changing its admission or order.
use super::super::Work;
use super::*;
use plugin_runtime::plugin_protocol::{
    api,
    ui::{self, CanvasEvent},
};

/// Retain the complete immutable public document and public ownership metadata at one actor boundary.
/// Tree snapshots are deliberately absent: reading them may expire pending requests.
pub(in crate::extensions::worker) struct Sample {
    document: Option<Arc<ui::Document>>,
    ownership: serde_json::Value,
    processes: Option<usize>,
    resources: Option<usize>,
    at_millis: u128,
}

/// Sample only public immutable Manager observations; no guest, request, timer or expiry is polled.
pub(in crate::extensions::worker) fn sample(manager: &Manager) -> Option<Sample> {
    if std::env::var_os("NANOBUG_GIT_NATIVE_SCENE_PROBE").is_none() {
        return None;
    }
    let instance = manager.live.get("git");
    let installed = manager.installed.get("git");
    Some(Sample {
        document: instance.and_then(|instance| instance.views.get("history").cloned()),
        ownership: serde_json::json!({
            "instance": manager.instance_id("git"),
            "enabled": installed.map(|entry| entry.enabled),
            "grants": installed.map(|entry| &entry.grants),
            "live_trees": instance.map(|instance| instance.tree_resources().iter()
                .map(|resource| serde_json::json!({"handle": resource.handle(), "live": resource.is_live()}))
                .collect::<Vec<_>>()),
        }),
        processes: instance.map(|instance| instance.process_count()),
        resources: instance.map(|instance| instance.resource_count()),
        at_millis: millis(),
    })
}

/// Describe original callbacks without serializing text, command arguments or process output.
pub(in crate::extensions::worker) fn describe_work(work: Option<&Work>) -> serde_json::Value {
    if std::env::var_os("NANOBUG_GIT_NATIVE_SCENE_PROBE").is_none() {
        return serde_json::Value::Null;
    }
    match work {
        None => serde_json::json!({"kind": "idle"}),
        Some(Work::Event(plugin, epoch, panel, event)) => serde_json::json!({
            "kind": "event", "plugin": plugin, "epoch": epoch, "panel": panel,
            "notification": notification(event),
        }),
        Some(Work::FormChange {
            plugin,
            epoch,
            panel,
            event,
            ..
        }) => serde_json::json!({
            "kind": "form_change", "plugin": plugin, "epoch": epoch, "panel": panel,
            "revision": event.revision, "node": event.node,
        }),
        Some(work) => {
            serde_json::json!({"kind": "other_work", "variant": format!("{:?}", std::mem::discriminant(work))})
        }
    }
}

/// Record changed actor boundaries and original event calls, including complete document equality.
/// Fingerprints exclude only the outer UI revision, preserving every public source/resource authority field.
pub(in crate::extensions::worker) fn transition(
    stage: &str,
    cause: &serde_json::Value,
    before: Option<Sample>,
    after: Option<Sample>,
) {
    let (Some(before), Some(after)) = (before, after) else {
        return;
    };
    let revisions = (
        before.document.as_ref().map(|scene| scene.revision),
        after.document.as_ref().map(|scene| scene.revision),
    );
    let ownership_same = before.ownership == after.ownership;
    if revisions.0 == revisions.1
        && ownership_same
        && before.processes == after.processes
        && before.resources == after.resources
        && !(stage == "work" && cause["kind"] == "event")
    {
        return;
    }
    let document_before = normalized(&before.document);
    let document_after = normalized(&after.document);
    let mut changed_fields = Vec::new();
    differences("", &document_before, &document_after, &mut changed_fields);
    let facts = serde_json::json!({
        "stage": stage, "cause": cause, "at_millis": after.at_millis,
        "before_at_millis": before.at_millis, "revision_before": revisions.0, "revision_after": revisions.1,
        "processes_before": before.processes, "processes_after": after.processes,
        "resources_before": before.resources, "resources_after": after.resources,
        "document_equal_except_revision": document_before == document_after,
        "document_sha256_before": fingerprint(&document_before),
        "document_sha256_after": fingerprint(&document_after),
        "changed_field_paths_first_32": changed_fields,
        "public_ownership_equal": ownership_same,
        "public_ownership_before": before.ownership, "public_ownership_after": after.ownership,
    });
    eprintln!("[git120-scene-probe] {facts}");
}

/// Serialize the complete document; only its top-level dispatch revision is omitted for comparison.
fn normalized(document: &Option<Arc<ui::Document>>) -> serde_json::Value {
    let mut value = serde_json::to_value(document).expect("public UI document serializes");
    if let Some(object) = value.as_object_mut() {
        object.remove("revision");
    }
    value
}

/// Content appears only as a digest; diagnostics never print changed text or document bytes.
fn fingerprint(value: &serde_json::Value) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("UI observation serializes"))
    )
}

/// Report bounded JSON field paths instead of values, including resource and source fields outside root.
fn differences(
    path: &str,
    before: &serde_json::Value,
    after: &serde_json::Value,
    out: &mut Vec<String>,
) {
    if before == after || out.len() >= 32 {
        return;
    }
    match (before, after) {
        (serde_json::Value::Object(left), serde_json::Value::Object(right)) => {
            let keys: std::collections::BTreeSet<_> = left.keys().chain(right.keys()).collect();
            for key in keys {
                let next = format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"));
                differences(
                    &next,
                    left.get(key).unwrap_or(&serde_json::Value::Null),
                    right.get(key).unwrap_or(&serde_json::Value::Null),
                    out,
                );
            }
        }
        (serde_json::Value::Array(left), serde_json::Value::Array(right))
            if left.len() == right.len() =>
        {
            for (index, (left, right)) in left.iter().zip(right).enumerate() {
                differences(&format!("{path}/{index}"), left, right, out);
            }
        }
        _ => out.push(path.to_owned()),
    }
}

/// Notification names and safe geometry distinguish delayed request receipts from ordinary input.
fn notification(event: &api::Notification) -> serde_json::Value {
    match event {
        api::Notification::Ui(event) => serde_json::json!({
            "kind": "ui", "revision": event.revision, "node": event.node,
            "action": match &event.action {
                ui::Action::Canvas(event) => canvas(event),
                ui::Action::ViewportInput(input) => canvas(&input.event),
                action => serde_json::json!({"kind": "non_canvas", "variant": format!("{:?}", std::mem::discriminant(action))}),
            },
        }),
        api::Notification::Request { handle, update } => serde_json::json!({
            "kind": "request", "handle": handle,
            "update": match update {
                api::RequestUpdate::Accepted => serde_json::json!({"kind":"accepted"}),
                api::RequestUpdate::Progress { .. } => serde_json::json!({"kind":"progress"}),
                api::RequestUpdate::Cancelled { reason, .. } => serde_json::json!({"kind":"cancelled","reason":reason}),
                api::RequestUpdate::Completed { result: Err(error) } => serde_json::json!({"kind":"completed_error","code":error.code}),
                api::RequestUpdate::Completed { result: Ok(api::EditorValue::PanelVisibility { panel, visible }) } => serde_json::json!({"kind":"completed","editor_value":"panel_visibility","panel":panel,"visible":visible}),
                api::RequestUpdate::Completed { result: Ok(value) } => serde_json::json!({"kind":"completed","editor_value":editor_value_kind(value)}),
            },
        }),
        api::Notification::PanelVisibility { panel, visible } => {
            serde_json::json!({"kind":"panel_visibility","panel":panel,"visible":visible})
        }
        api::Notification::Timer { handle, sequence } => {
            serde_json::json!({"kind":"timer","handle":handle,"sequence":sequence})
        }
        api::Notification::Focus(focused) => serde_json::json!({"kind":"focus","focused":focused}),
        api::Notification::Resize { width, height } => {
            serde_json::json!({"kind":"resize","width":width,"height":height})
        }
        event => serde_json::json!({"kind": match event {
            api::Notification::Preview { .. } => "preview",
            api::Notification::FilePreview { .. } => "file_preview",
            api::Notification::Process { .. } => "process",
            api::Notification::Document { .. } => "document",
            api::Notification::DocumentEvent { .. } => "document_event",
            api::Notification::WorkspaceFile { .. } => "workspace_file",
            api::Notification::Theme(_) => "theme",
            api::Notification::Command { .. } => "command",
            api::Notification::CommandInvocation(_) => "command_invocation",
            api::Notification::CommandRequest { .. } => "command_request",
            api::Notification::PreferenceChanged { .. } => "preference_changed",
            _ => "other_notification",
        }, "variant": format!("{:?}", std::mem::discriminant(event))}),
    }
}

/// Canvas metadata is safe to inspect; printable input is never included in this actor trace.
fn canvas(event: &CanvasEvent) -> serde_json::Value {
    match event {
        CanvasEvent::Text { .. } => serde_json::json!({"kind":"text_redacted"}),
        CanvasEvent::Key {
            key,
            ctrl,
            alt,
            shift,
        } => serde_json::json!({
            "kind":"key", "key": if matches!(key.as_str(),"home"|"end"|"enter"|"space") { key.as_str() } else { "other_key" },
            "ctrl":ctrl,"alt":alt,"shift":shift,
        }),
        event => serde_json::to_value(event).expect("canvas geometry serializes"),
    }
}

/// Expose only the public result variant, never text, paths or clipboard contents.
fn editor_value_kind(value: &api::EditorValue) -> &'static str {
    match value {
        api::EditorValue::LanguageBinding { .. } => "language_binding",
        api::EditorValue::Language(_) => "language",
        api::EditorValue::Workspace(_) => "workspace",
        api::EditorValue::Decorations { .. } => "decorations",
        api::EditorValue::DocumentOpened(_) => "document_opened",
        api::EditorValue::DocumentLocated { .. } => "document_located",
        api::EditorValue::DocumentsCompared { .. } => "documents_compared",
        api::EditorValue::Documents { .. } => "documents",
        api::EditorValue::DocumentSnapshot(_) => "document_snapshot",
        api::EditorValue::Interaction(_) => "interaction",
        api::EditorValue::Opened { .. } => "opened",
        api::EditorValue::ImageSaved { .. } => "image_saved",
        api::EditorValue::Clipboard { .. } => "clipboard",
        api::EditorValue::Unit => "unit",
        api::EditorValue::Selection { .. } => "selection",
        api::EditorValue::DocumentSelection { .. } => "document_selection",
        api::EditorValue::Edited { .. } => "edited",
        api::EditorValue::Directory { .. } => "directory",
        api::EditorValue::Saved { .. } => "saved",
        api::EditorValue::PanelVisibility { .. } => "panel_visibility",
    }
}

/// The same UTC millisecond scale correlates actor boundaries with the native callback trace.
fn millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
}
