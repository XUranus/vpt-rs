use crate::backend::Backend;
use crate::error::Result;
use crate::types::RestorePlan;

pub trait RestorePlanner: Backend {
    fn restore_volume(&self, plan: &RestorePlan) -> Result<()>;
}
