use anyhow::{anyhow, Context, Result};
use futures_util::stream::{self, StreamExt};
use sha1::{Digest, Sha1};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub type Progress = Arc<dyn Fn(u64, u64) + Send + Sync>;

#[derive(Clone, Debug)]
pub struct DlItem {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

pub fn sha1_hex(bytes: &[u8]) -> String {
    let mut h = Sha1::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

pub fn sha1_file(path: &std::path::Path) -> Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha1::new();
    let mut buf = vec![0u8; 1024 * 128];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}

fn looks_present(item: &DlItem) -> bool {
    if let Ok(meta) = std::fs::metadata(&item.path) {
        if let Some(size) = item.size {
            if meta.len() == size {
                return true;
            }
        }
    }
    false
}

pub async fn ensure_files(
    client: &reqwest::Client,
    items: Vec<DlItem>,
    concurrency: usize,
    progress: Option<Progress>,
) -> Result<u64> {
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut unique: Vec<DlItem> = Vec::new();
    for it in items {
        if seen.insert(it.path.clone()) {
            unique.push(it);
        }
    }

    let total: u64 = unique.iter().filter_map(|i| i.size).sum();
    let done = Arc::new(AtomicU64::new(0));
    let mut skipped: u64 = 0;
    let mut pending: Vec<DlItem> = Vec::new();
    for it in unique {
        if looks_present(&it) {
            skipped += it.size.unwrap_or(0);
        } else {
            pending.push(it);
        }
    }
    done.store(skipped, Ordering::SeqCst);
    if let Some(cb) = &progress {
        cb(skipped, total);
    }

    let client = client.clone();
    let results: Vec<Result<u64>> = stream::iter(pending)
        .map(|it| {
            let client = client.clone();
            let done = done.clone();
            let progress = progress.clone();
            async move {
                let resp = client
                    .get(&it.url)
                    .send()
                    .await
                    .with_context(|| format!("GET {}", it.url))?
                    .error_for_status()?;
                let bytes = resp.bytes().await?;
                if let Some(size) = it.size {
                    if bytes.len() as u64 != size {
                        return Err(anyhow!(
                            "size mismatch for {:?}: {} != {}",
                            it.path,
                            bytes.len(),
                            size
                        ));
                    }
                }
                if let Some(want) = &it.sha1 {
                    let got = sha1_hex(&bytes);
                    if !got.eq_ignore_ascii_case(want) {
                        return Err(anyhow!(
                            "sha1 mismatch for {:?}: {} != {}",
                            it.path,
                            got,
                            want
                        ));
                    }
                }
                if let Some(parent) = it.path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let tmp = it.path.with_extension("part");
                std::fs::write(&tmp, &bytes)?;
                std::fs::rename(&tmp, &it.path)?;
                let n = done.fetch_add(bytes.len() as u64, Ordering::SeqCst)
                    + bytes.len() as u64;
                if let Some(cb) = &progress {
                    cb(n, total);
                }
                Ok(bytes.len() as u64)
            }
        })
        .buffer_unordered(concurrency.max(1))
        .collect()
        .await;

    let mut downloaded = 0u64;
    for r in results {
        downloaded += r?;
    }
    Ok(downloaded)
}

pub async fn fetch_json<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
) -> Result<T> {
    let r = client.get(url).send().await?.error_for_status()?;
    Ok(r.json().await?)
}

pub async fn fetch_text(client: &reqwest::Client, url: &str) -> Result<String> {
    let r = client.get(url).send().await?.error_for_status()?;
    Ok(r.text().await?)
}
