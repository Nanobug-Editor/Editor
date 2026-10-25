//! Disposable divergent history supplies real parent OIDs for colored native graph acceptance.
use super::*;

/// Add a two-parent merge with unchanged source trees, preserving the fixture's dirty-file setup.
/// All refs and objects belong to `root`, the parent-owned temporary repository.
pub(super) fn prepare(root: &Path) -> anyhow::Result<()> {
    let tree = git_text(root, &["rev-parse", "HEAD^{tree}"])?;
    let base = git_text(root, &["rev-parse", "HEAD"])?;
    let mut tips = Vec::new();
    for branch in ["primary", "feature"] {
        let mut parent = base.trim().to_owned();
        for index in 0..2 {
            parent = git_text(
                root,
                &[
                    "commit-tree",
                    tree.trim(),
                    "-p",
                    &parent,
                    "-m",
                    &format!("Topology {branch} {index}"),
                ],
            )?
            .trim()
            .to_owned();
        }
        tips.push(parent);
    }
    let merge = git_text(
        root,
        &[
            "commit-tree",
            tree.trim(),
            "-p",
            &tips[0],
            "-p",
            &tips[1],
            "-m",
            "Topology color merge",
        ],
    )?;
    git(root, &["update-ref", "refs/heads/main", merge.trim()])?;
    git(root, &["update-ref", "refs/heads/color-feature", &tips[1]])?;
    Ok(())
}
