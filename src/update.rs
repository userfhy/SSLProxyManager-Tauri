use crate::config;
use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub latest_version: String,
    pub download_url: String,
    pub release_notes: String,
    pub is_mandatory: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub has_update: bool,
    pub is_prerelease: bool,
    pub current_version: String,
    pub update_info: Option<UpdateInfo>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubRelease {
    tag_name: String,
    prerelease: bool,
    body: Option<String>,
    html_url: Option<String>,
}

fn normalize_github_tag(tag: &str) -> Result<Version> {
    // 约定 tag 为 v1.0.6，但也兼容 1.0.6
    let v = tag.trim().trim_start_matches('v');
    Version::parse(v).context("无效的 GitHub tag 版本号")
}

fn pick_download_url(rel: &GithubRelease) -> String {
    // 统一返回 GitHub Release 页面（tag 对应的 release 页）
    // 例如：https://github.com/userfhy/SSLProxyManager-Tauri/releases/tag/v1.0.6
    rel.html_url.clone().unwrap_or_default()
}

pub async fn check_for_updates(current_version: &str, cfg: config::UpdateConfig) -> Result<CheckResult> {
    // 兼容旧逻辑：仍然尊重 enabled 开关；server_url 配置将被前端隐藏，但仍允许保留在配置里
    if !cfg.enabled {
        return Ok(CheckResult {
            has_update: false,
            is_prerelease: false,
            current_version: current_version.to_string(),
            update_info: None,
            error: Some("更新检查未启用".to_string()),
        });
    }

    let timeout = if cfg.timeout_ms > 0 {
        Duration::from_millis(cfg.timeout_ms as u64)
    } else {
        Duration::from_secs(10)
    };

    let client = Client::builder()
        .timeout(timeout)
        .build()
        .context("创建 HTTP 客户端失败")?;

    // 固定 GitHub 仓库
    let owner = "userfhy";
    let repo = "SSLProxyManager-Tauri";

    // 1) 拉取 release 列表（可筛选 prerelease）
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases?per_page=20",
        owner, repo
    );

    let resp = client
        .get(url)
        .header("User-Agent", "SSLProxyManager-Update-Checker/1.0")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .context("请求 GitHub releases 失败")?;

    if !resp.status().is_success() {
        return Err(anyhow!("GitHub 返回错误状态: {}", resp.status()));
    }

    let releases: Vec<GithubRelease> = resp.json().await.context("解析 GitHub releases 失败")?;

    let candidate = if cfg.ignore_prerelease {
        releases.into_iter().find(|r| !r.prerelease)
    } else {
        releases.into_iter().next()
    };

    let Some(rel) = candidate else {
        return Ok(CheckResult {
            has_update: false,
            is_prerelease: false,
            current_version: current_version.to_string(),
            update_info: None,
            error: Some("未找到可用的 GitHub Release".to_string()),
        });
    };

    let latest_version_semver = normalize_github_tag(&rel.tag_name)?;
    let is_prerelease = rel.prerelease;

    // ignore_prerelease=true 时，我们已经筛掉了 prerelease，因此此分支通常不会命中
    if cfg.ignore_prerelease && is_prerelease {
        return Ok(CheckResult {
            has_update: false,
            is_prerelease: true,
            current_version: current_version.to_string(),
            update_info: Some(UpdateInfo {
                latest_version: rel.tag_name.clone(),
                download_url: pick_download_url(&rel),
                release_notes: rel.body.unwrap_or_default(),
                is_mandatory: false,
            }),
            error: None,
        });
    }

    let has_update = if current_version == "dev" {
        true
    } else {
        let current = Version::parse(current_version).context("无效的当前版本号")?;
        latest_version_semver > current
    };

    Ok(CheckResult {
        has_update,
        is_prerelease,
        current_version: current_version.to_string(),
        update_info: Some(UpdateInfo {
            latest_version: rel.tag_name.clone(),
            download_url: pick_download_url(&rel),
            release_notes: rel.body.unwrap_or_default(),
            is_mandatory: false,
        }),
        error: None,
    })
}
