use crate::process::hidden_command;
use serde::Serialize;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

const MAX_INSTALL_DEPTH: usize = 2;
const MAX_INSTALL_RESULTS_PER_ROOT: usize = 80;
const MAX_PATH_RESULTS_PER_ROOT: usize = 12;
const APP_SCAN_CACHE_TTL: Duration = Duration::from_secs(300);

struct AppScanCache {
    scanned_at: Instant,
    apps: Vec<AppEntry>,
    refresh_in_progress: bool,
}

static APP_SCAN_CACHE: OnceLock<Mutex<Option<AppScanCache>>> = OnceLock::new();

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub path: String,
    pub source: String,
    pub aliases: Vec<String>,
}

pub fn scan_apps() -> Vec<AppEntry> {
    let cache = APP_SCAN_CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(mut cached) = cache.lock() {
        if let Some(snapshot) = cached.as_ref() {
            if snapshot.scanned_at.elapsed() < APP_SCAN_CACHE_TTL {
                return snapshot.apps.clone();
            }

            let apps = snapshot.apps.clone();
            if !snapshot.refresh_in_progress {
                if let Some(snapshot) = cached.as_mut() {
                    snapshot.refresh_in_progress = true;
                }
                spawn_app_scan_refresh();
            }
            return apps;
        }

        *cached = Some(AppScanCache {
            scanned_at: Instant::now() - APP_SCAN_CACHE_TTL,
            apps: Vec::new(),
            refresh_in_progress: true,
        });
        spawn_app_scan_refresh();
        return Vec::new();
    }

    scan_apps_uncached()
}

pub fn warm_app_scan_cache() {
    let cache = APP_SCAN_CACHE.get_or_init(|| Mutex::new(None));
    let should_refresh = if let Ok(mut cached) = cache.lock() {
        match cached.as_ref() {
            Some(snapshot) if snapshot.scanned_at.elapsed() < APP_SCAN_CACHE_TTL => false,
            Some(snapshot) if snapshot.refresh_in_progress => false,
            _ => {
                *cached = Some(AppScanCache {
                    scanned_at: Instant::now() - APP_SCAN_CACHE_TTL,
                    apps: cached
                        .as_ref()
                        .map(|snapshot| snapshot.apps.clone())
                        .unwrap_or_default(),
                    refresh_in_progress: true,
                });
                true
            }
        }
    } else {
        false
    };

    if should_refresh {
        spawn_app_scan_refresh();
    }
}

fn spawn_app_scan_refresh() {
    thread::spawn(|| {
        let apps = scan_apps_uncached();
        let cache = APP_SCAN_CACHE.get_or_init(|| Mutex::new(None));
        if let Ok(mut cached) = cache.lock() {
            *cached = Some(AppScanCache {
                scanned_at: Instant::now(),
                apps,
                refresh_in_progress: false,
            });
        }
    });
}

#[cfg(test)]
pub fn replace_app_scan_cache_for_test(apps: Vec<AppEntry>, scanned_at: Instant) {
    let cache = APP_SCAN_CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(mut cached) = cache.lock() {
        *cached = Some(AppScanCache {
            scanned_at,
            apps,
            refresh_in_progress: false,
        });
    }
}

fn scan_apps_uncached() -> Vec<AppEntry> {
    let mut seen_paths = HashSet::new();
    let mut apps = Vec::new();

    for (source, root) in shortcut_roots() {
        collect_shortcut_files(&root, &source, &mut seen_paths, &mut apps);
    }

    for (source, root) in install_roots() {
        collect_exe_files(
            &root,
            &source,
            0,
            MAX_INSTALL_RESULTS_PER_ROOT,
            &mut seen_paths,
            &mut apps,
        );
    }

    collect_registry_install_apps(&mut seen_paths, &mut apps);

    for root in path_roots() {
        collect_exe_files(
            &root,
            "PATH",
            0,
            MAX_PATH_RESULTS_PER_ROOT,
            &mut seen_paths,
            &mut apps,
        );
    }

    apps.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    apps
}

