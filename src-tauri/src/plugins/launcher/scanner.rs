#[cfg(any(target_os = "linux", test))]
use std::ffi::OsStr;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde::Serialize;
use walkdir::WalkDir;

use super::search::normalize_text;

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredApp {
    pub name: String,
    pub normalized_name: String,
    pub path: String,
    pub arguments: Option<String>,
    pub working_dir: Option<String>,
    pub icon_path: Option<String>,
    pub source: String,
}

pub fn scan_installed_apps() -> Result<Vec<DiscoveredApp>> {
    let mut apps = BTreeMap::<String, DiscoveredApp>::new();

    for app in platform_scan_installed_apps()? {
        merge_app(&mut apps, app);
    }

    Ok(apps.into_values().collect())
}

#[cfg(target_os = "windows")]
fn platform_scan_installed_apps() -> Result<Vec<DiscoveredApp>> {
    let mut apps = Vec::new();
    apps.extend(scan_windows_start_menu()?);
    apps.extend(scan_windows_registry_app_paths()?);
    Ok(apps)
}

#[cfg(target_os = "linux")]
fn platform_scan_installed_apps() -> Result<Vec<DiscoveredApp>> {
    scan_linux_desktop_entries()
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn platform_scan_installed_apps() -> Result<Vec<DiscoveredApp>> {
    Ok(Vec::new())
}

fn merge_app(store: &mut BTreeMap<String, DiscoveredApp>, candidate: DiscoveredApp) {
    let key = candidate.path.to_ascii_lowercase();

    match store.get_mut(&key) {
        Some(existing) => {
            existing.source = merge_sources(&existing.source, &candidate.source);

            if existing.name.len() < candidate.name.len() && !candidate.name.ends_with(".exe") {
                existing.name = candidate.name.clone();
                existing.normalized_name = candidate.normalized_name.clone();
            }

            if existing.arguments.is_none() {
                existing.arguments = candidate.arguments.clone();
            }

            if existing.working_dir.is_none() {
                existing.working_dir = candidate.working_dir.clone();
            }

            if existing.icon_path.is_none() {
                existing.icon_path = candidate.icon_path.clone();
            }
        }
        None => {
            store.insert(key, candidate);
        }
    }
}

fn merge_sources(left: &str, right: &str) -> String {
    let mut sources = left
        .split(',')
        .chain(right.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    sources.sort_unstable();
    sources.dedup();
    sources.join(",")
}

fn build_app(
    name: impl Into<String>,
    path: impl Into<String>,
    arguments: Option<String>,
    working_dir: Option<String>,
    icon_path: Option<String>,
    source: &str,
) -> Option<DiscoveredApp> {
    let path = path.into();
    if path.trim().is_empty() {
        return None;
    }

    let name = name.into();
    if name.trim().is_empty() {
        return None;
    }

    if command_looks_like_path(&path) && !Path::new(&path).exists() {
        return None;
    }

    Some(DiscoveredApp {
        normalized_name: normalize_text(&name),
        name,
        path,
        arguments,
        working_dir,
        icon_path,
        source: source.to_string(),
    })
}

#[cfg(target_os = "windows")]
fn scan_windows_start_menu() -> Result<Vec<DiscoveredApp>> {
    use lnk::{encoding::WINDOWS_1252, ShellLink};

    let mut apps = Vec::new();
    let mut directories = Vec::new();

    if let Ok(app_data) = std::env::var("APPDATA") {
        directories.push(PathBuf::from(app_data).join("Microsoft\\Windows\\Start Menu\\Programs"));
    }

    if let Ok(program_data) = std::env::var("ProgramData") {
        directories
            .push(PathBuf::from(program_data).join("Microsoft\\Windows\\Start Menu\\Programs"));
    }

    for directory in directories
        .into_iter()
        .filter(|directory| directory.exists())
    {
        for entry in WalkDir::new(&directory).follow_links(false) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };

            let path = entry.path();
            if !entry.file_type().is_file() || !is_shortcut(path) {
                continue;
            }

            let shortcut = match ShellLink::open(path, WINDOWS_1252) {
                Ok(shortcut) => shortcut,
                Err(_) => continue,
            };

            let Some(target) = shortcut.link_target() else {
                continue;
            };

            let target_path = PathBuf::from(target);
            if !is_launchable_target(&target_path) {
                continue;
            }

            let name = shortcut
                .string_data()
                .name_string()
                .as_deref()
                .filter(|name| !name.trim().is_empty())
                .map(str::to_string)
                .or_else(|| {
                    path.file_stem()
                        .and_then(|value| value.to_str())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| target_path_display_name(&target_path));

            let arguments = shortcut
                .string_data()
                .command_line_arguments()
                .as_ref()
                .map(ToOwned::to_owned);
            let working_dir = shortcut
                .string_data()
                .working_dir()
                .as_ref()
                .map(ToOwned::to_owned)
                .or_else(|| {
                    target_path
                        .parent()
                        .map(|directory| directory.to_string_lossy().to_string())
                });
            let icon_path = shortcut
                .string_data()
                .icon_location()
                .as_ref()
                .map(ToOwned::to_owned)
                .or_else(|| Some(target_path.to_string_lossy().to_string()));

            if let Some(app) = build_app(
                name,
                target_path.to_string_lossy().to_string(),
                arguments,
                working_dir,
                icon_path,
                "start_menu",
            ) {
                apps.push(app);
            }
        }
    }

    Ok(apps)
}

#[cfg(target_os = "windows")]
fn scan_windows_registry_app_paths() -> Result<Vec<DiscoveredApp>> {
    use winreg::{enums::*, RegKey};

    let mut apps = Vec::new();
    let roots = [
        (RegKey::predef(HKEY_CURRENT_USER), "HKCU"),
        (RegKey::predef(HKEY_LOCAL_MACHINE), "HKLM"),
    ];
    let registry_path = "Software\\Microsoft\\Windows\\CurrentVersion\\App Paths";

    for (root, hive_name) in roots {
        let Ok(app_paths) = root.open_subkey(registry_path) else {
            continue;
        };

        for key_name in app_paths.enum_keys().flatten() {
            let Ok(entry) = app_paths.open_subkey(&key_name) else {
                continue;
            };

            let Ok(path) = entry.get_value::<String, _>("") else {
                continue;
            };

            let path = PathBuf::from(path.trim_matches('"'));
            if !is_launchable_target(&path) {
                continue;
            }

            let working_dir = entry.get_value::<String, _>("Path").ok().or_else(|| {
                path.parent()
                    .map(|directory| directory.to_string_lossy().to_string())
            });
            let name = registry_app_name(&key_name, &path);

            if let Some(app) = build_app(
                name,
                path.to_string_lossy().to_string(),
                None,
                working_dir,
                Some(path.to_string_lossy().to_string()),
                &format!("registry_app_paths:{hive_name}"),
            ) {
                apps.push(app);
            }
        }
    }

    Ok(apps)
}

#[cfg(target_os = "linux")]
fn scan_linux_desktop_entries() -> Result<Vec<DiscoveredApp>> {
    let mut apps = Vec::new();

    for directory in ["/usr/share/applications", "/usr/local/share/applications"] {
        let path = Path::new(directory);
        if !path.exists() {
            continue;
        }

        for entry in WalkDir::new(path).follow_links(false) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let path = entry.path();

            if !entry.file_type().is_file()
                || path.extension().and_then(|ext| ext.to_str()) != Some("desktop")
            {
                continue;
            }

            let Some(app) = parse_desktop_entry(path)? else {
                continue;
            };
            apps.push(app);
        }
    }

    Ok(apps)
}

