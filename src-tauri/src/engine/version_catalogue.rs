use crate::engine::download::{fetch_json, Progress};
use crate::engine::manifest::{VersionManifest, VersionSummary};
use crate::engine::paths::Dirs;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const CACHE_MAX_AGE_SECS: u64 = 3600; // 1 hour

#[derive(Clone, Debug, Default)]
pub enum VersionOrder {
    #[default]
    NewestFirst,
    OldestFirst,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VersionInfo {
    pub id: String,
    pub kind: String,
    pub release_time: String,
    pub java_major: Option<i32>,
    pub installed: bool,
    pub client_size_mb: Option<f64>,
}

#[derive(Clone, Debug, Default)]
pub enum VersionTypeFilter {
    #[default]
    All,
    Release,
    Snapshot,
    Legacy,
    ExcludeSnapshot,
}

#[derive(Clone, Debug)]
struct CachedManifest {
    manifest: VersionManifest,
    fetched_at: SystemTime,
}

pub struct VersionCatalogue {
    client: reqwest::Client,
    dirs: Dirs,
    cache: Arc<Mutex<Option<CachedManifest>>>,
}

impl VersionCatalogue {
    pub fn new(client: reqwest::Client, dirs: Dirs) -> Self {
        Self {
            client,
            dirs,
            cache: Arc::new(Mutex::new(None)),
        }
    }

    /// Fetch the version manifest from Mojang, using cache when fresh.
    pub async fn fetch_manifest(&self, force: bool) -> Result<VersionManifest> {
        let now = SystemTime::now();

        {
            let cache = self.cache.lock().unwrap();
            if !force {
                if let Some(cached) = cache.as_ref() {
                    if let Ok(age) = now.duration_since(cached.fetched_at) {
                        if age.as_secs() < CACHE_MAX_AGE_SECS {
                            return Ok(cached.manifest.clone());
                        }
                    }
                }
            }
        }

        let manifest: VersionManifest =
            fetch_json(&self.client, VERSION_MANIFEST_URL)
                .await
                .context("fetching Minecraft version manifest from Mojang")?;

        let mut cache = self.cache.lock().unwrap();
        *cache = Some(CachedManifest {
            manifest: manifest.clone(),
            fetched_at: now,
        });

        Ok(manifest)
    }

    /// List version summaries, optionally filtered by type and ordered.
    pub fn list_versions(
        &self,
        manifest: &VersionManifest,
        filter: &VersionTypeFilter,
        order: VersionOrder,
    ) -> Vec<VersionSummary> {
        let mut versions: Vec<VersionSummary> = manifest
            .versions
            .iter()
            .filter(|v| matches_filter(v, filter))
            .cloned()
            .collect();

        match order {
            VersionOrder::NewestFirst => {
                versions.sort_by(|a, b| b.release_time.cmp(&a.release_time));
            }
            VersionOrder::OldestFirst => {
                versions.sort_by(|a, b| a.release_time.cmp(&b.release_time));
            }
        }
        versions
    }

    /// Get detailed info for a specific version, including install status.
    pub async fn version_info(
        &self,
        id: &str,
        manifest: &VersionManifest,
    ) -> Result<VersionInfo> {
        let entry = manifest
            .versions
            .iter()
            .find(|v| v.id == id)
            .ok_or_else(|| anyhow!("version {id} not found in manifest"))?;

        let details = crate::engine::manifest::fetch_version(&self.client, &entry.url)
            .await
            .context(format!("fetching version details for {id}"))?;

        let installed = self.is_installed(id);
        let client_size_mb = details
            .downloads
            .as_ref()
            .and_then(|d| d.client.as_ref())
            .map(|c| c.size.unwrap_or(0) as f64 / 1_048_576.0);

        Ok(VersionInfo {
            id: entry.id.clone(),
            kind: entry.kind.clone(),
            release_time: entry.release_time.clone(),
            java_major: Some(details.java_major()),
            installed,
            client_size_mb,
        })
    }