pub fn shortcut_roots() -> Vec<(String, PathBuf)> {
    let mut roots = Vec::new();

    if let Ok(app_data) = env::var("APPDATA") {
        roots.push((
            "开始菜单".into(),
            PathBuf::from(app_data).join("Microsoft\\Windows\\Start Menu"),
        ));
    }

    if let Ok(program_data) = env::var("PROGRAMDATA") {
        roots.push((
            "公共开始菜单".into(),
            PathBuf::from(program_data).join("Microsoft\\Windows\\Start Menu"),
        ));
    }

    if let Ok(user_profile) = env::var("USERPROFILE") {
        roots.push(("桌面".into(), PathBuf::from(user_profile).join("Desktop")));
    }

    if let Ok(public_dir) = env::var("PUBLIC") {
        roots.push(("公共桌面".into(), PathBuf::from(public_dir).join("Desktop")));
    }

    roots
}

pub fn install_roots() -> Vec<(String, PathBuf)> {
    let mut roots = Vec::new();

    if let Ok(program_files) = env::var("ProgramFiles") {
        roots.push(("Program Files".into(), PathBuf::from(program_files)));
    }

    if let Ok(program_files_x86) = env::var("ProgramFiles(x86)") {
        roots.push((
            "Program Files (x86)".into(),
            PathBuf::from(program_files_x86),
        ));
    }

    if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
        let local_app_data = PathBuf::from(local_app_data);
        roots.push(("Local Programs".into(), local_app_data.join("Programs")));
        roots.push((
            "WindowsApps".into(),
            local_app_data.join("Microsoft\\WindowsApps"),
        ));
    }

    roots
}

pub fn path_roots() -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    env::var_os("PATH")
        .into_iter()
        .flat_map(|value| env::split_paths(&value).collect::<Vec<_>>())
        .filter(|path| path.exists() && path.is_dir())
        .filter(|path| seen.insert(path.display().to_string().to_lowercase()))
        .collect()
}

pub fn launch_app(path: &str) -> Result<(), String> {
    hidden_command("cmd")
        .args(["/C", "start", "", path])
        .spawn()
        .map_err(|error| format!("启动失败：{error}"))?;

    Ok(())
}

fn collect_shortcut_files(
    root: &Path,
    source: &str,
    seen_paths: &mut HashSet<String>,
    apps: &mut Vec<AppEntry>,
) {
    if !root.exists() {
        return;
    }

    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            collect_shortcut_files(&path, source, seen_paths, apps);
            continue;
        }

        let is_supported_shortcut = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("lnk") || extension.eq_ignore_ascii_case("url")
            });

        if !is_supported_shortcut {
            continue;
        }

        let normalized_path = path.display().to_string();
        if !seen_paths.insert(normalized_path.clone()) {
            continue;
        }

        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("Unknown")
            .to_string();
        if should_skip_program_name(&name) {
            continue;
        }
        let display_name = clean_display_name(&name);

        apps.push(AppEntry {
            id: format!("app:{}", normalized_path.to_lowercase()),
            aliases: app_aliases(&display_name, &normalized_path),
            name: display_name,
            path: normalized_path,
            source: source.to_string(),
        });
    }
}

fn collect_exe_files(
    root: &Path,
    source: &str,
    depth: usize,
    max_results: usize,
    seen_paths: &mut HashSet<String>,
    apps: &mut Vec<AppEntry>,
) {
    if depth > MAX_INSTALL_DEPTH || !root.exists() {
        return;
    }

    let initial_count = apps.len();
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        if apps.len().saturating_sub(initial_count) >= max_results {
            return;
        }

        let path = entry.path();

        if path.is_dir() {
            collect_exe_files(&path, source, depth + 1, max_results, seen_paths, apps);
            continue;
        }

        let is_exe = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"));

        if !is_exe {
            continue;
        }

        let normalized_path = path.display().to_string();
        if !seen_paths.insert(normalized_path.clone()) {
            continue;
        }

        let Some(file_stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };

        if should_skip_program_name(file_stem) {
            continue;
        }
        let display_name = clean_display_name(file_stem);

        apps.push(AppEntry {
            id: format!("app:{}", normalized_path.to_lowercase()),
            aliases: app_aliases(&display_name, &normalized_path),
            name: display_name,
            path: normalized_path,
            source: source.to_string(),
        });
    }
}