#[cfg(target_os = "linux")]
fn parse_desktop_entry(path: &Path) -> Result<Option<DiscoveredApp>> {
    use anyhow::Context;

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read desktop entry {}", path.display()))?;

    let mut name = None;
    let mut exec = None;
    let mut icon = None;

    for line in content.lines() {
        if line.starts_with("Name=") && name.is_none() {
            name = Some(line.trim_start_matches("Name=").trim().to_string());
        } else if line.starts_with("Exec=") && exec.is_none() {
            exec = Some(line.trim_start_matches("Exec=").trim().to_string());
        } else if line.starts_with("Icon=") && icon.is_none() {
            icon = Some(line.trim_start_matches("Icon=").trim().to_string());
        }
    }

    let Some(exec) = exec else {
        return Ok(None);
    };

    let path_env = std::env::var_os("PATH");
    let Some(parsed_exec) = parse_desktop_exec(&exec, path_env.as_deref()) else {
        return Ok(None);
    };

    Ok(build_app(
        name.unwrap_or_else(|| target_path_display_name(Path::new(&parsed_exec.command))),
        parsed_exec.command,
        parsed_exec.arguments,
        parsed_exec.working_dir,
        icon,
        "desktop_entry",
    ))
}

#[cfg(any(not(target_os = "windows"), test))]
pub(crate) fn split_command_line_words(input: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;

    for character in input.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }

        match quote {
            Some('"') => match character {
                '"' => quote = None,
                '\\' => escaped = true,
                _ => current.push(character),
            },
            Some('\'') => {
                if character == '\'' {
                    quote = None;
                } else {
                    current.push(character);
                }
            }
            Some(_) => unreachable!(),
            None => match character {
                '"' | '\'' => quote = Some(character),
                '\\' => escaped = true,
                value if value.is_whitespace() => {
                    if !current.is_empty() {
                        words.push(std::mem::take(&mut current));
                    }
                }
                _ => current.push(character),
            },
        }
    }

    if escaped {
        current.push('\\');
    }

    if !current.is_empty() {
        words.push(current);
    }

    words
}

