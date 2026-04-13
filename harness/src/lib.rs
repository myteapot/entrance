pub mod bootstrap_mcp_cycle;
pub mod config;
mod environment_runtime;
pub mod paths;
pub mod plugins;
pub mod projections;
pub mod runtime;

pub use paths::HarnessPaths;
pub use projections::{
    rebuild_nota_projections, write_hot_root_projection, HotRootProjectionWriteReport,
    ProjectionRebuildReport,
};
pub use runtime::{
    boot, boot_for_paths, boot_for_root, resolve_app_data_dir, resolve_runtime_paths,
    RuntimeServices,
};

#[allow(dead_code)]
type AppPaths = HarnessPaths;
#[allow(dead_code)]
type StartupState = RuntimeServices;

#[allow(unused_imports)]
mod core {
    pub mod config_store {
        pub(crate) use crate::config::*;
    }

    pub(crate) use crate::{
        boot_for_paths as bootstrap_for_paths, resolve_runtime_paths, AppPaths,
    };
    pub(crate) use entrance_core::nota as nota_runtime;
    pub(crate) use entrance_core::*;
}

#[allow(unused_imports)]
mod hosts {
    pub mod desktop {
        pub mod hotkey {
            pub const DEFAULT_LAUNCHER_HOTKEY: &str = "Alt+Space";
        }
    }

    pub mod plugins {
        pub(crate) use crate::plugins::*;
    }
}

#[cfg(test)]
use std::sync::{Mutex, OnceLock};

#[cfg(test)]
static TEST_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(test)]
pub(crate) fn test_env_guard() -> std::sync::MutexGuard<'static, ()> {
    TEST_ENV_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("test environment lock should not be poisoned")
}
