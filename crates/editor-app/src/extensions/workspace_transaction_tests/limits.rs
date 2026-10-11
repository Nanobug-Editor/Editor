//! Planned disk growth must fit the same bounded recovery capture before presenting destructive consent.
use super::*;

/// Two copies of a 16 MiB source would exceed the 32 MiB observable post-state budget.
#[gpui::test]
#[ignore = "build workspace-scaffold using the current host --plugin-package first"]
fn workspace_transaction_copy_growth_is_rejected_before_preview_or_disk_effects(
    cx: &mut TestAppContext,
) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let directory = path.parent().unwrap();
        std::fs::write(directory.join("large.bin"), vec![b'x'; 16 * 1024 * 1024]).unwrap();
        let (_profile, mut manager) = manager(directory, "workspace-scaffold");
        let root = workspace_root(&mut manager, "workspace-scaffold");
        let source = target(&root, "large.bin");
        let version = stamp(
            visual,
            &app,
            &mut manager,
            "workspace-scaffold",
            source.clone(),
        );
        let value = begin(
            &mut manager,
            "workspace-scaffold",
            api::Operation::Editor {
                operation: api::EditorOperation::Workspace {
                    operation: workspace::Operation::ApplyPlan {
                        plan: workspace::Plan {
                            origin: None,
                            title: "Bounded copy proposal / 有限复制计划".into(),
                            changes: ["first.bin", "second.bin"]
                                .into_iter()
                                .map(|name| workspace::Change::Copy {
                                    source: source.clone(),
                                    target: target(&root, name),
                                    expected_source: version.clone(),
                                })
                                .collect(),
                        },
                    },
                },
                timeout_ms: 120_000,
            },
        )
        .unwrap();
        assert!(matches!(value, api::Value::BatchAccepted { .. }));
        let request = publish(visual, &app, &mut manager, "workspace-scaffold")
            .pop()
            .unwrap();
        let receipt = batch(&request);
        assert_eq!(receipt.failed.unwrap().code, api::ErrorCode::LimitExceeded);
        assert!(receipt.completed.is_empty());
        assert!(receipt.affected.is_empty());
        assert!(!receipt.recoverable);
        visual.update(|window, cx| window.draw(cx).clear(cx));
        assert!(visual.debug_bounds("workspace-plan-confirm").is_none());
        assert!(!directory.join("first.bin").exists());
        assert!(!directory.join("second.bin").exists());
        assert_eq!(
            std::fs::metadata(directory.join("large.bin"))
                .unwrap()
                .len(),
            16 * 1024 * 1024
        );
    });
}
