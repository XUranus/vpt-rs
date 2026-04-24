use crate::error::Result;
use crate::types::{Capability, RestorePlan};

pub trait RestorePlanner: Send + Sync {
    fn backend_name(&self) -> &'static str;
    fn capabilities(&self) -> &'static [Capability];
    fn restore_volume(&self, plan: &RestorePlan) -> Result<()>;
}