    /// Check whether a version is installed locally (client jar exists).
    pub fn is_installed(&self, id: &str) -> bool {
        self.dirs.client_jar(id).exists()
    }

    /// Get cached manifest without fetching (for offline use).
    pub fn cached_manifest(&self) -> Option<VersionManifest> {
        self.cache.lock().unwrap().as_ref().map(|c| c.manifest.clone())
    }
}

fn matches_filter(version: &VersionSummary, filter: &VersionTypeFilter) -> bool {
    match filter {
        VersionTypeFilter::All => true,
        VersionTypeFilter::Release => version.kind == "release",
        VersionTypeFilter::Snapshot => version.kind == "snapshot",
        VersionTypeFilter::Legacy => {
            version.kind == "old_beta" || version.kind == "old_alpha"
        }
        VersionTypeFilter::ExcludeSnapshot => version.kind != "snapshot",
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct CataloguePayload {
    pub versions: Vec<VersionSummary>,
    pub latest_release: Option<String>,
    pub latest_snapshot: Option<String>,
    pub total: usize,
    pub cached: bool,
}

impl VersionCatalogue {
    pub fn payload(
        &self,
        manifest: &VersionManifest,
        filter: &VersionTypeFilter,
    ) -> CataloguePayload {
        let versions = self.list_versions(manifest, filter, VersionOrder::NewestFirst);
        CataloguePayload {
            versions,
            latest_release: Some(manifest.latest.release.clone()),
            latest_snapshot: Some(manifest.latest.snapshot.clone()),
            total: manifest.versions.len(),
            cached: true, // if we have a manifest, it came from cache or fetch
        }
    }
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_filter_release() {
        let v = VersionSummary {
            id: "1.20.1".to_string(),
            kind: "release".to_string(),
            url: "https://example.com/1.20.1.json".to_string(),
            time: "2023-01-01T00:00:00+00:00".to_string(),
            release_time: "2023-01-01T00:00:00+00:00".to_string(),
            sha1: "abc".to_string(),
            complianceLevel: 1,
        };
        assert!(matches_filter(&v, &VersionTypeFilter::All));
        assert!(matches_filter(&v, &VersionTypeFilter::Release));
        assert!(!matches_filter(&v, &VersionTypeFilter::Snapshot));
        assert!(!matches_filter(&v, &VersionTypeFilter::Legacy));
    }

    #[test]
    fn test_matches_filter_snapshot() {
        let v = VersionSummary {
            id: "23w42a".to_string(),
            kind: "snapshot".to_string(),
            url: "https://example.com/23w42a.json".to_string(),
            time: "2023-10-01T00:00:00+00:00".to_string(),
            release_time: "2023-10-01T00:00:00+00:00".to_string(),
            sha1: "def".to_string(),
            complianceLevel: 1,
        };
        assert!(!matches_filter(&v, &VersionTypeFilter::Release));
        assert!(matches_filter(&v, &VersionTypeFilter::Snapshot));
        assert!(matches_filter(&v, &VersionTypeFilter::ExcludeSnapshot) == false);
    }

    #[test]
    fn test_matches_filter_legacy() {
        let v = VersionSummary {
            id: "1.8.9".to_string(),
            kind: "old_beta".to_string(),
            url: "https://example.com/1.8.9.json".to_string(),
            time: "2015-01-01T00:00:00+00:00".to_string(),
            release_time: "2015-01-01T00:00:00+00:00".to_string(),
            sha1: "ghi".to_string(),
            complianceLevel: 1,
        };
        assert!(matches_filter(&v, &VersionTypeFilter::Legacy));
        assert!(!matches_filter(&v, &VersionTypeFilter::Release));
    }

    #[test]
    fn test_now_secs_is_reasonable() {
        let secs = now_secs();
        assert!(secs > 1_700_000_000); // after year 2023
    }
}
