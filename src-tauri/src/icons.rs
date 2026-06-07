use crate::search::{ResultKind, SearchResult};
use crate::storage;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, UNIX_EPOCH};

const ICON_CACHE_DIR: &str = "icons";
const CACHE_PRUNE_MAX_FILES: usize = 512;
const CACHE_PRUNE_MAX_AGE: Duration = Duration::from_secs(90 * 24 * 60 * 60);
const CACHE_PRUNE_INTERVAL: Duration = Duration::from_secs(30 * 60);

static LAST_CACHE_PRUNE: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
static ICON_EXTRACTION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IconRequest {
    pub result_id: String,
    target: IconTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum IconTarget {
    Folder,
    FileType { extension: Option<String> },
    Path { path: String },
    Shortcut { path: String },
    Url,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IconCacheStatus {
    pub directory: String,
    pub file_count: usize,
    pub size_bytes: u64,
}

pub fn attach_cached_icons(results: &mut [SearchResult]) {
    for result in results {
        if result.icon_path.is_some() {
            continue;
        }

        let Some(target) = icon_target_for_result(result) else {
            continue;
        };
        let Some(path) = cache_path_for_target(&target) else {
            continue;
        };

        if path.is_file() {
            result.icon_path = Some(path.display().to_string());
        }
    }
}

pub fn pending_icon_requests(results: &[SearchResult], limit: usize) -> Vec<IconRequest> {
    results
        .iter()
        .take(limit)
        .filter(|result| result.icon_path.is_none())
        .filter_map(|result| {
            icon_target_for_result(result).map(|target| IconRequest {
                result_id: result.id.clone(),
                target,
            })
        })
        .collect()
}

pub fn resolve_icon_request(request: &IconRequest) -> Option<String> {
    let output_path = cache_path_for_target(&request.target)?;
    if output_path.is_file() {
        return Some(output_path.display().to_string());
    }

    let cache_dir = cache_dir().ok()?;
    fs::create_dir_all(&cache_dir).ok()?;
    prune_cache_periodically();

    #[cfg(windows)]
    {
        let extraction_lock = ICON_EXTRACTION_LOCK.get_or_init(|| Mutex::new(()));
        let Ok(_extraction_guard) = extraction_lock.lock() else {
            return None;
        };
        if output_path.is_file() {
            return Some(output_path.display().to_string());
        }
        if extract_target_icon(&request.target, &output_path) {
            return Some(output_path.display().to_string());
        }
    }

    #[cfg(not(windows))]
    {
        let _ = request;
    }

    None
}

pub fn icon_cache_status() -> Result<IconCacheStatus, String> {
    let directory = cache_dir().map_err(|error| error.to_string())?;
    let mut file_count = 0usize;
    let mut size_bytes = 0u64;

    if directory.is_dir() {
        for entry in fs::read_dir(&directory).map_err(|error| error.to_string())? {
            let Ok(entry) = entry else {
                continue;
            };
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if metadata.is_file() {
                file_count += 1;
                size_bytes += metadata.len();
            }
        }
    }

    Ok(IconCacheStatus {
        directory: directory.display().to_string(),
        file_count,
        size_bytes,
    })
}

pub fn clear_icon_cache() -> Result<usize, String> {
    let directory = cache_dir().map_err(|error| error.to_string())?;
    if !directory.is_dir() {
        return Ok(0);
    }

    let mut cleared = 0usize;
    for entry in fs::read_dir(&directory).map_err(|error| error.to_string())? {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if fs::remove_file(&path).is_ok() {
            cleared += 1;
        }
    }

    Ok(cleared)
}

fn icon_target_for_result(result: &SearchResult) -> Option<IconTarget> {
    if result.id.starts_with("custom-command:") {
        return custom_command_icon_target(&result.subtitle);
    }

    match result.kind {
        ResultKind::App => app_icon_target(&result.subtitle),
        ResultKind::File => file_icon_target(result),
        ResultKind::Command
        | ResultKind::Calculator
        | ResultKind::AiAction
        | ResultKind::WebSearch
        | ResultKind::Tool => None,
    }
}

fn app_icon_target(path: &str) -> Option<IconTarget> {
    let extension = normalized_extension_from_path(path);
    match extension.as_deref() {
        Some("lnk") => Some(IconTarget::Shortcut {
            path: path.to_string(),
        }),
        Some("url") => Some(IconTarget::Url),
        _ if is_url_like(path) => Some(IconTarget::Url),
        _ if !path.trim().is_empty() && !is_probably_slow_path(path) => Some(IconTarget::Path {
            path: path.to_string(),
        }),
        _ => None,
    }
}

fn file_icon_target(result: &SearchResult) -> Option<IconTarget> {
    let path = file_result_path(result);
    if path.trim().is_empty() {
        return None;
    }

    if is_directory_result(result) {
        return Some(IconTarget::Folder);
    }

    let extension = result
        .file_metadata
        .as_ref()
        .and_then(|metadata| metadata.extension.as_deref().and_then(normalize_extension))
        .or_else(|| normalized_extension_from_path(&path));

    match extension.as_deref() {
        Some("lnk") if !is_probably_slow_path(&path) => Some(IconTarget::Shortcut { path }),
        Some("url") => Some(IconTarget::Url),
        _ => Some(IconTarget::FileType { extension }),
    }
}

fn custom_command_icon_target(target: &str) -> Option<IconTarget> {
    let target = target.trim();
    if target.is_empty() {
        return None;
    }
    if is_url_like(target) {
        return Some(IconTarget::Url);
    }

    let extension = normalized_extension_from_path(target);
    match extension.as_deref() {
        Some("exe") | Some("com") | Some("bat") | Some("cmd") if !is_probably_slow_path(target) => {
            Some(IconTarget::Path {
                path: target.to_string(),
            })
        }
        Some("lnk") if !is_probably_slow_path(target) => Some(IconTarget::Shortcut {
            path: target.to_string(),
        }),
        Some("url") => Some(IconTarget::Url),
        Some(_) => Some(IconTarget::FileType { extension }),
        None if looks_path_like(target)
            && !is_probably_slow_path(target)
            && Path::new(target).is_dir() =>
        {
            Some(IconTarget::Folder)
        }
        None if looks_path_like(target) => Some(IconTarget::FileType { extension: None }),
        None => None,
    }
}

fn is_directory_result(result: &SearchResult) -> bool {
    if let Some(metadata) = result.file_metadata.as_ref() {
        return metadata.is_dir;
    }

    if result.source.contains("目录") {
        return true;
    }

    let path = file_result_path(result);
    !path.trim().is_empty() && !is_probably_slow_path(&path) && Path::new(&path).is_dir()
}

fn file_result_path(result: &SearchResult) -> String {
    result
        .file_metadata
        .as_ref()
        .map(|metadata| metadata.full_path.clone())
        .filter(|path| !path.trim().is_empty())
        .unwrap_or_else(|| result.subtitle.clone())
}

fn cache_path_for_target(target: &IconTarget) -> Option<PathBuf> {
    let filename = icon_cache_file_name(target)?;
    cache_dir().ok().map(|directory| directory.join(filename))
}

fn cache_dir() -> Result<PathBuf, storage::StorageError> {
    storage::data_dir().map(|directory| directory.join(ICON_CACHE_DIR))
}

fn icon_cache_file_name(target: &IconTarget) -> Option<String> {
    match target {
        IconTarget::Folder => Some("folder.png".into()),
        IconTarget::Url => Some("url.png".into()),
        IconTarget::FileType { extension } => {
            let extension = extension
                .as_deref()
                .and_then(normalize_extension)
                .unwrap_or_else(|| "file".into());
            Some(format!("ext-{}.png", cache_key_part(&extension)))
        }
        IconTarget::Path { path } => {
            if is_probably_slow_path(path) {
                return None;
            }
            Some(format!(
                "path-{}.png",
                stable_hash(&path_cache_identity(path))
            ))
        }
        IconTarget::Shortcut { path } => {
            if is_probably_slow_path(path) {
                return None;
            }
            Some(format!(
                "shortcut-{}.png",
                stable_hash(&path_cache_identity(path))
            ))
        }
    }
}

fn path_cache_identity(path: &str) -> String {
    let normalized_path = normalize_path_text(path);
    let modified = fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| format!("{}-{}", duration.as_secs(), duration.subsec_nanos()))
        .unwrap_or_else(|| "missing".into());

    format!("{normalized_path}|{modified}")
}

fn normalize_path_text(path: &str) -> String {
    path.trim().replace('/', "\\").to_lowercase()
}

fn normalized_extension_from_path(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(normalize_extension)
}

fn normalize_extension(extension: &str) -> Option<String> {
    let normalized = extension.trim().trim_start_matches('.').to_lowercase();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn cache_key_part(value: &str) -> String {
    let normalized = value
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || *character == '-' || *character == '_'
        })
        .collect::<String>();
    if normalized.is_empty() {
        stable_hash(value)
    } else {
        normalized
    }
}

