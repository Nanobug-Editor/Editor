//! Create an isolated, pre-authorized test installation under target/, never in user settings.
use plugin_runtime::{Manager, Package, plugin_protocol::Environment};
use std::path::PathBuf;

/// Explicit ZIP arguments prepare only the requested test packages; an empty list models a fresh source-free install.
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let root = PathBuf::from(args.get(1).expect("fixture root"));
    let workspace = PathBuf::from(args.get(2).expect("workspace"));
    std::fs::create_dir_all(&root)?;
    std::fs::create_dir_all(&workspace)?;
    anyhow::ensure!(
        root.canonicalize()?
            .starts_with(std::env::current_dir()?.join("target").canonicalize()?),
        "Fixtures must stay under target/"
    );
    let mut manager = Manager::open(
        root,
        Environment {
            workspace: workspace.display().to_string(),
            os: std::env::consts::OS.into(),
            ..Environment::default()
        },
    )?;
    let packages = args.iter().skip(3).map(PathBuf::from).collect::<Vec<_>>();
    for path in packages {
        let package = Package::read(&path)?;
        manager.install(&package, package.manifest.permissions.clone())?;
    }
    Ok(())
}