#[derive(Debug, Default, PartialEq)]
struct RegistryInstallEntry {
    display_name: Option<String>,
    install_location: Option<String>,
    display_icon: Option<String>,
}

fn collect_registry_install_apps(seen_paths: &mut HashSet<String>, apps: &mut Vec<AppEntry>) {
    for root in uninstall_registry_roots() {
        let Ok(output) = hidden_command("reg").args(["query", root, "/s"]).output() else {
            continue;
        };
        if !output.status.success() {
            continue;
        }

        let text = String::from_utf8_lossy(&output.stdout);
        for entry in parse_registry_install_entries(&text) {
            collect_registry_install_entry(&entry, seen_paths, apps);
        }
    }
}

fn uninstall_registry_roots() -> &'static [&'static str] {
    &[
        r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
    ]
}

fn parse_registry_install_entries(output: &str) -> Vec<RegistryInstallEntry> {
    let mut entries = Vec::new();
    let mut current = RegistryInstallEntry::default();

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("HKEY_")
            || trimmed.starts_with("HKCU\\")
            || trimmed.starts_with("HKLM\\")
        {
            push_registry_install_entry(&mut entries, &mut current);
            continue;
        }

        if let Some((name, value)) = parse_registry_value_line(trimmed) {
            match name {
                "DisplayName" => current.display_name = Some(value.to_string()),
                "InstallLocation" => current.install_location = Some(value.to_string()),
                "DisplayIcon" => current.display_icon = Some(value.to_string()),
                _ => {}
            }
        }
    }

    push_registry_install_entry(&mut entries, &mut current);
    entries
}

fn push_registry_install_entry(
    entries: &mut Vec<RegistryInstallEntry>,
    current: &mut RegistryInstallEntry,
) {
    if current.display_name.is_some()
        || current.install_location.is_some()
        || current.display_icon.is_some()
    {
        entries.push(std::mem::take(current));
    }
}

fn parse_registry_value_line(line: &str) -> Option<(&str, &str)> {
    let mut parts = line.split_whitespace();
    let name = parts.next()?;
    let value_type = parts.next()?;
    if !value_type.starts_with("REG_") {
        return None;
    }
    let value_start = line.find(value_type)? + value_type.len();
    Some((name, line[value_start..].trim()))
}

fn collect_registry_install_entry(
    entry: &RegistryInstallEntry,
    seen_paths: &mut HashSet<String>,
    apps: &mut Vec<AppEntry>,
) {
    let display_name = entry
        .display_name
        .as_deref()
        .map(clean_display_name)
        .filter(|name| !name.is_empty() && !should_skip_program_name(name));

    if let Some(path) = entry
        .display_icon
        .as_deref()
        .and_then(registry_display_icon_exe_path)
    {
        push_registry_app(path, display_name.as_deref(), seen_paths, apps);
    }

    if let Some(location) = entry.install_location.as_deref() {
        collect_registry_install_location(location, display_name.as_deref(), seen_paths, apps);
    }
}

fn collect_registry_install_location(
    install_location: &str,
    display_name: Option<&str>,
    seen_paths: &mut HashSet<String>,
    apps: &mut Vec<AppEntry>,
) {
    let root = PathBuf::from(install_location.trim().trim_matches('"'));
    if !root.is_dir() {
        return;
    }

    if let Some(display_name) = display_name {
        let compact_name = compact_alias(display_name).unwrap_or_default();
        let mut candidate_paths = Vec::new();
        if !compact_name.is_empty() {
            candidate_paths.push(root.join(format!("{compact_name}.exe")));
        }
        for word in words(display_name) {
            candidate_paths.push(root.join(format!("{}.exe", word.to_lowercase())));
            candidate_paths.push(root.join(format!("{word}.exe")));
        }

        for candidate in candidate_paths {
            if candidate.is_file() {
                push_registry_app(candidate, Some(display_name), seen_paths, apps);
                return;
            }
        }
    }

    collect_exe_files(&root, "注册表安装位置", 0, 8, seen_paths, apps);
}