fn stable_hash(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn looks_path_like(value: &str) -> bool {
    value.contains(['\\', '/']) || value.chars().nth(1) == Some(':')
}

fn is_url_like(value: &str) -> bool {
    let value = value.trim().to_lowercase();
    value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("ftp://")
        || value.starts_with("file://")
}

fn is_probably_slow_path(path: &str) -> bool {
    let path = path.trim();
    let lower = path.to_lowercase();
    if lower.starts_with(r"\\?\unc\") {
        return true;
    }
    if lower.starts_with(r"\\?\") {
        return false;
    }
    path.starts_with(r"\\") || path.starts_with("//")
}

fn prune_cache_periodically() {
    let last_prune = LAST_CACHE_PRUNE.get_or_init(|| Mutex::new(None));
    let Ok(mut last_prune) = last_prune.lock() else {
        return;
    };

    if last_prune.is_some_and(|last_prune| last_prune.elapsed() < CACHE_PRUNE_INTERVAL) {
        return;
    }

    let _ = prune_icon_cache(CACHE_PRUNE_MAX_FILES, CACHE_PRUNE_MAX_AGE);
    *last_prune = Some(Instant::now());
}

fn prune_icon_cache(max_files: usize, max_age: Duration) -> Result<usize, String> {
    let directory = cache_dir().map_err(|error| error.to_string())?;
    if !directory.is_dir() {
        return Ok(0);
    }

    let now = std::time::SystemTime::now();
    let mut entries = Vec::new();
    let mut removed = 0usize;
    for entry in fs::read_dir(&directory).map_err(|error| error.to_string())? {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }

        let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
        let too_old = now
            .duration_since(modified)
            .map(|age| age > max_age)
            .unwrap_or(false);
        if too_old {
            if fs::remove_file(&path).is_ok() {
                removed += 1;
            }
            continue;
        }

        entries.push((path, modified));
    }

    if entries.len() > max_files {
        entries.sort_by_key(|(_, modified)| *modified);
        let overflow_count = entries.len() - max_files;
        for (path, _) in entries.into_iter().take(overflow_count) {
            if fs::remove_file(path).is_ok() {
                removed += 1;
            }
        }
    }

    Ok(removed)
}

