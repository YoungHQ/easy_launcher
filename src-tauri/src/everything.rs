use crate::process::hidden_command;
use serde::{Deserialize, Serialize};
use std::env;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const EVERYTHING_HTTP_ADDR: &str = "127.0.0.1:8080";
const EVERYTHING_RUNNING_CACHE_TTL: Duration = Duration::from_secs(3);
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;

static EVERYTHING_RUNNING_CACHE: OnceLock<Mutex<EverythingRunningCache>> = OnceLock::new();

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EverythingStatus {
    pub installed: bool,
    pub running: bool,
    pub ipc_available: bool,
    pub http_available: bool,
    pub install_path: Option<String>,
    pub message: String,
}

#[derive(Clone)]
pub struct EverythingFileResult {
    pub name: String,
    pub path: String,
    pub is_folder: bool,
}

#[derive(Deserialize)]
struct EverythingHttpResponse {
    results: Vec<EverythingHttpItem>,
}

#[derive(Deserialize)]
struct EverythingHttpItem {
    name: String,
    path: String,
    #[serde(default)]
    r#type: Option<String>,
}

#[derive(Default)]
struct EverythingRunningCache {
    entry: Option<EverythingRunningCacheEntry>,
}

struct EverythingRunningCacheEntry {
    running: bool,
    checked_at: Instant,
}

impl EverythingRunningCache {
    fn get_or_refresh(&mut self, probe: impl FnOnce() -> bool) -> bool {
        self.get_or_refresh_at(Instant::now(), EVERYTHING_RUNNING_CACHE_TTL, probe)
    }

    fn get_or_refresh_at(
        &mut self,
        now: Instant,
        ttl: Duration,
        probe: impl FnOnce() -> bool,
    ) -> bool {
        if let Some(entry) = &self.entry {
            let age = now
                .checked_duration_since(entry.checked_at)
                .unwrap_or(Duration::ZERO);
            if age <= ttl {
                return entry.running;
            }
        }

        let running = probe();
        self.entry = Some(EverythingRunningCacheEntry {
            running,
            checked_at: now,
        });
        running
    }

    fn invalidate(&mut self) {
        self.entry = None;
    }
}

pub fn detect_everything_status(configured_path: Option<&str>) -> EverythingStatus {
    let install_path = find_everything_install_path(configured_path);
    let installed = install_path.is_some();
    let running = is_everything_running();
    let http_available = is_http_available();
    let ipc_available = running;

    let message = if !installed {
        "未检测到 Everything，请安装后使用文件搜索".into()
    } else if !running {
        "Everything 已安装但未运行".into()
    } else if http_available {
        "Everything 正在运行，HTTP 备用接口可用".into()
    } else {
        "Everything 正在运行，HTTP 备用接口未开启".into()
    };

    EverythingStatus {
        installed,
        running,
        ipc_available,
        http_available,
        install_path: install_path.map(|path| path.display().to_string()),
        message,
    }
}

pub fn search_everything_http(query: &str, limit: usize) -> Vec<EverythingFileResult> {
    let query = query.trim();
    if query.is_empty() || !is_http_available() {
        return Vec::new();
    }

    let encoded_query = urlencoding::encode(query);
    let request_path = format!("/?search={encoded_query}&json=1&count={limit}");
    let request =
        format!("GET {request_path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");

    let Ok(mut stream) = TcpStream::connect_timeout(
        &EVERYTHING_HTTP_ADDR
            .parse::<SocketAddr>()
            .expect("valid everything http address"),
        Duration::from_millis(350),
    ) else {
        return Vec::new();
    };

    let _ = stream.set_read_timeout(Some(Duration::from_millis(900)));
    if stream.write_all(request.as_bytes()).is_err() {
        return Vec::new();
    }

    let mut response = String::new();
    if stream.read_to_string(&mut response).is_err() {
        return Vec::new();
    }

    parse_http_body(&response)
        .and_then(parse_everything_results)
        .unwrap_or_default()
}

