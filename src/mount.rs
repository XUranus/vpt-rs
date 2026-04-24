use crate::error::Result;
use crate::types::{Capability, MountHandle, MountRequest};

pub trait MountManager: Send + Sync {
    fn backend_name(&self) -> &'static str;
    fn capabilities(&self) -> &'static [Capability];
    fn mount_snapshot(&self, request: &MountRequest) -> Result<MountHandle>;
    fn unmount(&self, handle: &MountHandle) -> Result<()>;
}
