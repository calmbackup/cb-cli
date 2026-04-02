use crate::core::api::ApiClient;
use crate::core::config::Config;
use crate::core::dumper::DatabaseDumper;
use crate::core::types::{ProgressFn, RestoreResult, Result};

/// Orchestrates the full restore pipeline.
pub struct RestoreService {
    pub config: Config,
    pub dumper: Box<dyn DatabaseDumper>,
    pub key: [u8; 32],
    pub api: ApiClient,
}

impl RestoreService {
    /// Execute the 7-step restore pipeline.
    ///
    /// 1. Fetch backup details (checksum, download URL)
    /// 2. Check local cache (verify checksum, skip download if valid)
    /// 3. Download from cloud if needed
    /// 4. Decrypt backup
    /// 5. Extract tar.gz archive
    /// 6. Restore database
    /// 7. Restore directories (walk extracted dirs back to original paths)
    pub async fn restore(
        &self,
        backup_id: &str,
        prune_local: bool,
        on_progress: ProgressFn,
    ) -> Result<RestoreResult> {
        todo!("Implement 7-step restore pipeline")
    }
}
