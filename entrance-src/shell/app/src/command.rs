#[derive(Debug, Clone)]
pub enum Command {
    Help,
    Status,
    Drawer(Vec<String>),
    Hive(Vec<String>),
    Launcher(Vec<String>),
    DaemonStdio,
    DaemonHttp,
    McpStdio,
}
