//! A real provider update must preserve the currently usable mandatory consumers before cutover.
use super::*;

/// Change the actual TypeScript archive's offered adapter contract, while retaining its executable
/// and permissions. This direction differs from an incompatible requirement on the Vue candidate.
pub(in crate::extensions::worker::native_vue_recovery_tests::scenario) async fn exercise(
    h: &Harness,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let original = Package::read(&input("NANOBUG_TYPESCRIPT_PACKAGE"))?;
    let vue = Package::read(&input("NANOBUG_VUE_PACKAGE"))?;
    let before = Baseline::capture(h, [&original, &vue], cx).await?;
    let candidate = incompatible_provider(&original)?;
    let log_watermark = h
        .actor
        .state
        .lock()
        .unwrap()
        .logs
        .records(&original.manifest.id)
        .last()
        .map_or(0, |record| record.id);
    ensure!(
        h.actor.queue_lifecycle(Work::Install(candidate, true)),
        "provider candidate lifecycle queue was already occupied"
    );
    wait_for_rejection(h, &original.manifest.id, log_watermark, cx).await?;
    before.assert_retained(h, cx).await?;
    eprintln!(
        "native TypeScript provider candidate: both owners, three typed Vue backends and private records retained"
    );
    Ok(())
}

/// Keep both runtime owners and all existing analysis incarnations observable before reanalysis.
/// Preference bytes are scoped to this child's profile; an empty record set is reported honestly.
struct Baseline {
    identities: BTreeMap<String, (String, String, u64)>,
    plans: Vec<Arc<plugin_runtime::LanguageService>>,
    observations: Vec<Observation>,
    profile: PathBuf,
    preferences: BTreeMap<PathBuf, Vec<u8>>,
}

impl Baseline {
    /// Only ordinary standard requests establish working semantics; package metadata alone does not.
    async fn capture(
        h: &Harness,
        originals: [&Package; 2],
        cx: &mut AsyncApp,
    ) -> anyhow::Result<Self> {
        let mut observations = Vec::new();
        let mut plans = Vec::new();
        for index in 0..h.paths.len() {
            let observed = h.analyze(index, cx).await?;
            assert_semantics(index, &observed)?;
            observations.push(observed);
            plans.push(h.plan(index)?);
        }
        let identities = {
            let state = h.actor.state.lock().unwrap();
            originals
                .into_iter()
                .map(|original| {
                    let id = &original.manifest.id;
                    let installed = state
                        .entries
                        .iter()
                        .find(|entry| &entry.manifest.id == id)
                        .context("provider candidate input is not installed")?;
                    ensure!(
                        installed.digest == original.digest
                            && installed.manifest.version == original.manifest.version,
                        "provider candidate input differs from the installed archive"
                    );
                    let owner = *state
                        .instance_epochs
                        .get(id)
                        .context("working provider or consumer has no published owner epoch")?;
                    Ok((
                        id.clone(),
                        (
                            installed.digest.clone(),
                            installed.manifest.version.clone(),
                            owner,
                        ),
                    ))
                })
                .collect::<anyhow::Result<BTreeMap<_, _>>>()?
        };
        let profile =
            PathBuf::from(std::env::var_os(ROOT_ENV).context("parent-owned profile root")?)
                .join("profile");
        let preferences = preference_records(&profile)?;
        eprintln!(
            "native provider candidate: {} real preference records (nonempty choice coverage is separate)",
            preferences.len()
        );
        Ok(Self {
            identities,
            plans,
            observations,
            profile,
            preferences,
        })
    }

    /// Inspect ownership before issuing any fresh request, so lazy recovery cannot hide a cutover.
    async fn assert_retained(&self, h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
        {
            let state = h.actor.state.lock().unwrap();
            for (id, (digest, version, owner)) in &self.identities {
                let installed = state
                    .entries
                    .iter()
                    .find(|entry| &entry.manifest.id == id)
                    .context("provider rejection removed an installed package")?;
                ensure!(
                    installed.digest == *digest
                        && installed.manifest.version == *version
                        && state.instance_epochs.get(id) == Some(owner),
                    "provider rejection changed the original package or owner of {id}"
                );
            }
        }
        for index in 0..h.paths.len() {
            ensure!(
                Arc::ptr_eq(&self.plans[index], &h.plan(index)?)
                    && self.plans[index].process_count() == 1
                    && self.observations[index].capture.is_live(),
                "provider rejection retired a previously working Vue context {index}"
            );
            let observed = h.analyze(index, cx).await?;
            assert_semantics(index, &observed)?;
            ensure!(
                Arc::ptr_eq(&self.observations[index].server, &observed.server)
                    && Arc::ptr_eq(&self.plans[index], &h.plan(index)?),
                "provider rejection rebuilt Vue backend {index} instead of retaining it"
            );
        }
        ensure!(
            preference_records(&self.profile)? == self.preferences,
            "provider rejection changed private preference records"
        );
        for (path, disk) in h.paths.iter().zip(&h.disks) {
            ensure!(
                fs::read_to_string(path)? == *disk,
                "provider rejection changed a project source file"
            );
        }
        Ok(())
    }
}

