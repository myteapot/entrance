pub mod server;
pub mod stdio_client;

#[allow(unused_imports)]
mod core {
    pub(crate) use entrance_core::nota as nota_runtime;
    pub(crate) use entrance_core::*;
    pub(crate) use entrance_harness::bootstrap_mcp_cycle;
    pub(crate) use entrance_harness::{
        boot_for_paths as bootstrap_for_paths, resolve_app_data_dir, HarnessPaths as AppPaths,
        RuntimeServices as StartupState,
    };
}

#[allow(unused_imports)]
mod hosts {
    pub mod plugins {
        pub(crate) use entrance_harness::plugins::*;
    }
}

pub use entrance_core::overview::{build_nota_runtime_overview, build_nota_runtime_status};
pub use server::{McpPluginSet, McpServer, McpTransport};