pub fn try_search_everything_ipc(
    query: &str,
    limit: usize,
    match_path: bool,
) -> Result<Vec<EverythingFileResult>, String> {
    let query = query.trim();
    if query.is_empty() || !is_everything_running() {
        return Ok(Vec::new());
    }

    match search_everything_ipc_inner(query, limit, match_path) {
        Ok(results) => Ok(results),
        Err(_) => {
            invalidate_everything_running_cache();
            Err("Everything IPC 查询失败".into())
        }
    }
}

#[cfg(windows)]
fn search_everything_ipc_inner(
    query: &str,
    limit: usize,
    match_path: bool,
) -> Result<Vec<EverythingFileResult>, String> {
    use everything_ipc::wm::{EverythingClient, RequestFlags, SearchFlags};

    let client = EverythingClient::new().map_err(|error| error.to_string())?;
    let search_flags = if match_path {
        SearchFlags::MatchPath
    } else {
        SearchFlags::empty()
    };
    let list = client
        .query_wait(query)
        .search_flags(search_flags)
        .request_flags(RequestFlags::FileName | RequestFlags::Path | RequestFlags::Attributes)
        .max_results(limit as u32)
        .call()
        .map_err(|error| error.to_string())?;

    let mut results = Vec::new();
    for item in list.iter() {
        let Some(name) = item.get_string(RequestFlags::FileName) else {
            continue;
        };
        let Some(path) = item.get_string(RequestFlags::Path) else {
            continue;
        };

        let full_path = if path.is_empty() {
            name.clone()
        } else {
            format!("{path}\\{name}")
        };
        let is_folder = item
            .get_u32(RequestFlags::Attributes)
            .is_some_and(|attributes| attributes & FILE_ATTRIBUTE_DIRECTORY != 0)
            || std::path::Path::new(&full_path).is_dir();

        results.push(EverythingFileResult {
            path: full_path,
            name,
            is_folder,
        });
    }

    Ok(results)
}

#[cfg(not(windows))]
fn search_everything_ipc_inner(
    _query: &str,
    _limit: usize,
    _match_path: bool,
) -> Result<Vec<EverythingFileResult>, String> {
    Ok(Vec::new())
}

fn find_everything_install_path(configured_path: Option<&str>) -> Option<PathBuf> {
    candidate_paths(configured_path)
        .into_iter()
        .find(|path| path.exists())
}

fn candidate_paths(configured_path: Option<&str>) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    push_optional_path(&mut paths, configured_path);
    push_optional_path(
        &mut paths,
        env::var("EASY_LAUNCHER_EVERYTHING_EXE").ok().as_deref(),
    );
    push_optional_path(&mut paths, env::var("VITE_EVERYTHING_EXE").ok().as_deref());

    if let Ok(program_files) = env::var("ProgramFiles") {
        paths.push(PathBuf::from(program_files).join("Everything\\Everything.exe"));
    }

    if let Ok(program_files_x86) = env::var("ProgramFiles(x86)") {
        paths.push(PathBuf::from(program_files_x86).join("Everything\\Everything.exe"));
    }

    if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
        paths.push(PathBuf::from(&local_app_data).join("Everything\\Everything.exe"));
        paths.push(PathBuf::from(local_app_data).join("Programs\\Everything\\Everything.exe"));
    }

    paths
}

fn push_optional_path(paths: &mut Vec<PathBuf>, path: Option<&str>) {
    let Some(path) = path.map(str::trim).filter(|path| !path.is_empty()) else {
        return;
    };
    paths.push(PathBuf::from(path));
}

fn is_everything_running() -> bool {
    let cache =
        EVERYTHING_RUNNING_CACHE.get_or_init(|| Mutex::new(EverythingRunningCache::default()));
    let Ok(mut cache) = cache.lock() else {
        return probe_everything_running();
    };

    cache.get_or_refresh(probe_everything_running)
}