#[cfg(windows)]
fn extract_target_icon(target: &IconTarget, output_path: &Path) -> bool {
    match target {
        IconTarget::Folder => extract_shell_icon("folder", true, true, output_path),
        IconTarget::FileType { extension } => {
            let path = extension
                .as_deref()
                .and_then(normalize_extension)
                .map(|extension| format!(".{extension}"))
                .unwrap_or_else(|| "file".into());
            extract_shell_icon(&path, false, true, output_path)
        }
        IconTarget::Path { path } => extract_path_icon(path, output_path),
        IconTarget::Shortcut { path } => extract_shortcut_icon(path, output_path),
        IconTarget::Url => {
            extract_shell_icon(".url", false, true, output_path)
                || extract_shell_icon("https://example.com", false, true, output_path)
        }
    }
}

#[cfg(windows)]
fn extract_shortcut_icon(path: &str, output_path: &Path) -> bool {
    if let Some(target_path) = resolve_shortcut_target(path) {
        if !is_probably_slow_path(&target_path) && extract_path_icon(&target_path, output_path) {
            return true;
        }
    }

    extract_path_icon(path, output_path) || extract_shell_icon(".lnk", false, true, output_path)
}

#[cfg(windows)]
fn extract_path_icon(path: &str, output_path: &Path) -> bool {
    if is_probably_slow_path(path) {
        return false;
    }

    extract_shell_icon(path, Path::new(path).is_dir(), false, output_path)
}

#[cfg(windows)]
fn extract_shell_icon(
    path: &str,
    is_dir: bool,
    use_file_attributes: bool,
    output_path: &Path,
) -> bool {
    use std::mem::size_of;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL, FILE_FLAGS_AND_ATTRIBUTES,
    };
    use windows::Win32::UI::Shell::{
        SHGetFileInfoW, SHFILEINFOW, SHGFI_FLAGS, SHGFI_ICON, SHGFI_LARGEICON,
        SHGFI_USEFILEATTRIBUTES,
    };
    use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;

    let wide_path = wide_string(path);
    let mut file_info = SHFILEINFOW::default();
    let attributes: FILE_FLAGS_AND_ATTRIBUTES = if is_dir {
        FILE_ATTRIBUTE_DIRECTORY
    } else {
        FILE_ATTRIBUTE_NORMAL
    };
    let mut flags = SHGFI_FLAGS(SHGFI_ICON.0 | SHGFI_LARGEICON.0);
    if use_file_attributes {
        flags = SHGFI_FLAGS(flags.0 | SHGFI_USEFILEATTRIBUTES.0);
    }

    let result = unsafe {
        SHGetFileInfoW(
            PCWSTR(wide_path.as_ptr()),
            attributes,
            Some(&mut file_info),
            size_of::<SHFILEINFOW>() as u32,
            flags,
        )
    };

    if result == 0 || file_info.hIcon.0.is_null() {
        return false;
    }

    let saved = save_hicon_png(file_info.hIcon, output_path);
    let _ = unsafe { DestroyIcon(file_info.hIcon) };
    saved
}

