use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const GITHUB_LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/YoungHQ/easy_launcher/releases/latest";
const GITHUB_RELEASES_URL: &str = "https://api.github.com/repos/YoungHQ/easy_launcher/releases";
const GITHUB_RELEASE_PAGE_PREFIX: &str = "https://github.com/YoungHQ/easy_launcher/releases/";
const USER_AGENT: &str = "EasyLauncher";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    current_version: String,
    latest_version: Option<String>,
    latest_tag: Option<String>,
    release_name: Option<String>,
    release_url: Option<String>,
    published_at: Option<String>,
    is_newer: Option<bool>,
    is_prerelease: bool,
    asset_name: Option<String>,
    asset_download_url: Option<String>,
    error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    name: Option<String>,
    html_url: String,
    published_at: Option<String>,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

#[derive(Clone, Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Semver3 {
    major: u64,
    minor: u64,
    patch: u64,
}

enum UpdateFetchError {
    NoRelease,
    Message(String),
}

impl UpdateFetchError {
    fn message(self) -> String {
        match self {
            UpdateFetchError::NoRelease => "暂未发现公开发布版本。".into(),
            UpdateFetchError::Message(message) => message,
        }
    }
}

impl UpdateCheckResult {
    fn failed(current_version: &str, error: impl Into<String>) -> Self {
        Self {
            current_version: current_version.into(),
            latest_version: None,
            latest_tag: None,
            release_name: None,
            release_url: None,
            published_at: None,
            is_newer: None,
            is_prerelease: false,
            asset_name: None,
            asset_download_url: None,
            error: Some(error.into()),
        }
    }
}

pub async fn check_for_updates(
    current_version: &str,
    include_prerelease: bool,
) -> UpdateCheckResult {
    match fetch_latest_release(include_prerelease).await {
        Ok(Some(release)) => build_update_result(current_version, release),
        Ok(None) => UpdateCheckResult::failed(current_version, "暂未发现公开发布版本。"),
        Err(error) => UpdateCheckResult::failed(current_version, error.message()),
    }
}

pub fn is_allowed_release_url(url: &str) -> bool {
    url.trim().starts_with(GITHUB_RELEASE_PAGE_PREFIX)
}

async fn fetch_latest_release(
    include_prerelease: bool,
) -> Result<Option<GithubRelease>, UpdateFetchError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| UpdateFetchError::Message(format!("创建更新检查客户端失败：{error}")))?;

    if include_prerelease {
        let releases = fetch_release_list(&client).await?;
        return Ok(select_release(releases, true));
    }

    match fetch_release(&client, GITHUB_LATEST_RELEASE_URL).await {
        Ok(release) if !release.draft && !release.prerelease => Ok(Some(release)),
        Ok(_) => Ok(None),
        Err(UpdateFetchError::NoRelease) => {
            let releases = fetch_release_list(&client).await?;
            Ok(select_release(releases, false))
        }
        Err(error) => Err(error),
    }
}

async fn fetch_release(
    client: &reqwest::Client,
    url: &str,
) -> Result<GithubRelease, UpdateFetchError> {
    let response = send_github_get(client, url).await?;
    response
        .json::<GithubRelease>()
        .await
        .map_err(|_| UpdateFetchError::Message("读取 GitHub 发布信息失败。".into()))
}

async fn fetch_release_list(
    client: &reqwest::Client,
) -> Result<Vec<GithubRelease>, UpdateFetchError> {
    let response = send_github_get(client, GITHUB_RELEASES_URL).await?;
    response
        .json::<Vec<GithubRelease>>()
        .await
        .map_err(|_| UpdateFetchError::Message("读取 GitHub 发布信息失败。".into()))
}

async fn send_github_get(
    client: &reqwest::Client,
    url: &str,
) -> Result<reqwest::Response, UpdateFetchError> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|_| UpdateFetchError::Message("无法连接 GitHub，请稍后重试。".into()))?;
    let status = response.status();

    if status.is_success() {
        return Ok(response);
    }

    if status == StatusCode::NOT_FOUND {
        return Err(UpdateFetchError::NoRelease);
    }

    if status == StatusCode::FORBIDDEN || status == StatusCode::TOO_MANY_REQUESTS {
        return Err(UpdateFetchError::Message(
            "GitHub API 暂时限制访问，请稍后重试。".into(),
        ));
    }

    Err(UpdateFetchError::Message(format!(
        "GitHub 发布信息读取失败（{status}）。"
    )))
}

fn select_release(releases: Vec<GithubRelease>, include_prerelease: bool) -> Option<GithubRelease> {
    releases
        .into_iter()
        .find(|release| !release.draft && (include_prerelease || !release.prerelease))
}

