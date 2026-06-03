use crate::backend::Backend;
use crate::error::Result;
use crate::types::{MountHandle, MountRequest};

pub trait MountManager: Backend {
    fn mount_snapshot(&self, request: &MountRequest) -> Result<MountHandle>;
    fn unmount(&self, handle: &MountHandle) -> Result<()>;
}
