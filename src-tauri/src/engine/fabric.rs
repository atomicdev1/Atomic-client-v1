use anyhow::Result;
use serde::Deserialize;
use std::path::PathBuf;

use crate::engine::download::{ensure_files, DlItem, Progress};
use crate::engine::paths::{path_from_maven, Dirs};

#[derive(Debug, Clone, Default)]
pub struct LoaderProfile {
    pub main_class: String,
    pub libraries: Vec<LoaderLib>,
    pub loader_version: String,
    pub loader_name: String,
}

#[derive(Debug, Clone)]
pub struct LoaderLib {
    pub name: String,
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

#[derive(Deserialize)]
struct RawProfile {
    #[serde(rename = "mainClass")]
    main_class: String,
    #[serde(default)]
    libraries: Vec<RawLib>,
}

#[derive(Deserialize)]
struct RawLib {
    name: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    sha1: String,
    #[serde(default)]
    size: u64,
}

#[derive(Deserialize)]
struct RawLoader {
    version: String,
    stable: bool,
}

const FABRIC_META: &str = "https://meta.fabricmc.net";
const QUILT_META: &str = "https://meta.quiltmc.org";

/// Resolves the newest loader for the game version and returns its profile.
pub async fn fetch_profile(
    client: &reqwest::Client,
    mc_version: &str,
    loader: &str,
) -> Result<LoaderProfile> {
    let base = if loader.eq_ignore_ascii_case("quilt") {
        QUILT_META
    } else {
        FABRIC_META
    };

    let list_url = format!("{base}/v2/versions/loader/{mc_version}");
    let list: Vec<serde_json::Value> = crate::engine::download::fetch_json(client, &list_url).await?;
    let mut chosen = String::new();
    for entry in &list {
        if let Some(v) = entry.get("loader").and_then(|l| l.get("version")).and_then(|v| v.as_str())
        {
            let stable = entry
                .get("loader")
                .and_then(|l| l.get("stable"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if stable {
                chosen = v.to_string();
                break;
            }
        }
    }
    if chosen.is_empty() {
        let loaders: Vec<RawLoader> = crate::engine::download::fetch_json(
            client,
            &format!("{base}/v2/versions/loader"),
        )
        .await?;
        chosen = loaders
            .iter()
            .find(|l| l.stable)
            .or(loaders.first())
            .map(|l| l.version.clone())
            .unwrap_or_default();
    }
    if chosen.is_empty() {
        return Err(anyhow::anyhow!("no {loader} loader found for {mc_version}"));
    }

    let profile_url = format!("{base}/v2/versions/loader/{mc_version}/{chosen}/profile/json");
    let raw: RawProfile = crate::engine::download::fetch_json(client, &profile_url).await?;

    Ok(LoaderProfile {
        main_class: raw.main_class,
        libraries: raw
            .libraries
            .into_iter()
            .map(|l| LoaderLib {
                name: l.name,
                url: l.url,
                sha1: if l.sha1.is_empty() { None } else { Some(l.sha1) },
                size: if l.size == 0 { None } else { Some(l.size) },
            })
            .collect(),
        loader_version: chosen,
        loader_name: loader.to_string(),
    })
}

/// Downloads loader libraries into the shared library folder and returns their
/// local paths in classpath order.
pub async fn install_libs(
    client: &reqwest::Client,
    dirs: &Dirs,
    profile: &LoaderProfile,
    progress: Option<Progress>,
) -> Result<Vec<PathBuf>> {
    let mut items = Vec::new();
    let mut paths = Vec::new();
    for lib in &profile.libraries {
        let Some(rel) = path_from_maven(&lib.name) else {
            continue;
        };
        let path = dirs.libraries.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
        let mut url = if lib.url.is_empty() {
            "https://maven.fabricmc.net/".to_string()
        } else if lib.url.starts_with("http") {
            lib.url.clone()
        } else {
            format!("https://{}", lib.url)
        };
        if !url.ends_with('/') {
            url.push('/');
        }
        if let Some(rel_url) = path_from_maven(&lib.name) {
            items.push(DlItem {
                url: format!("{url}{rel_url}"),
                path: path.clone(),
                sha1: lib.sha1.clone(),
                size: lib.size,
            });
        }
        paths.push(path);
    }
    ensure_files(client, items, 8, progress).await?;
    Ok(paths)
}