#[cfg(windows)]
fn save_hicon_png(
    hicon: windows::Win32::UI::WindowsAndMessaging::HICON,
    output_path: &Path,
) -> bool {
    use std::ptr::null_mut;
    use windows::core::{GUID, PCWSTR};
    use windows::Win32::Graphics::GdiPlus::{
        GdipCreateBitmapFromHICON, GdipDisposeImage, GdipSaveImageToFile, GdiplusShutdown,
        GdiplusStartup, GdiplusStartupInput, GpBitmap, GpImage, Ok as GdiPlusOk,
    };

    const PNG_ENCODER_CLSID: GUID = GUID::from_u128(0x557cf406_1a04_11d3_9a73_0000f81ef32e);

    let input = GdiplusStartupInput {
        GdiplusVersion: 1,
        DebugEventCallback: 0,
        SuppressBackgroundThread: false.into(),
        SuppressExternalCodecs: true.into(),
    };
    let mut token = 0usize;
    if unsafe { GdiplusStartup(&mut token, &input, null_mut()) } != GdiPlusOk {
        return false;
    }

    let mut bitmap: *mut GpBitmap = null_mut();
    let created = unsafe { GdipCreateBitmapFromHICON(hicon, &mut bitmap) } == GdiPlusOk;
    if !created || bitmap.is_null() {
        unsafe { GdiplusShutdown(token) };
        return false;
    }

    let image = bitmap.cast::<GpImage>();
    let wide_output = wide_path(output_path);
    let saved = unsafe {
        GdipSaveImageToFile(
            image,
            PCWSTR(wide_output.as_ptr()),
            &PNG_ENCODER_CLSID,
            null_mut(),
        )
    } == GdiPlusOk;
    let _ = unsafe { GdipDisposeImage(image) };
    unsafe { GdiplusShutdown(token) };

    if !saved {
        let _ = fs::remove_file(output_path);
        return false;
    }

    output_path
        .metadata()
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

#[cfg(windows)]
fn resolve_shortcut_target(path: &str) -> Option<String> {
    use std::ptr::null_mut;
    use windows::core::{Interface, GUID, PCWSTR};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, STGM_READ,
    };
    use windows::Win32::UI::Shell::IShellLinkW;

    const CLSID_SHELL_LINK: GUID = GUID::from_u128(0x00021401_0000_0000_c000_000000000046);

    let coinit = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let should_uninitialize = coinit.is_ok();
    if !should_uninitialize {
        return None;
    }

    let result = (|| {
        let shell_link: IShellLinkW =
            unsafe { CoCreateInstance(&CLSID_SHELL_LINK, None, CLSCTX_INPROC_SERVER).ok()? };
        let persist_file: IPersistFile = shell_link.cast().ok()?;
        let wide_shortcut = wide_string(path);
        unsafe {
            persist_file
                .Load(PCWSTR(wide_shortcut.as_ptr()), STGM_READ)
                .ok()?;
        }

        let mut target_buffer = [0u16; 1024];
        unsafe {
            shell_link.GetPath(&mut target_buffer, null_mut(), 0).ok()?;
        }

        wide_buffer_to_string(&target_buffer)
    })();

    unsafe { CoUninitialize() };
    result
}

