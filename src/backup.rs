use crate::backend::Backend;
use crate::error::Result;
use crate::types::BackupPlan;

pub trait BackupExecutor: Backend {
    fn backup_volume(&self, plan: &BackupPlan) -> Result<()>;
}
