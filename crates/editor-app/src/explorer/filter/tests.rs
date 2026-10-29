//! Name matching and expansion isolation for the Explorer's transient projection.

use super::*;

/// Mixed-case files remain discoverable below collapsed ancestors, including non-ASCII names.
#[test]
fn explorer_filter_matches_names_without_case_and_preserves_ancestors() {
    let roots = vec![TreeItem::new("root", "Workspace").children([
        TreeItem::new("root/src", "src").children([
            TreeItem::new("root/src/Main.RS", "Main.RS"),
            TreeItem::new("root/src/other.txt", "other.txt"),
            TreeItem::new("root/src/文件É.txt", "文件É.txt"),
        ]),
        TreeItem::new("root/empty", "Empty"),
    ])];
    let filtered = filter_items(&roots, "main.rs");
    assert_eq!(filtered[0].children.len(), 1);
    assert_eq!(filtered[0].children[0].children.len(), 1);
    assert_eq!(
        filtered[0].children[0].children[0].label.as_str(),
        "Main.RS"
    );
    assert!(filtered[0].is_expanded() && filtered[0].children[0].is_expanded());
    assert!(!roots[0].is_expanded() && !roots[0].children[0].is_expanded());
    assert_eq!(
        filter_items(&roots, "文件é")[0].children[0].children.len(),
        1
    );
    assert_eq!(
        filter_items(&roots, "empty")[0].children[0].label.as_str(),
        "Empty"
    );
    assert!(filter_items(&roots, "workspace")[0].children.is_empty());
}

/// A matched folder retains its contents; absent matches leave only the workspace heading.
#[test]
fn explorer_filter_folder_match_keeps_contents_and_no_match_drops_unrelated_names() {
    let roots = vec![
        TreeItem::new("root", "root").children([
            TreeItem::new("root/Sources", "Sources")
                .children([TreeItem::new("root/Sources/main.rs", "main.rs")]),
            TreeItem::new("root/README", "README"),
        ]),
    ];
    let filtered = filter_items(&roots, "sources");
    assert_eq!(filtered[0].children.len(), 1);
    assert_eq!(
        filtered[0].children[0].children[0].label.as_str(),
        "main.rs"
    );
    assert!(filter_items(&roots, "absent")[0].children.is_empty());
}