#[cfg(windows)]
fn wide_string(value: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

#[cfg(windows)]
fn wide_path(value: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    value.as_os_str().encode_wide().chain(Some(0)).collect()
}

#[cfg(windows)]
fn wide_buffer_to_string(buffer: &[u16]) -> Option<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    let length = buffer
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(buffer.len());
    if length == 0 {
        None
    } else {
        Some(
            OsString::from_wide(&buffer[..length])
                .to_string_lossy()
                .to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_metadata::FileMetadata;

    #[test]
    fn extension_cache_keys_are_normalized() {
        let target = IconTarget::FileType {
            extension: Some("PDF".into()),
        };

        assert_eq!(
            icon_cache_file_name(&target).as_deref(),
            Some("ext-pdf.png")
        );
    }

    #[test]
    fn folder_cache_key_is_fixed() {
        assert_eq!(
            icon_cache_file_name(&IconTarget::Folder).as_deref(),
            Some("folder.png")
        );
    }

    #[test]
    fn path_cache_key_is_stable_for_same_path_and_mtime() {
        let path =
            std::env::temp_dir().join(format!("easy-launcher-icon-key-{}.txt", std::process::id()));
        fs::write(&path, b"icon").expect("write temp file");
        let target = IconTarget::Path {
            path: path.display().to_string(),
        };

        let first = icon_cache_file_name(&target);
        let second = icon_cache_file_name(&target);

        assert_eq!(first, second);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn missing_path_extension_uses_file_type_target_without_panicking() {
        let result = SearchResult {
            id: "file:missing".into(),
            title: "missing.TXT".into(),
            subtitle: r"Z:\missing\missing.TXT".into(),
            kind: ResultKind::File,
            action: crate::search::ActionKind::OpenFile,
            source: "测试文件".into(),
            score: 0.5,
            shortcut: None,
            file_metadata: None,
            icon_path: None,
        };

        assert_eq!(
            icon_target_for_result(&result),
            Some(IconTarget::FileType {
                extension: Some("txt".into())
            })
        );
    }

    #[test]
    fn directory_result_uses_folder_target() {
        let result = SearchResult {
            id: "file:folder".into(),
            title: "folder".into(),
            subtitle: r"C:\folder".into(),
            kind: ResultKind::File,
            action: crate::search::ActionKind::OpenFile,
            source: "Everything 目录".into(),
            score: 0.5,
            shortcut: None,
            file_metadata: Some(FileMetadata {
                is_dir: true,
                size_bytes: None,
                modified_unix_seconds: None,
                extension: None,
                full_path: r"C:\folder".into(),
            }),
            icon_path: None,
        };

        assert_eq!(icon_target_for_result(&result), Some(IconTarget::Folder));
    }

    #[test]
    fn existing_directory_without_metadata_uses_folder_target() {
        let directory = std::env::temp_dir().join(format!(
            "easy-launcher-icon-directory-{}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("create temp directory");

        let result = SearchResult {
            id: format!("file:{}", directory.display()),
            title: "huhu_html".into(),
            subtitle: directory.display().to_string(),
            kind: ResultKind::File,
            action: crate::search::ActionKind::OpenFile,
            source: "Everything 文件".into(),
            score: 0.5,
            shortcut: None,
            file_metadata: None,
            icon_path: None,
        };

        assert_eq!(icon_target_for_result(&result), Some(IconTarget::Folder));

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn custom_program_command_uses_path_target() {
        let result = SearchResult {
            id: "custom-command:notepad".into(),
            title: "Notepad".into(),
            subtitle: r"C:\Windows\System32\notepad.exe".into(),
            kind: ResultKind::Command,
            action: crate::search::ActionKind::RunCommand,
            source: "自定义命令 · program".into(),
            score: 0.5,
            shortcut: None,
            file_metadata: None,
            icon_path: None,
        };

        assert_eq!(
            icon_target_for_result(&result),
            Some(IconTarget::Path {
                path: r"C:\Windows\System32\notepad.exe".into()
            })
        );
    }

    #[cfg(windows)]
    #[test]
    fn resolves_file_type_icon_to_cached_png() {
        assert_resolves_icon_to_png(IconTarget::FileType {
            extension: Some("txt".into()),
        });
    }

    #[cfg(windows)]
    #[test]
    fn resolves_folder_icon_to_cached_png() {
        assert_resolves_icon_to_png(IconTarget::Folder);
    }

    #[cfg(windows)]
    #[test]
    fn resolves_executable_path_icon_to_cached_png() {
        let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        let notepad = Path::new(&system_root).join("System32\\notepad.exe");
        assert!(
            notepad.is_file(),
            "expected Windows notepad executable at {}",
            notepad.display()
        );

        assert_resolves_icon_to_png(IconTarget::Path {
            path: notepad.display().to_string(),
        });
    }

    #[cfg(windows)]
    fn assert_resolves_icon_to_png(target: IconTarget) {
        let request = IconRequest {
            result_id: "test".into(),
            target,
        };
        let icon_path = resolve_icon_request(&request).expect("resolve icon");
        let icon_path = PathBuf::from(icon_path);
        let metadata = fs::metadata(&icon_path).expect("icon metadata");

        assert_eq!(
            icon_path.extension().and_then(|value| value.to_str()),
            Some("png")
        );
        assert!(metadata.is_file());
        assert!(metadata.len() > 0);
    }
}
