use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;

use crate::engine::rules::Rule;

pub const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
pub const JAVA_RUNTIME_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Latest {
    #[serde(default)]
    pub release: String,
    #[serde(default)]
    pub snapshot: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct VersionSummary {
    pub id: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    pub url: String,
    #[serde(rename = "releaseTime", default)]
    pub release_time: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct VersionManifest {
    #[serde(default)]
    pub latest: Latest,
    #[serde(default)]
    pub versions: Vec<VersionSummary>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Artifact {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

impl Artifact {
    pub fn url_norm(&self) -> Option<String> {
        self.url.as_ref().map(|u| {
            if u.starts_with("http") {
                u.clone()
            } else {
                format!("https://{u}")
            }
        })
    }
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct LibDownloads {
    #[serde(default)]
    pub artifact: Option<Artifact>,
    #[serde(default)]
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Extract {
    #[serde(default)]
    pub exclude: Option<Vec<String>>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Library {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibDownloads>,
    #[serde(default)]
    pub natives: Option<HashMap<String, String>>,
    #[serde(default)]
    pub extract: Option<Extract>,
    #[serde(default)]
    pub rules: Option<Vec<Rule>>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct RuleArg {
    pub rules: Vec<Rule>,
    pub value: ArgValue,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum ArgValue {
    One(String),
    Many(Vec<String>),
}

impl ArgValue {
    pub fn push_to(&self, out: &mut Vec<String>) {
        match self {
            ArgValue::One(s) => out.push(s.clone()),
            ArgValue::Many(v) => out.extend(v.iter().cloned()),
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum ArgEntry {
    Plain(String),
    Conditional(RuleArg),
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<ArgEntry>,
    #[serde(default)]
    pub jvm: Vec<ArgEntry>,
    #[serde(rename = "default-user-jvm", default)]
    pub default_user_jvm: Vec<ArgEntry>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct VersionDownloads {
    #[serde(default)]
    pub client: Option<Artifact>,
    #[serde(default)]
    pub server: Option<Artifact>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct JavaVersion {
    #[serde(default)]
    pub component: String,
    #[serde(rename = "majorVersion", default)]
    pub major_version: i32,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct LogFile {
    pub id: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub url: Option<String>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct LogClient {
    pub argument: String,
    pub file: LogFile,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Logging {
    pub client: LogClient,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct AssetIndexRef {
    pub id: String,
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    #[serde(rename = "totalSize", default)]
    pub total_size: Option<u64>,
    #[serde(rename = "virtual", default)]
    pub is_virtual: bool,
    #[serde(rename = "map_to_resources", default)]
    pub map_to_resources: bool,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct VersionDetails {
    pub id: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(rename = "mainClass", default)]
    pub main_class: String,
    #[serde(rename = "minecraftArguments", default)]
    pub minecraft_arguments: Option<String>,
    #[serde(default)]
    pub arguments: Option<Arguments>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub downloads: Option<VersionDownloads>,
    #[serde(rename = "assetIndex", default)]
    pub asset_index: Option<AssetIndexRef>,
    #[serde(default)]
    pub assets: Option<String>,
    #[serde(rename = "javaVersion", default)]
    pub java_version: Option<JavaVersion>,
    #[serde(default)]
    pub logging: Option<Logging>,
    #[serde(rename = "releaseTime", default)]
    pub release_time: String,
    #[serde(rename = "minimumLauncherVersion", default)]
    pub minimum_launcher_version: Option<i32>,
}

impl VersionDetails {
    pub fn asset_index_id(&self) -> String {
        self.asset_index
            .as_ref()
            .map(|a| a.id.clone())
            .or_else(|| self.assets.clone())
            .unwrap_or_else(|| "legacy".to_string())
    }

    pub fn java_component(&self) -> String {
        self.java_version
            .as_ref()
            .map(|j| j.component.clone())
            .filter(|c| !c.is_empty())
            .unwrap_or_else(|| "jre-legacy".to_string())
    }

    pub fn java_major(&self) -> i32 {
        self.java_version
            .as_ref()
            .map(|j| j.major_version)
            .filter(|v| *v > 0)
            .unwrap_or(8)
    }
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct AssetObject {
    pub hash: String,
    #[serde(default)]
    pub size: u64,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct AssetIndexFile {
    #[serde(default)]
    pub objects: HashMap<String, AssetObject>,
    #[serde(rename = "virtual", default)]
    pub is_virtual: bool,
    #[serde(rename = "map_to_resources", default)]
    pub map_to_resources: bool,
}

pub async fn fetch_manifest(client: &reqwest::Client) -> Result<VersionManifest> {
    let r = client
        .get(VERSION_MANIFEST_URL)
        .send()
        .await
        .context("fetching version manifest")?
        .error_for_status()?;
    Ok(r.json().await?)
}

pub async fn fetch_version(client: &reqwest::Client, url: &str) -> Result<VersionDetails> {
    let r = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("fetching version {url}"))?
        .error_for_status()?;
    Ok(r.json().await?)
}

pub async fn find_version(client: &reqwest::Client, id: &str) -> Result<VersionSummary> {
    let manifest = fetch_manifest(client).await?;
    manifest
        .versions
        .into_iter()
        .find(|v| v.id == id)
        .with_context(|| format!("version {id} not found in Mojang manifest"))
}

pub async fn fetch_assets_index(client: &reqwest::Client, url: &str) -> Result<AssetIndexFile> {
    let r = client
        .get(url)
        .send()
        .await?
        .error_for_status()?;
    Ok(r.json().await?)
}