fn registry_display_icon_exe_path(value: &str) -> Option<PathBuf> {
    let mut text = value.trim();
    if text.is_empty() {
        return None;
    }

    if let Some(stripped) = text.strip_prefix('"') {
        let end = stripped.find('"')?;
        text = &stripped[..end];
    } else if let Some(index) = text.to_lowercase().find(".exe") {
        text = &text[..index + 4];
    }

    let path = PathBuf::from(text.trim().trim_matches('"'));
    path.is_file().then_some(path)
}

fn push_registry_app(
    path: PathBuf,
    display_name: Option<&str>,
    seen_paths: &mut HashSet<String>,
    apps: &mut Vec<AppEntry>,
) {
    let normalized_path = path.display().to_string();
    if !seen_paths.insert(normalized_path.clone()) {
        return;
    }

    let file_stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("Unknown");
    if should_skip_program_name(file_stem) {
        return;
    }

    let name = display_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| clean_display_name(file_stem));

    apps.push(AppEntry {
        id: format!("app:{}", normalized_path.to_lowercase()),
        aliases: app_aliases(&name, &normalized_path),
        name,
        path: normalized_path,
        source: "注册表安装位置".into(),
    });
}

fn should_skip_program_name(name: &str) -> bool {
    let normalized_name = name.to_lowercase();
    let noisy_contains = [
        "unins",
        "uninstall",
        "uninstaller",
        "卸载",
        "卸載",
        "desinstalar",
        "désinstaller",
        "deinstallieren",
        "disinstallare",
        "install",
        "setup",
        "installer",
        "updater",
        "updatehelper",
        "crashpad_handler",
        "crashreporter",
        "maintenancetool",
        "squirrel",
    ];

    noisy_contains
        .iter()
        .any(|suffix| normalized_name.contains(suffix))
}

fn clean_display_name(name: &str) -> String {
    let mut cleaned = name.trim().to_string();
    let suffixes = [
        " - Shortcut",
        " - shortcut",
        " - 快捷方式",
        " 快捷方式",
        " shortcut",
        " Shortcut",
    ];

    for suffix in suffixes {
        if let Some(stripped) = cleaned.strip_suffix(suffix) {
            cleaned = stripped.trim().to_string();
            break;
        }
    }

    if cleaned.is_empty() {
        name.trim().to_string()
    } else {
        cleaned
    }
}

fn app_aliases(name: &str, path: &str) -> Vec<String> {
    let mut aliases = Vec::new();
    push_unique_alias(&mut aliases, compact_alias(name));
    push_unique_alias(&mut aliases, abbreviation_alias(name));
    push_unique_alias(&mut aliases, initials_plus_last_word_alias(name));

    if let Some(file_name) = path.rsplit(['\\', '/']).next() {
        if let Some(stem) = file_name
            .rsplit_once('.')
            .map(|(stem, _)| stem)
            .or(Some(file_name))
        {
            push_unique_alias(&mut aliases, Some(stem.to_string()));
            push_unique_alias(&mut aliases, compact_alias(stem));
        }
    }

    aliases
}

fn push_unique_alias(aliases: &mut Vec<String>, alias: Option<String>) {
    let Some(alias) = alias else {
        return;
    };
    let alias = alias.trim().to_lowercase();
    if alias.len() > 1 && !aliases.iter().any(|existing| existing == &alias) {
        aliases.push(alias);
    }
}

fn compact_alias(text: &str) -> Option<String> {
    let compact = text
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(|character| character.to_lowercase())
        .collect::<String>();

    if compact.is_empty() {
        None
    } else {
        Some(compact)
    }
}

fn abbreviation_alias(text: &str) -> Option<String> {
    let abbreviation = words(text)
        .into_iter()
        .filter_map(|word| word.chars().next())
        .collect::<String>();

    if abbreviation.is_empty() {
        None
    } else {
        Some(abbreviation)
    }
}

