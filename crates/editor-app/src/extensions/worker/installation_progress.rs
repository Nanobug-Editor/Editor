//! Local and reviewed-market installations publish the same translated dependency stages.
//! Progress never declares language readiness and never changes an installation's authority.
use super::InstallationProgress;
use plugin_runtime::InstallStage;
use rust_i18n::t;

impl InstallationProgress {
    /// Render an observed manager stage in the current UI language. Only the existing transaction
    /// boundary controls cancellation; this adapter neither advances nor repeats any native work.
    pub(super) fn advance(&mut self, stage: InstallStage) {
        use InstallStage::*;
        self.cancellable = !matches!(stage, Prepared | Migrating | Committing | Committed);
        self.message = match stage {
            Migrating => t!("plugins.install_progress.migrating").to_string(),
            Committing => t!("plugins.install_progress.committing").to_string(),
            Committed => t!("plugins.install_progress.committed").to_string(),
            Preparing => t!("plugins.install_progress.preparing").to_string(),
            Downloading(id) => t!("plugins.install_progress.downloading", id = id).to_string(),
            Verifying(id) => t!("plugins.install_progress.verifying", id = id).to_string(),
            Extracting(id) => t!("plugins.install_progress.extracting", id = id).to_string(),
            AwaitingAuthorization(id) => {
                t!("plugins.install_progress.authorization", id = id).to_string()
            }
            Installing(purpose) => {
                t!("plugins.install_progress.installing", purpose = purpose).to_string()
            }
            Prepared => t!("plugins.install_progress.prepared").to_string(),
        };
    }
}