fn invalidate_everything_running_cache() {
    if let Some(cache) = EVERYTHING_RUNNING_CACHE.get() {
        if let Ok(mut cache) = cache.lock() {
            cache.invalidate();
        }
    }
}

fn probe_everything_running() -> bool {
    let output = hidden_command("tasklist")
        .args(["/FI", "IMAGENAME eq Everything.exe", "/NH"])
        .output();

    output
        .map(|output| String::from_utf8_lossy(&output.stdout).contains("Everything.exe"))
        .unwrap_or(false)
}

fn is_http_available() -> bool {
    let Ok(address) = EVERYTHING_HTTP_ADDR.parse::<SocketAddr>() else {
        return false;
    };

    TcpStream::connect_timeout(&address, Duration::from_millis(180)).is_ok()
}

fn parse_http_body(response: &str) -> Option<&str> {
    response.split_once("\r\n\r\n").map(|(_, body)| body)
}

fn parse_everything_results(body: &str) -> Option<Vec<EverythingFileResult>> {
    let response: EverythingHttpResponse = serde_json::from_str(body).ok()?;

    Some(
        response
            .results
            .into_iter()
            .map(|item| EverythingFileResult {
                path: if item.path.is_empty() {
                    item.name.clone()
                } else {
                    format!("{}\\{}", item.path, item.name)
                },
                name: item.name,
                is_folder: item
                    .r#type
                    .as_deref()
                    .is_some_and(|item_type| item_type.eq_ignore_ascii_case("folder")),
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_paths_include_known_everything_locations() {
        let paths = candidate_paths(None);
        let joined = paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(joined.contains("Everything.exe"));
    }

    #[test]
    fn candidate_paths_prefer_configured_everything_path() {
        let paths = candidate_paths(Some(r"C:\Tools\Everything\Everything.exe"));

        assert_eq!(
            paths.first().map(|path| path.display().to_string()),
            Some(r"C:\Tools\Everything\Everything.exe".into())
        );
    }

    #[test]
    fn parses_everything_http_results() {
        let body = r#"{"results":[{"name":"demo.txt","path":"C:\\Temp","type":"file"}]}"#;
        let results = parse_everything_results(body).expect("parse everything response");

        assert_eq!(results[0].name, "demo.txt");
        assert_eq!(results[0].path, "C:\\Temp\\demo.txt");
        assert!(!results[0].is_folder);
    }

    #[test]
    fn running_cache_reuses_value_within_ttl() {
        let mut cache = EverythingRunningCache::default();
        let now = Instant::now();
        let mut probes = 0;

        assert!(cache.get_or_refresh_at(now, Duration::from_secs(3), || {
            probes += 1;
            true
        }));
        assert!(cache.get_or_refresh_at(
            now + Duration::from_secs(1),
            Duration::from_secs(3),
            || {
                probes += 1;
                false
            }
        ));

        assert_eq!(probes, 1);
    }

    #[test]
    fn running_cache_refreshes_after_ttl() {
        let mut cache = EverythingRunningCache::default();
        let now = Instant::now();
        let mut probes = 0;

        assert!(cache.get_or_refresh_at(now, Duration::from_secs(3), || {
            probes += 1;
            true
        }));
        assert!(!cache.get_or_refresh_at(
            now + Duration::from_secs(4),
            Duration::from_secs(3),
            || {
                probes += 1;
                false
            },
        ));

        assert_eq!(probes, 2);
    }

    #[test]
    fn running_cache_can_be_invalidated() {
        let mut cache = EverythingRunningCache::default();
        let now = Instant::now();

        assert!(cache.get_or_refresh_at(now, Duration::from_secs(3), || true));
        cache.invalidate();

        assert!(!cache.get_or_refresh_at(
            now + Duration::from_secs(1),
            Duration::from_secs(3),
            || false
        ));
    }
}
