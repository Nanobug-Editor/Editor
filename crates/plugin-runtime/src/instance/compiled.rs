//! Live instances retain immutable engine-owned code, while every role receives fresh Store state.
use super::*;
use sha2::{Digest, Sha256};

/// One code reference per instance; clones contain no Store, permissions, WASI state or resource handles.
/// The owning instance and bounded preparation pool delimit retention, with no global/disk cache.
#[derive(Clone)]
pub(crate) struct CompiledCode {
    component: Component,
    digest: [u8; 32],
}

impl CompiledCode {
    /// Code can only be instantiated with the exact engine that compiled it. Its watchdog advances
    /// time globally, while fuel, memory limits and epoch deadlines remain per newly created Store.
    pub(crate) fn engine(&self) -> &Engine {
        self.component.engine()
    }

    pub(super) fn component(&self) -> &Component {
        &self.component
    }

    /// Admit a live owner's immutable code only after checking both byte identity and engine identity.
    /// A stale/mismatched snapshot fails closed; cold compilation records timing without module data.
    pub(super) fn prepare(
        engine: &Engine,
        bytes: &[u8],
        reused: Option<&Self>,
        logs: &crate::RuntimeLogs,
        plugin: &str,
        pure: bool,
    ) -> anyhow::Result<Self> {
        let mode = if pure { "pure-language" } else { "main" };
        let digest = Sha256::digest(bytes).into();
        if let Some(code) = reused {
            anyhow::ensure!(code.digest == digest, "Prepared component bytes changed");
            anyhow::ensure!(
                Engine::same(code.engine(), engine),
                "Prepared component engine changed"
            );
            logs.append(
                plugin,
                crate::LogLevel::Info,
                "wasm/compile",
                format!("Reusing {mode} compiled component for an isolated Store"),
            );
            return Ok(code.clone());
        }
        let started = std::time::Instant::now();
        logs.append(
            plugin,
            crate::LogLevel::Info,
            "wasm/compile",
            format!("Begin {mode} component compilation ({} bytes)", bytes.len()),
        );
        let component = Component::new(engine, bytes).inspect_err(|error| {
            logs.append(
                plugin,
                crate::LogLevel::Error,
                "wasm/compile",
                format!(
                    "{mode} component compilation failed after {} ms: {error:#}",
                    started.elapsed().as_millis()
                ),
            );
        })?;
        logs.append(
            plugin,
            crate::LogLevel::Info,
            "wasm/compile",
            format!(
                "Completed {mode} component compilation in {} ms",
                started.elapsed().as_millis()
            ),
        );
        Ok(Self { component, digest })
    }
}

impl Instance {
    /// Capture only immutable code from this exact live owner before detaching role preparation.
    pub(crate) fn compiled_code(&self) -> CompiledCode {
        self.compiled.clone()
    }

    /// Initialize a readonly-discovery candidate with fresh state and the live owner's compiled code.
    /// Byte checks precede reuse; all normal negotiation, grants, staging and lifecycle checks still run.
    pub(crate) fn prepare_role_worker(
        code: &CompiledCode,
        bytes: &[u8],
        manifest: &Manifest,
        grants: &BTreeSet<String>,
        environment: Environment,
        data: PathBuf,
        assets: PathBuf,
        snapshot: Option<Snapshot>,
        host_resources: crate::HostResources,
    ) -> anyhow::Result<Self> {
        Self::prepare_with_mode(
            code.engine(),
            bytes,
            manifest,
            grants,
            environment,
            data,
            assets,
            snapshot,
            host_resources,
            Some(code),
            false,
        )
    }
}
