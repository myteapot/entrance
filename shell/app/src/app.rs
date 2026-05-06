use anyhow::Result;
use entrance_core::Plugin;
use entrance_core::{boot, AppKernel, PluginContext};
use entrance_drawer::DrawerPlugin;
use entrance_hive::HivePlugin;
use entrance_launcher::LauncherPlugin;

#[derive(Debug, Clone)]
pub struct AppServices {
    pub kernel: AppKernel,
    pub drawer: DrawerPlugin,
    pub hive: HivePlugin,
    pub launcher: LauncherPlugin,
}

pub fn boot_services() -> Result<AppServices> {
    let kernel = boot()?;
    let ctx = PluginContext {
        kernel: kernel.clone(),
    };
    let drawer = DrawerPlugin::new(&ctx);
    let hive = HivePlugin::new(&ctx);
    let launcher = LauncherPlugin::new(&ctx);

    drawer.init(&ctx)?;
    hive.init(&ctx)?;
    launcher.init(&ctx)?;

    Ok(AppServices {
        drawer,
        hive,
        launcher,
        kernel,
    })
}