fn initials_plus_last_word_alias(text: &str) -> Option<String> {
    let parts = words(text);
    if parts.len() < 2 {
        return None;
    }

    let mut alias = parts[..parts.len() - 1]
        .iter()
        .filter_map(|word| word.chars().next())
        .collect::<String>();
    alias.push_str(parts[parts.len() - 1]);

    Some(alias)
}

fn words(text: &str) -> Vec<&str> {
    text.split(|character: char| {
        !character.is_ascii_alphanumeric() && !matches!(character, '+' | '#')
    })
    .filter(|part| !part.is_empty())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_roots_uses_windows_locations_when_available() {
        let roots = shortcut_roots();

        assert!(roots.iter().all(|(_, path)| !path.as_os_str().is_empty()));
    }

    #[test]
    fn path_roots_are_existing_directories() {
        let roots = path_roots();

        assert!(roots.iter().all(|path| path.is_dir()));
    }

    #[test]
    fn skips_noisy_executables() {
        assert!(should_skip_program_name("uninstall"));
        assert!(should_skip_program_name("MyAppUpdater"));
        assert!(!should_skip_program_name("Code"));
    }

    #[test]
    fn expired_cache_returns_snapshot_without_blocking_for_refresh() {
        replace_app_scan_cache_for_test(
            vec![AppEntry {
                id: "app:cached".into(),
                name: "Cached App".into(),
                path: r"C:\Apps\cached.exe".into(),
                source: "测试".into(),
                aliases: Vec::new(),
            }],
            Instant::now() - APP_SCAN_CACHE_TTL - Duration::from_secs(1),
        );

        let apps = scan_apps();

        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].id, "app:cached");
    }

    #[test]
    fn cleans_shortcut_display_names() {
        assert_eq!(
            clean_display_name("Visual Studio Code - Shortcut"),
            "Visual Studio Code"
        );
        assert_eq!(clean_display_name("微信 快捷方式"), "微信");
    }

    #[test]
    fn builds_flow_style_program_aliases() {
        let aliases = app_aliases("Visual Studio Code", r"C:\Apps\Code.exe");

        assert!(aliases.contains(&"vsc".into()));
        assert!(aliases.contains(&"vscode".into()));
        assert!(aliases.contains(&"code".into()));
    }

    #[test]
    fn parses_registry_uninstall_entries() {
        let output = r#"
HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Mozilla Thunderbird
    DisplayName    REG_SZ    Mozilla Thunderbird
    InstallLocation    REG_SZ    E:\Mozilla Thunderbird
    DisplayIcon    REG_SZ    E:\Mozilla Thunderbird\thunderbird.exe,0

HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Other
    DisplayName    REG_SZ    Other App
"#;

        let entries = parse_registry_install_entries(output);

        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0],
            RegistryInstallEntry {
                display_name: Some("Mozilla Thunderbird".into()),
                install_location: Some(r"E:\Mozilla Thunderbird".into()),
                display_icon: Some(r"E:\Mozilla Thunderbird\thunderbird.exe,0".into()),
            }
        );
        assert_eq!(entries[1].display_name.as_deref(), Some("Other App"));
    }

    #[test]
    fn registry_install_location_finds_display_name_exe() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-launcher-registry-install-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_root).expect("create temp install root");
        let executable = temp_root.join("thunderbird.exe");
        std::fs::write(&executable, "").expect("create executable");
        let mut seen_paths = HashSet::new();
        let mut apps = Vec::new();

        collect_registry_install_location(
            temp_root.to_str().expect("utf-8 temp path"),
            Some("Mozilla Thunderbird"),
            &mut seen_paths,
            &mut apps,
        );

        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Mozilla Thunderbird");
        assert_eq!(apps[0].path, executable.display().to_string());
        assert_eq!(apps[0].source, "注册表安装位置");
        assert!(apps[0].aliases.contains(&"thunderbird".into()));

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn skips_localized_uninstallers() {
        assert!(should_skip_program_name("卸载 MyApp"));
    }
}