fn build_update_result(current_version: &str, release: GithubRelease) -> UpdateCheckResult {
    let current = parse_semver3(current_version);
    let latest = parse_semver3(&release.tag_name);
    let is_newer = current
        .zip(latest)
        .map(|(current, latest)| latest > current);
    let error = if is_newer.is_none() {
        Some("发现发布信息，但无法识别版本号。".into())
    } else {
        None
    };
    let latest_version = latest.map(|version| version.to_string());
    let asset = select_msi_asset(&release.assets);

    UpdateCheckResult {
        current_version: current_version.into(),
        latest_version,
        latest_tag: Some(release.tag_name),
        release_name: release.name,
        release_url: Some(release.html_url),
        published_at: release.published_at,
        is_newer,
        is_prerelease: release.prerelease,
        asset_name: asset.map(|asset| asset.name.clone()),
        asset_download_url: asset.map(|asset| asset.browser_download_url.clone()),
        error,
    }
}

fn select_msi_asset(assets: &[GithubAsset]) -> Option<&GithubAsset> {
    assets.iter().find(|asset| {
        asset.name.to_lowercase().ends_with(".msi")
            && asset.browser_download_url.starts_with("https://")
    })
}

fn parse_semver3(version: &str) -> Option<Semver3> {
    let version = version.trim();
    let version = version
        .strip_prefix('v')
        .or_else(|| version.strip_prefix('V'))
        .unwrap_or(version);
    let parts = version.split('.').collect::<Vec<_>>();
    if parts.len() != 3 || parts.iter().any(|part| part.is_empty()) {
        return None;
    }

    let mut parsed = Vec::with_capacity(3);
    for part in parts {
        if !part.chars().all(|character| character.is_ascii_digit()) {
            return None;
        }
        parsed.push(part.parse::<u64>().ok()?);
    }

    Some(Semver3 {
        major: parsed[0],
        minor: parsed[1],
        patch: parsed[2],
    })
}

impl std::fmt::Display for Semver3 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_parser_accepts_plain_and_v_prefixed_versions() {
        assert_eq!(
            parse_semver3("0.1.0"),
            Some(Semver3 {
                major: 0,
                minor: 1,
                patch: 0
            })
        );
        assert_eq!(
            parse_semver3("v1.20.300"),
            Some(Semver3 {
                major: 1,
                minor: 20,
                patch: 300
            })
        );
    }

    #[test]
    fn semver_parser_rejects_suffixes_and_missing_parts() {
        assert_eq!(parse_semver3("v0.1"), None);
        assert_eq!(parse_semver3("v0.1.0-beta"), None);
        assert_eq!(parse_semver3("v0.1.0.1"), None);
    }

    #[test]
    fn update_result_marks_newer_release() {
        let result = build_update_result(
            "0.1.0",
            GithubRelease {
                tag_name: "v0.2.0".into(),
                name: Some("Easy Launcher v0.2.0".into()),
                html_url: "https://github.com/YoungHQ/easy_launcher/releases/tag/v0.2.0".into(),
                published_at: Some("2026-06-06T00:00:00Z".into()),
                prerelease: false,
                draft: false,
                assets: vec![GithubAsset {
                    name: "EasyLauncher_0.2.0_x64_en-US.msi".into(),
                    browser_download_url:
                        "https://github.com/YoungHQ/easy_launcher/releases/download/v0.2.0/app.msi"
                            .into(),
                }],
            },
        );

        assert_eq!(result.latest_version.as_deref(), Some("0.2.0"));
        assert_eq!(result.is_newer, Some(true));
        assert_eq!(
            result.asset_name.as_deref(),
            Some("EasyLauncher_0.2.0_x64_en-US.msi")
        );
        assert!(result.error.is_none());
    }

    #[test]
    fn update_result_keeps_release_url_when_version_is_invalid() {
        let result = build_update_result(
            "0.1.0",
            GithubRelease {
                tag_name: "latest".into(),
                name: None,
                html_url: "https://github.com/YoungHQ/easy_launcher/releases/tag/latest".into(),
                published_at: None,
                prerelease: false,
                draft: false,
                assets: Vec::new(),
            },
        );

        assert_eq!(result.is_newer, None);
        assert_eq!(result.latest_tag.as_deref(), Some("latest"));
        assert_eq!(
            result.error.as_deref(),
            Some("发现发布信息，但无法识别版本号。")
        );
    }

    #[test]
    fn release_url_allowlist_accepts_project_release_pages_only() {
        assert!(is_allowed_release_url(
            "https://github.com/YoungHQ/easy_launcher/releases/tag/v0.1.0"
        ));
        assert!(!is_allowed_release_url(
            "https://example.com/releases/tag/v0.1.0"
        ));
    }
}