#[cfg(any(target_os = "linux", test))]
#[derive(Debug, PartialEq, Eq)]
struct ParsedDesktopExec {
    command: String,
    arguments: Option<String>,
    working_dir: Option<String>,
}

#[cfg(any(target_os = "linux", test))]
fn parse_desktop_exec(exec: &str, path_env: Option<&OsStr>) -> Option<ParsedDesktopExec> {
    let mut parts = split_command_line_words(exec)
        .into_iter()
        .map(|part| strip_desktop_field_codes(&part))
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    let original_command = parts.first()?.clone();
    let command = resolve_command_path(&original_command, path_env)
        .unwrap_or_else(|| PathBuf::from(&original_command))
        .to_string_lossy()
        .to_string();
    let arguments = serialize_command_line(&parts.drain(1..).collect::<Vec<_>>());
    let working_dir = command_looks_like_path(&original_command)
        .then(|| Path::new(&command).parent())
        .flatten()
        .map(|directory| directory.to_string_lossy().to_string());

    Some(ParsedDesktopExec {
        command,
        arguments,
        working_dir,
    })
}

#[cfg(any(target_os = "linux", test))]
fn strip_desktop_field_codes(input: &str) -> String {
    let mut cleaned = String::with_capacity(input.len());
    let mut characters = input.chars().peekable();

    while let Some(character) = characters.next() {
        if character != '%' {
            cleaned.push(character);
            continue;
        }

        match characters.peek().copied() {
            Some('%') => {
                cleaned.push('%');
                characters.next();
            }
            Some(value) if value.is_ascii_alphabetic() => {
                characters.next();
            }
            _ => cleaned.push(character),
        }
    }

    cleaned
}

