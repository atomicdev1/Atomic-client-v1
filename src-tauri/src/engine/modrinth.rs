use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::engine::download::{ensure_files, DlItem, Progress};
use crate::engine::paths::Dirs;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SearchHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub author: String,
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    hits: Vec<RawHit>,
}

#[derive(Deserialize)]
struct RawHit {
    project_id: String,
    slug: String,
    title: String,
    description: String,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    author: String,
}

#[derive(Deserialize)]
struct VersionFile {
    #[serde(default)]
    url: String,
    filename: String,
    #[serde(default)]
    hashes: serde_json::Value,
    #[serde(default)]
    size: Option<u64>,
}

#[derive(Deserialize)]
struct ModVersion {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version_number: String,
    #[serde(default)]
    files: Vec<VersionFile>,
}

const MODRINTH: &str = "https://api.modrinth.com/v2";

pub async fn search(client: &reqwest::Client, query: &str, limit: u32) -> Result<Vec<SearchHit>> {
    let url = format!("{MODRINTH}/search");
    let res: SearchResponse = client
        .get(url)
        .query(&[
            ("query", query.to_string()),
            ("limit", limit.to_string()),
            (
                "facets",
                "[[\"project_type:mod\"]]".to_string(),
            ),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(res
        .hits
        .into_iter()
        .map(|h| SearchHit {
            project_id: h.project_id,
            slug: h.slug,
            title: h.title,
            description: h.description,
            downloads: h.downloads,
            icon_url: h.icon_url,
            author: h.author,
        })
        .collect())
}

/// Installs the newest compatible mod file into the instance's mods folder.
pub async fn install(
    client: &reqwest::Client,
    dirs: &Dirs,
    slug: &str,
    mc_version: &str,
    loader: &str,
    instance: &str,
    progress: Option<Progress>,
) -> Result<PathBuf> {
    let loader_name = if loader.eq_ignore_ascii_case("quilt") {
        "quilt"
    } else {
        "fabric"
    };
    let url = format!("{MODRINTH}/project/{slug}/version");
    let res = client
        .get(url)
        .query(&[
            (
                "loaders",
                format!("[\"{loader_name}\"]"),
            ),
            ("game_versions", format!("[\"{mc_version}\"]")),
        ])
        .send()
        .await?;
    let status = res.status();
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(anyhow!("Modrinth version lookup failed ({status}): {body}"));
    }
    let versions: Vec<ModVersion> = res.json().await?;
    let version = versions
        .first()
        .ok_or_else(|| anyhow!("no {loader_name} build of {slug} for {mc_version}"))?;
    let file = version
        .files
        .first()
        .ok_or_else(|| anyhow!("version {} has no files", version.version_number))?;
    let sha1 = file
        .hashes
        .get("sha1")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let mods = dirs.instance(instance).join("mods");
    std::fs::create_dir_all(&mods)?;
    let dest = mods.join(&file.filename);
    ensure_files(
        client,
        vec![DlItem {
            url: file.url.clone(),
            path: dest.clone(),
            sha1,
            size: file.size,
        }],
        1,
        progress,
    )
    .await?;
    Ok(dest)
}
