use anyhow::Result;

use crate::{AppKernel, Bus, Crypto, FileSystem, Scheduler, Store, Supervision, Versioning};

#[derive(Debug, Clone)]
pub struct PluginContext {
    pub kernel: AppKernel,
}

impl PluginContext {
    pub fn store(&self) -> Store {
        self.kernel.store.clone()
    }

    pub fn bus(&self) -> Bus {
        self.kernel.bus.clone()
    }

    pub fn versioning(&self) -> Versioning {
        self.kernel.versioning.clone()
    }

    pub fn crypto(&self) -> Crypto {
        self.kernel.crypto.clone()
    }

    pub fn fs(&self) -> FileSystem {
        self.kernel.fs.clone()
    }

    pub fn scheduler(&self) -> Scheduler {
        self.kernel.scheduler.clone()
    }

    pub fn supervision(&self) -> Supervision {
        self.kernel.supervision.clone()
    }
}

pub trait Plugin {
    fn name(&self) -> &'static str;
    fn init(&self, _ctx: &PluginContext) -> Result<()> {
        Ok(())
    }
}