#[cfg(any(target_os = "linux", test))]
fn serialize_command_line(parts: &[String]) -> Option<String> {
    if parts.is_empty() {
        return None;
    }

    Some(
        parts
            .iter()
            .map(|part| quote_command_line_word(part))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

#[cfg(any(target_os = "linux", test))]
fn quote_command_line_word(part: &str) -> String {
    if part.is_empty() {
        return "\"\"".to_string();
    }

    let requires_quotes = part
        .chars()
        .any(|character| character.is_whitespace() || matches!(character, '"' | '\\'));

    if !requires_quotes {
        return part.to_string();
    }

    let mut quoted = String::from("\"");
    for character in part.chars() {
        if matches!(character, '"' | '\\') {
            quoted.push('\\');
        }
        quoted.push(character);
    }
    quoted.push('"');
    quoted
}

fn command_looks_like_path(command: &str) -> bool {
    let command = command.trim();
    !command.is_empty()
        && (Path::new(command).is_absolute()
            || command.starts_with('.')
            || command.contains(std::path::MAIN_SEPARATOR)
            || command.contains('/')
            || command.contains('\\'))
}

#[cfg(any(target_os = "linux", test))]
fn resolve_command_path(command: &str, path_env: Option<&OsStr>) -> Option<PathBuf> {
    if command_looks_like_path(command) {
        return Some(PathBuf::from(command));
    }

    let path_env = path_env?;
    for directory in std::env::split_paths(path_env) {
        let candidate = directory.join(command);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn is_shortcut(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("lnk"))
        .unwrap_or(false)
}

fn is_launchable_target(path: &Path) -> bool {
    path.exists()
        && path
            .extension()
            .and_then(|value| value.to_str())
            .map(is_launchable_extension)
            .unwrap_or(true)
}

fn is_launchable_extension(extension: &str) -> bool {
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "exe" | "bat" | "cmd" | "com" | "msc" | "appref-ms" | "desktop"
    )
}

fn target_path_display_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

#[cfg(target_os = "windows")]
fn registry_app_name(key_name: &str, path: &Path) -> String {
    Path::new(key_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| target_path_display_name(path))
}

#[cfg(test)]
mod tests {
    use super::{parse_desktop_exec, split_command_line_words, strip_desktop_field_codes};
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn unique_temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after UNIX_EPOCH")
            .as_nanos();
        std::env::temp_dir().join(format!("entrance-{name}-{suffix}"))
    }

    #[test]
    fn command_line_parser_preserves_quoted_words() {
        let words =
            split_command_line_words(r#""/opt/Visual Studio Code/code" --profile "My User""#);

        assert_eq!(
            words,
            vec![
                "/opt/Visual Studio Code/code".to_string(),
                "--profile".to_string(),
                "My User".to_string(),
            ]
        );
    }

    #[test]
    fn strips_desktop_placeholders_but_keeps_literal_percent() {
        assert_eq!(strip_desktop_field_codes("firefox"), "firefox");
        assert_eq!(strip_desktop_field_codes("%U"), "");
        assert_eq!(strip_desktop_field_codes("--open=%f"), "--open=");
        assert_eq!(strip_desktop_field_codes("100%%"), "100%");
    }

    #[test]
    fn linux_desktop_exec_resolves_path_commands_and_drops_placeholders() {
        let temp_dir = unique_temp_dir("desktop-exec");
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let binary_path = temp_dir.join("firefox");
        fs::write(&binary_path, "#!/bin/sh\n").expect("fake executable should be created");
        let path_env = temp_dir.as_os_str().to_os_string();

        let parsed = parse_desktop_exec("firefox %U", Some(path_env.as_os_str()))
            .expect("desktop exec should parse");

        assert_eq!(parsed.command, binary_path.to_string_lossy().to_string());
        assert_eq!(parsed.arguments, None);
        assert_eq!(parsed.working_dir, None);

        fs::remove_file(&binary_path).ok();
        fs::remove_dir(&temp_dir).ok();
    }

    #[test]
    fn linux_desktop_exec_keeps_bare_commands_when_path_lookup_misses() {
        let parsed = parse_desktop_exec("firefox %U", None).expect("desktop exec should parse");

        assert_eq!(parsed.command, "firefox");
        assert_eq!(parsed.arguments, None);
        assert_eq!(parsed.working_dir, None);
    }

    #[test]
    fn linux_desktop_exec_keeps_quoted_arguments() {
        let parsed = parse_desktop_exec(
            r#""/opt/Visual Studio Code/code" --profile "My User" %F"#,
            None,
        )
        .expect("desktop exec should parse");

        assert_eq!(parsed.command, "/opt/Visual Studio Code/code");
        assert_eq!(parsed.arguments.as_deref(), Some("--profile \"My User\""));
        assert_eq!(
            parsed.working_dir.as_deref(),
            Some("/opt/Visual Studio Code")
        );
    }
}
