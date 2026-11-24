use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::engine::download::{ensure_files, fetch_json, DlItem, Progress};
use crate::engine::manifest::JAVA_RUNTIME_MANIFEST_URL;
use crate::engine::paths::Dirs;

pub fn platform_key() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86") => "windows-x86",
        ("windows", "aarch64") => "windows-arm64",
        ("windows", _) => "windows-x64",
        ("macos", "aarch64") => "mac-os-arm64",
        ("macos", _) => "mac-os",
        ("linux", "x86") => "linux-i386",
        _ => "linux",
    }
}

pub fn java_exe_name() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

fn component_for_major(major: i32) -> &'static str {
    match major {
        i if i <= 8 => "jre-legacy",
        9..=16 => "java-runtime-alpha",
        17..=20 => "java-runtime-gamma",
        21..=23 => "java-runtime-delta",
        _ => "java-runtime-epsilon",
    }
}

/// Downloads (once) the Mojang-provided Java runtime matching the version and
/// returns the path to the java executable.
pub async fn ensure_java(
    client: &reqwest::Client,
    dirs: &Dirs,
    component: &str,
    major: i32,
    progress: Option<Progress>,
) -> Result<PathBuf> {
    let platform = platform_key();
    let all: Value = fetch_json(client, JAVA_RUNTIME_MANIFEST_URL)
        .await
        .context("fetching Mojang java runtime index")?;

    let platform_obj = all
        .get(platform)
        .with_context(|| format!("java runtime index has no platform {platform}"))?;

    let mut chosen: Option<&Value> = None;
    // Try the explicit component first, then a major-version based fallback.
    for comp in [component, component_for_major(major)] {
        if let Some(list) = platform_obj.get(comp).and_then(|v| v.as_array()) {
            if let Some(first) = list.first() {
                chosen = Some(first);
                break;
            }
        }
    }
    let entry = chosen.ok_or_else(|| {
        anyhow!(
            "no java runtime available for {component} / java {major} on {platform}"
        )
    })?;

    let manifest_url = entry["manifest"]["url"]
        .as_str()
        .ok_or_else(|| anyhow!("java runtime entry missing manifest url"))?;
    let manifest_url = if manifest_url.starts_with("http") {
        manifest_url.to_string()
    } else {
        format!("https://{manifest_url}")
    };

    let home = dirs.java_home(component, platform);
    std::fs::create_dir_all(&home)?;

    let mf: Value = fetch_json(client, &manifest_url)
        .await
        .context("fetching java runtime file manifest")?;
    let files = mf["files"]
        .as_object()
        .ok_or_else(|| anyhow!("java runtime manifest missing files"))?;

    let mut items: Vec<DlItem> = Vec::new();
    let mut exec_paths: Vec<PathBuf> = Vec::new();
    for (name, f) in files {
        let kind = f.get("type").and_then(|v| v.as_str()).unwrap_or("file");
        let dest = home.join(name.replace('/', std::path::MAIN_SEPARATOR_STR));
        if kind == "directory" {
            std::fs::create_dir_all(&dest)?;
            continue;
        }
        if kind != "file" {
            continue;
        }
        if f.get("executable").and_then(|v| v.as_bool()).unwrap_or(false) {
            exec_paths.push(dest.clone());
        }
        let raw = &f["downloads"]["raw"];
        let Some(url) = raw.get("url").and_then(|v| v.as_str()) else {
            continue;
        };
        // Prefer the uncompressed raw entry; skip lzma-only files.
        items.push(DlItem {
            url: url.to_string(),
            path: dest,
            sha1: raw.get("sha1").and_then(|v| v.as_str()).map(str::to_string),
            size: raw.get("size").and_then(|v| v.as_u64()),
        });
    }

    ensure_files(client, items, 8, progress).await?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for p in exec_paths {
            if let Ok(meta) = std::fs::metadata(&p) {
                let mut perm = meta.permissions();
                perm.set_mode(0o755);
                let _ = std::fs::set_permissions(&p, perm);
            }
        }
    }

    let exe = home.join("bin").join(java_exe_name());
    if exe.exists() {
        return Ok(exe);
    }
    if let Some(found) = find_java_recursive(&home) {
        return Ok(found);
    }
    Err(anyhow!(
        "java executable missing after runtime install at {}",
        home.display()
    ))
}

fn find_java_recursive(root: &Path) -> Option<PathBuf> {
    let target = java_exe_name();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.file_name().and_then(|s| s.to_str()) == Some(target) {
                return Some(p);
            }
        }
    }
    None
}

pub fn check_java_binary(path: &Path) -> Result<String> {
    let out = std::process::Command::new(path)
        .arg("-version")
        .output()
        .with_context(|| format!("running {}", path.display()))?;
    Ok(String::from_utf8_lossy(&out.stderr).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_for_major() {
        assert_eq!(component_for_major(8), "jre-legacy");
        assert_eq!(component_for_major(7), "jre-legacy");
        assert_eq!(component_for_major(9), "java-runtime-alpha");
        assert_eq!(component_for_major(16), "java-runtime-alpha");
        assert_eq!(component_for_major(17), "java-runtime-gamma");
        assert_eq!(component_for_major(20), "java-runtime-gamma");
        assert_eq!(component_for_major(21), "java-runtime-delta");
        assert_eq!(component_for_major(23), "java-runtime-delta");
        assert_eq!(component_for_major(24), "java-runtime-epsilon");
        assert_eq!(component_for_major(25), "java-runtime-epsilon");
    }

    #[test]
    fn test_platform_key_windows() {
        if cfg!(windows) {
            assert_eq!(platform_key(), "windows-x64");
        }
    }

    #[test]
    fn test_java_exe_name_windows() {
        if cfg!(windows) {
            assert_eq!(java_exe_name(), "java.exe");
        }
    }
}
