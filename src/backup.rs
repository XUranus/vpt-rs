use crate::error::Result;
use crate::types::{BackupPlan, Capability};

pub trait BlockDeviceCopier: Send + Sync {
    fn backend_name(&self) -> &'static str;
    fn capabilities(&self) -> &'static [Capability];
    fn backup_volume(&self, plan: &BackupPlan) -> Result<()>;
}
