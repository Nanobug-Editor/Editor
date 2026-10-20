//! Actual Vue candidates must fail before replacing a working historical backend combination.
use super::*;
use std::collections::BTreeMap;
pub(super) mod provider;

/// Malformed executable preparation and a valid executable with an incompatible required contract
/// exercise separate rejection gates. Neither failure may replace any currently usable backend.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let original = Package::read(&input("NANOBUG_VUE_PACKAGE"))?;
    for index in 0..h.paths.len() {
        let observed = h.analyze(index, cx).await?;
        if index == 2 {
            observed.assert_semantics("toFixed", "toUpperCase", 2339)?;
        } else {
            observed.assert_semantics("toUpperCase", "toFixed", 2551)?;
        }
    }
    let plans = (0..h.paths.len())
        .map(|index| h.plan(index))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let servers = (0..h.paths.len())
        .map(|index| {
            h.main
                .update(cx, |_, _, cx| {
                    h.app
                        .read(cx)
                        .language_server_for_path(&h.paths[index])
                        .cloned()
                })?
                .context("working candidate has no native source binding")
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let profile = PathBuf::from(std::env::var_os(ROOT_ENV).context("parent-owned profile root")?)
        .join("profile");
    let preferences = preference_records(&profile)?;
    let epochs = {
        let state = h.actor.state.lock().unwrap();
        (
            state.instance_epochs.get(&original.manifest.id).copied(),
            state.command_epochs.get(&original.manifest.id).copied(),
        )
    };
    eprintln!(
        "native Vue candidate checks: {} real preference records (nonempty choice coverage is separate)",
        preferences.len()
    );
    for kind in [CandidateFailure::Preparation, CandidateFailure::Contract] {
        let candidate = candidate(&original, kind)?;
        let log_watermark = h
            .actor
            .state
            .lock()
            .unwrap()
            .logs
            .records(&original.manifest.id)
            .last()
            .map_or(0, |record| record.id);
        let started = Instant::now();
        let mut previous_progress = String::new();
        ensure!(
            h.actor.queue_lifecycle(Work::Install(candidate)),
            "candidate lifecycle queue was already occupied"
        );
        // A valid candidate compiles its real component again before cutover. This observer bounds
        // cold package preparation independently from the unchanged production language deadlines.
        h.wait(
            "actual candidate preparation or compatibility decision",
            120,
            |_, _| {
                let state = h.actor.state.lock().unwrap();
                let progress = format!("{:?}; {:?}", state.progress, state.installation);
                if progress != previous_progress {
                    eprintln!(
                        "native Vue candidate {kind:?} after {:?}: {progress}",
                        started.elapsed()
                    );
                    previous_progress = progress;
                }
                state.progress.is_none()
                    && state.install_control.is_none()
                    && state
                        .installation
                        .as_ref()
                        .is_some_and(|report| report.id == original.manifest.id)
            },
            cx,
        )
        .await?;
        {
            let state = h.actor.state.lock().unwrap();
            let report = state
                .installation
                .as_ref()
                .context("candidate has no actual installation receipt")?;
            ensure!(
                !report.installed,
                "{kind:?} candidate replaced a working Vue package"
            );
            // UI polling consumes OperationStatus; the bounded per-owner log retains this exact
            // failed operation after its receipt, independently of notification timing.
            let failure = state
                .logs
                .records(&original.manifest.id)
                .into_iter()
                .rev()
                .find(|record| {
                    record.id > log_watermark
                        && record.level == plugin_runtime::logs::LogLevel::Error
                        && record.source == "host.operation"
                })
                .context("rejected candidate has no attributed failure")?;
            eprintln!("native Vue candidate {kind:?} refusal: {}", failure.message);
            if matches!(kind, CandidateFailure::Preparation) {
                ensure!(
                    failure.message.contains("WebAssembly")
                        || failure.message.to_lowercase().contains("wasm"),
                    "candidate did not reach its intended executable preparation failure: {}",
                    failure.message
                );
            } else {
                ensure!(
                    failure.message.contains("typescript.adapter")
                        && failure.message.contains("compatible"),
                    "candidate did not reach its intended required-service incompatibility: {}",
                    failure.message
                );
            }
            let installed = state
                .entries
                .iter()
                .find(|entry| entry.manifest.id == original.manifest.id)
                .context("failed candidate removed the installed package")?;
            ensure!(
                installed.digest == original.digest
                    && installed.manifest.version == original.manifest.version,
                "failed candidate changed the installed identity"
            );
            let after = (
                state.instance_epochs.get(&original.manifest.id).copied(),
                state.command_epochs.get(&original.manifest.id).copied(),
            );
            ensure!(
                epochs == after,
                "{kind:?} rejected before cutover but retired native publication epochs: {epochs:?} -> {after:?}"
            );
        }
        for index in 0..h.paths.len() {
            let observed = h.analyze(index, cx).await?;
            if index == 2 {
                observed.assert_semantics("toFixed", "toUpperCase", 2339)?;
            } else {
                observed.assert_semantics("toUpperCase", "toFixed", 2551)?;
            }
            ensure!(
                Arc::ptr_eq(&plans[index], &h.plan(index)?)
                    && Arc::ptr_eq(&servers[index], &observed.server)
                    && plans[index].process_count() == 1,
                "{kind:?} failure replaced healthy Vue backend {index}"
            );
        }
        ensure!(
            preference_records(&profile)? == preferences,
            "failed candidate changed private preference records"
        );
        for (path, disk) in h.paths.iter().zip(&h.disks) {
            ensure!(
                fs::read_to_string(path)? == *disk,
                "candidate failure changed a source file"
            );
        }
        eprintln!(
            "native Vue candidate {kind:?}: old package, all three typed backends and private records retained"
        );
    }
    provider::exercise(h, cx).await?;
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum CandidateFailure {
    Preparation,
    Contract,
}

/// Start from the actual archive and change only explicit candidate inputs. The incompatible
/// contract retains the official WASM bytes; the malformed component still passes Package shape
/// admission and must be rejected by the real background compiler before old owners are retired.
fn candidate(original: &Package, kind: CandidateFailure) -> anyhow::Result<Package> {
    let mut files = original.files.clone();
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"])?;
    let mut version = semver::Version::parse(&original.manifest.version)?;
    version.patch = version
        .patch
        .checked_add(match kind {
            CandidateFailure::Preparation => 1,
            CandidateFailure::Contract => 2,
        })
        .context("test candidate version overflow")?;
    manifest["version"] = serde_json::json!(version.to_string());
    match kind {
        CandidateFailure::Preparation => {
            files.insert(
                original
                    .manifest
                    .component
                    .clone()
                    .context("Vue input has no component")?,
                b"\0asm\x0d\0\x01\0\xff".to_vec(),
            );
        }
        CandidateFailure::Contract => {
            let dependency = manifest["plugin_services"]["requires"]
                .get_mut("typescript.adapter")
                .context("Vue input has no required adapter contract")?;
            dependency["version"] = serde_json::json!("^99");
        }
    }
    let contribution = original
        .manifest
        .contributions
        .as_ref()
        .context("Vue input has no contributions")?;
    let mut parsed: toml::Value = toml::from_str(std::str::from_utf8(&files[contribution])?)?;
    parsed["plugin"]["version"] = toml::Value::String(version.to_string());
    files.insert(contribution.clone(), toml::to_string(&parsed)?.into_bytes());
    files.insert("manifest.json".into(), serde_json::to_vec(&manifest)?);
    let candidate = Package::from_files(files)?;
    ensure!(
        candidate.manifest.permissions == original.manifest.permissions,
        "candidate added permission requirements"
    );
    if matches!(kind, CandidateFailure::Contract) {
        ensure!(
            candidate.component() == original.component(),
            "contract rejection must use the original executable"
        );
    }
    Ok(candidate)
}

/// Immutable byte evidence is scoped to this child's own preference records, never another profile.
/// An empty map is reported as such and does not claim that a real user choice has been preserved.
fn preference_records(root: &Path) -> anyhow::Result<BTreeMap<PathBuf, Vec<u8>>> {
    fn visit(
        root: &Path,
        directory: &Path,
        records: &mut BTreeMap<PathBuf, Vec<u8>>,
    ) -> anyhow::Result<()> {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            ensure!(
                !kind.is_symlink(),
                "private preference scan encountered an unexpected link"
            );
            if kind.is_dir() {
                visit(root, &entry.path(), records)?;
            } else if entry
                .file_name()
                .to_string_lossy()
                .starts_with("preference-")
                && entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "json")
            {
                records.insert(
                    entry.path().strip_prefix(root)?.to_owned(),
                    fs::read(entry.path())?,
                );
            }
        }
        Ok(())
    }
    let mut records = BTreeMap::new();
    visit(root, root, &mut records)?;
    Ok(records)
}
