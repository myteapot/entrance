use std::path::{Path, PathBuf};

use anyhow::Result;
use dirs::home_dir;
use entrance_core::LauncherEntryCreate;
use walkdir::WalkDir;

pub fn scan(extra_scan_paths: &[String]) -> Result<Vec<LauncherEntryCreate>> {
    let mut paths = default_scan_paths();
    for path in extra_scan_paths {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            paths.push(candidate);
        }
    }

    let mut entries = Vec::new();
    for path in paths {
        collect_directory(&path, &mut entries);
    }

    Ok(entries)
}

fn collect_directory(root: &Path, entries: &mut Vec<LauncherEntryCreate>) {
    if !root.exists() {
        return;
    }

    for entry in WalkDir::new(root).max_depth(3) {
        let entry = match entry {
            Ok(value) => value,
            Err(_) => continue,
        };
        let path = entry.path();
        if !is_launchable_entry(
            entry.file_type().is_file(),
            entry.file_type().is_dir(),
            path,
        ) {
            continue;
        }

        let Some(name) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };

        entries.push(LauncherEntryCreate {
            name: name.to_string(),
            command: path.display().to_string(),
            arguments: None,
            working_dir: path.parent().map(|value| value.display().to_string()),
            source: source_name(root),
            pinned: false,
        });
    }
}

fn default_scan_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(target_os = "macos")]
    {
        paths.push(PathBuf::from("/Applications"));
        if let Some(home) = home_dir() {
            paths.push(home.join("Applications"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        paths.push(PathBuf::from("/usr/share/applications"));
        paths.push(PathBuf::from("/usr/local/share/applications"));
        if let Some(home) = home_dir() {
            paths.push(home.join(".local/share/applications"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(home) = home_dir() {
            paths.push(home.join("AppData/Roaming/Microsoft/Windows/Start Menu/Programs"));
        }
    }

    paths
}

fn is_launchable(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        return path.extension().and_then(|value| value.to_str()) == Some("app");
    }

    #[cfg(target_os = "linux")]
    {
        return matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("desktop") | Some("AppImage")
        );
    }

    #[cfg(target_os = "windows")]
    {
        return matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("exe") | Some("lnk")
        );
    }

    #[allow(unreachable_code)]
    false
}

fn is_launchable_entry(is_file: bool, is_dir: bool, path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        return (is_dir || is_file) && is_launchable(path);
    }

    #[cfg(not(target_os = "macos"))]
    {
        is_file && is_launchable(path)
    }
}

fn source_name(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("scan")
        .to_string()
}