/// Observe the actual lifecycle receipt, independently of production LSP request budgets. A refusal
/// must identify the changed domain contract, rather than a damaged executable or host API minimum.
async fn wait_for_rejection(
    h: &Harness,
    id: &str,
    log_watermark: u64,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let started = Instant::now();
    let mut previous_progress = String::new();
    h.wait(
        "actual provider candidate compatibility decision",
        120,
        |_, _| {
            let state = h.actor.state.lock().unwrap();
            let progress = format!("{:?}; {:?}", state.progress, state.installation);
            if progress != previous_progress {
                eprintln!(
                    "native TypeScript provider candidate after {:?}: {progress}",
                    started.elapsed()
                );
                previous_progress = progress;
            }
            state.progress.is_none()
                && state.install_control.is_none()
                && state
                    .installation
                    .as_ref()
                    .is_some_and(|report| report.id == id)
        },
        cx,
    )
    .await?;
    let state = h.actor.state.lock().unwrap();
    let report = state
        .installation
        .as_ref()
        .context("provider candidate has no actual installation receipt")?;
    ensure!(
        !report.installed,
        "incompatible TypeScript provider candidate replaced the usable historical combination"
    );
    // OperationStatus is a one-shot UI delivery. Attribute the durable failure by its original
    // owner's log and this candidate's watermark, never by whichever notice the UI consumed last.
    let failure = state
        .logs
        .records(id)
        .into_iter()
        .rev()
        .find(|record| {
            record.id > log_watermark
                && record.level == plugin_runtime::logs::LogLevel::Error
                && record.source == "host.operation"
        })
        .context("provider refusal has no attributed failure")?;
    eprintln!(
        "native TypeScript provider candidate refusal: {}",
        failure.message
    );
    ensure!(
        failure.message.contains("typescript.adapter")
            && (failure.message.contains("compatible") || failure.message.contains("required")),
        "candidate did not reach mandatory consumer compatibility validation: {}",
        failure.message
    );
    Ok(())
}

/// Retain every executable/resource byte and only vary the offered contract plus test package
/// versions. The production plugin repository and the original input archive remain untouched.
fn incompatible_provider(original: &Package) -> anyhow::Result<Package> {
    let mut files = original.files.clone();
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"])?;
    let mut version = semver::Version::parse(&original.manifest.version)?;
    version.patch = version
        .patch
        .checked_add(1)
        .context("test candidate version overflow")?;
    manifest["version"] = serde_json::json!(version.to_string());
    let contract = manifest["plugin_services"]["provides"]
        .get_mut("typescript.adapter")
        .context("TypeScript input has no offered adapter contract")?;
    contract["version"] = serde_json::json!("2.0.0");
    let contribution = original
        .manifest
        .contributions
        .as_ref()
        .context("TypeScript input has no contributions")?;
    let mut parsed: toml::Value = toml::from_str(std::str::from_utf8(&files[contribution])?)?;
    parsed["plugin"]["version"] = toml::Value::String(version.to_string());
    files.insert(contribution.clone(), toml::to_string(&parsed)?.into_bytes());
    files.insert("manifest.json".into(), serde_json::to_vec(&manifest)?);
    let candidate = Package::from_files(files)?;
    ensure!(
        candidate.component() == original.component()
            && candidate.manifest.permissions == original.manifest.permissions,
        "provider compatibility candidate changed executable bytes or permission requirements"
    );
    Ok(candidate)
}

/// Opposite valid methods distinguish retained historic string typing from the modern numeric one.
fn assert_semantics(index: usize, observed: &Observation) -> anyhow::Result<()> {
    if index == 2 {
        observed.assert_semantics("toFixed", "toUpperCase", 2339)
    } else {
        observed.assert_semantics("toUpperCase", "toFixed", 2551)
    }
}
