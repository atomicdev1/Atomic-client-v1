use anyhow::{anyhow, Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::engine::args::{build, ArgContext, PlayerInfo};
use crate::engine::download::{ensure_files, fetch_text, DlItem, Progress};
use crate::engine::fabric::{fetch_profile, install_libs};
use crate::engine::java::ensure_java;
use crate::engine::manifest::{
    fetch_assets_index, fetch_version, find_version, AssetIndexFile, Artifact, Library,
    VersionDetails,
};
use crate::engine::paths::{path_from_maven, Dirs};
use crate::engine::rules::{allowed, feature_map, os_name};

pub type LogFn = Arc<dyn Fn(String) + Send + Sync>;

#[derive(Clone, Debug)]
pub struct LaunchOptions {
    pub version: String,
    pub loader: String,
    pub player_name: String,
    pub uuid: String,
    pub access_token: String,
    pub user_type: String,
    pub xuid: String,
    pub client_id: String,
    pub demo: bool,
    pub memory_mb: u32,
    pub java_path: Option<PathBuf>,
    pub instance: String,
    pub dry_run: bool,
    pub width: u32,
    pub height: u32,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        LaunchOptions {
            version: String::new(),
            loader: "vanilla".to_string(),
            player_name: "Player".to_string(),
            uuid: String::new(),
            access_token: "0".to_string(),
            user_type: "legacy".to_string(),
            xuid: String::new(),
            client_id: String::new(),
            demo: false,
            memory_mb: 4096,
            java_path: None,
            instance: "default".to_string(),
            dry_run: false,
            width: 854,
            height: 480,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct LaunchResult {
    pub command: String,
    pub pid: Option<u32>,
    pub version_id: String,
    pub instance_dir: String,
    pub main_class: String,
}

fn artifact_item(art: &Artifact, fallback_rel: Option<String>, root: &Path) -> Option<DlItem> {
    let rel = art.path.clone().or(fallback_rel)?;
    let url = art.url_norm()?;
    Some(DlItem {
        url,
        path: root.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR)),
        sha1: art.sha1.clone(),
        size: art.size,
    })
}

struct LibPlan {
    downloads: Vec<DlItem>,
    classpath: Vec<PathBuf>,
    natives: Vec<(PathBuf, Vec<String>)>,
}

fn plan_libraries(details: &VersionDetails, dirs: &Dirs, java_major: i32) -> LibPlan {
    let features = feature_map(false, true);
    let mut downloads = Vec::new();
    let mut classpath = Vec::new();
    let mut natives = Vec::new();
    let platform = os_name();

    for lib in &details.libraries {
        if !allowed(lib.rules.as_ref(), &features) {
            continue;
        }
        let mut lib_path: Option<PathBuf> = None;

        if let Some(dl) = &lib.downloads {
            if let Some(art) = &dl.artifact {
                if let Some(item) = artifact_item(art, None, &dirs.libraries) {
                    lib_path = Some(item.path.clone());
                    downloads.push(item);
                }
            }
            if let Some(classifiers) = &dl.classifiers {
                if let Some(natives_map) = &lib.natives {
                    if let Some(key) = natives_map.get(platform) {
                        if let Some(art) = classifiers.get(key) {
                            if let Some(item) = artifact_item(art, None, &dirs.libraries) {
                                let excludes = lib
                                    .extract
                                    .as_ref()
                                    .and_then(|e| e.exclude.clone())
                                    .unwrap_or_else(|| vec!["META-INF/".to_string()]);
                                natives.push((item.path.clone(), excludes));
                                downloads.push(item);
                            }
                        }
                    }
                }
            }
        } else if !lib.name.is_empty() {
            // Some manifests (and loader profiles) carry only maven coordinates.
            if let Some(rel) = path_from_maven(&lib.name) {
                if let (Some(url), _) = (lib.url.as_ref(), ()) {
                    let base = if url.starts_with("http") {
                        url.clone()
                    } else {
                        format!("https://{url}")
                    };
                    let base = base.trim_end_matches('/');
                    let path = dirs
                        .libraries
                        .join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
                    downloads.push(DlItem {
                        url: format!("{base}/{}", rel.replace('\\', "/")),
                        path: path.clone(),
                        sha1: lib.sha1.clone(),
                        size: lib.size,
                    });
                    lib_path = Some(path);
                }
            }
        }

        // Modern manifests ship native jars as their own library entries.
        if let Some(p) = &lib_path {
            let is_native_jar = lib
                .name
                .contains(":natives-")
                || p.to_string_lossy().contains("-natives-");
            if is_native_jar {
                let excludes = lib
                    .extract
                    .as_ref()
                    .and_then(|e| e.exclude.clone())
                    .unwrap_or_else(|| vec!["META-INF/".to_string()]);
                natives.push((p.clone(), excludes));
            }
        }

        if let Some(p) = lib_path {
            // Legacy LWJGL 2 native jars contain classes too; official launcher
            // keeps natives jars on the classpath, so we mirror that.
            classpath.push(p);
        }
    }

    // Java 8 and older need the legacy lwjgl jinput fix handled by the
    // classifier logic above; nothing else required here.
    let _ = java_major;
    LibPlan {
        downloads,
        classpath,
        natives,
    }
}

fn extract_natives(jar: &Path, dest: &Path, excludes: &[String]) -> Result<()> {
    let file = std::fs::File::open(jar)
        .with_context(|| format!("opening native jar {}", jar.display()))?;
    let mut archive = zip::ZipArchive::new(file)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        if entry.is_dir() {
            continue;
        }
        if excludes.iter().any(|e| name.starts_with(e)) {
            continue;
        }
        // Flatten like the official launcher does.
        let fname = name.rsplit('/').next().unwrap_or(&name);
        if fname.is_empty() {
            continue;
        }
        let out = dest.join(fname);
        std::fs::create_dir_all(dest)?;
        let mut f = std::fs::File::create(&out)?;
        std::io::copy(&mut entry, &mut f)?;
    }
    Ok(())
}

async fn install_assets(
    client: &reqwest::Client,
    dirs: &Dirs,
    details: &VersionDetails,
    progress: Option<Progress>,
    log: &Option<LogFn>,
) -> Result<(String, Option<PathBuf>)> {
    let index_id = details.asset_index_id();
    let mut virtual_dir: Option<PathBuf> = None;

    let Some(aref) = &details.asset_index else {
        return Ok((index_id, virtual_dir));
    };

    let index_path = dirs.index_file(&index_id);
    ensure_files(
        client,
        vec![DlItem {
            url: if aref.url.starts_with("http") {
                aref.url.clone()
            } else {
                format!("https://{}", aref.url)
            },
            path: index_path.clone(),
            sha1: aref.sha1.clone(),
            size: aref.size,
        }],
        1,
        progress.clone(),
    )
    .await?;

    let text = std::fs::read_to_string(&index_path)?;
    let index: AssetIndexFile = serde_json::from_str(&text)?;

    let mut items = Vec::with_capacity(index.objects.len());
    for (_, obj) in index.objects.iter() {
        items.push(DlItem {
            url: format!("https://resources.download.minecraft.net/{}/{}", &obj.hash[..2], obj.hash),
            path: dirs.object_path(&obj.hash),
            sha1: Some(obj.hash.clone()),
            size: Some(obj.size),
        });
    }
    if let Some(cb) = &log {
        cb(format!(
            "assets: {} objects ({:.1} MB)",
            items.len(),
            (items.iter().filter_map(|i| i.size).sum::<u64>() as f64) / 1_048_576.0
        ));
    }
    ensure_files(client, items, 16, progress).await?;

    if index.is_virtual || index.map_to_resources || aref.is_virtual || aref.map_to_resources {
        let vdir = dirs.virtual_assets(&index_id);
        std::fs::create_dir_all(&vdir)?;
        for (name, obj) in index.objects.iter() {
            let src = dirs.object_path(&obj.hash);
            let dst = vdir.join(name.replace('/', std::path::MAIN_SEPARATOR_STR));
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent)?;
            }
            if !dst.exists() {
                let _ = std::fs::copy(&src, &dst);
            }
        }
        if let Some(cb) = &log {
            cb(format!("assets: virtual layout materialized at {}", vdir.display()));
        }
        virtual_dir = Some(vdir);
    }

    Ok((index_id, virtual_dir))
}

pub async fn install_and_launch(
    client: &reqwest::Client,
    dirs: &Dirs,
    opts: &LaunchOptions,
    progress: Option<Progress>,
    log: Option<LogFn>,
) -> Result<LaunchResult> {
    let say = |m: String| {
        if let Some(cb) = &log {
            cb(m.clone());
        }
        println!("[atomic] {m}");
    };

    if opts.version.trim().is_empty() {
        return Err(anyhow!("no version selected"));
    }

    say(format!("resolving Minecraft {} ...", opts.version));
    let summary = find_version(client, &opts.version).await?;
    let raw = fetch_text(client, &summary.url).await?;
    let details: VersionDetails = serde_json::from_str(&raw)?;
    let vdir = dirs.version_dir(&opts.version);
    std::fs::create_dir_all(&vdir)?;

    // Persist the raw version json for offline reuse / debugging.
    let _ = std::fs::write(dirs.version_json(&opts.version), &raw);

    let features = feature_map(opts.demo, true);
    let java_major = details.java_major();

    // 1) client jar
    if let Some(client_art) = details.downloads.as_ref().and_then(|d| d.client.as_ref()) {
        if let Some(item) = artifact_item(client_art, Some(format!("{}.jar", opts.version)), &vdir) {
            let item = DlItem {
                path: dirs.client_jar(&opts.version),
                ..item
            };
            say("client jar ...".to_string());
            ensure_files(client, vec![item], 1, progress.clone()).await?;
        }
    }

    // 2) loader
    let mut main_class = details.main_class.clone();
    let mut loader_cp: Vec<PathBuf> = Vec::new();
    let loader = opts.loader.to_ascii_lowercase();
    let mut effective = details.clone();
    if loader == "fabric" || loader == "quilt" {
        say(format!("resolving {loader} loader for {} ...", opts.version));
        let profile = fetch_profile(client, &opts.version, &loader).await?;
        say(format!(
            "{loader} {}{} ready",
            if loader == "quilt" { "quilt-loader " } else { "" },
            profile.loader_version
        ));
        loader_cp = install_libs(client, dirs, &profile, progress.clone()).await?;
        main_class = profile.main_class.clone();
        effective.main_class = profile.main_class;
    }

    // 3) libraries + natives
    let plan = plan_libraries(&details, dirs, java_major);
    say(format!(
        "libraries: {} downloads, {} native jars",
        plan.downloads.len(),
        plan.natives.len()
    ));
    ensure_files(client, plan.downloads, 16, progress.clone()).await?;

    let display_version = if loader == "fabric" || loader == "quilt" {
        format!("{loader}-{}", opts.version)
    } else {
        opts.version.clone()
    };
    let natives_dir = dirs.natives(&display_version);
    let _ = std::fs::remove_dir_all(&natives_dir);
    std::fs::create_dir_all(&natives_dir)?;
    let mut lib_path_dirs: Vec<String> = Vec::new();
    for (i, (jar, excludes)) in plan.natives.iter().enumerate() {
        let extract_dir = natives_dir.join(format!("native-{i}"));
        extract_natives(jar, &extract_dir, excludes)?;
        lib_path_dirs.push(extract_dir.to_string_lossy().to_string());
    }
    lib_path_dirs.push(natives_dir.to_string_lossy().to_string());
    let library_path = lib_path_dirs.join(";");
    say(format!("natives extracted ({} jars)", plan.natives.len()));

    // 4) assets
    let (assets_index, virtual_assets) =
        install_assets(client, dirs, &details, progress.clone(), &log).await?;

    // 5) logging config
    let mut logging_arg: Option<String> = None;
    if let Some(lg) = &details.logging {
        if let Some(url) = &lg.client.file.url {
            let dest = dirs.log_config(&opts.version);
            let item = DlItem {
                url: if url.starts_with("http") {
                    url.clone()
                } else {
                    format!("https://{url}")
                },
                path: dest.clone(),
                sha1: lg.client.file.sha1.clone(),
                size: lg.client.file.size,
            };
            ensure_files(client, vec![item], 1, progress.clone()).await?;
            logging_arg = Some(
                lg.client
                    .argument
                    .replace("${path}", &dest.to_string_lossy()),
            );
        }
    }

    // 6) java runtime
    let java = match &opts.java_path {
        Some(p) if !p.as_os_str().is_empty() => p.clone(),
        _ => {
            say(format!(
                "java runtime {} (java {}) ...",
                details.java_component(),
                java_major
            ));
            ensure_java(
                client,
                dirs,
                &details.java_component(),
                java_major,
                progress.clone(),
            )
            .await?
        }
    };
    say(format!("java: {}", java.display()));

    // 7) classpath + args
    let mut classpath: Vec<PathBuf> = Vec::new();
    classpath.push(dirs.client_jar(&opts.version));
    classpath.extend(plan.classpath.iter().cloned());
    classpath.extend(loader_cp.iter().cloned());
    classpath.retain(|p| p.exists());

    let game_dir = dirs.instance(&opts.instance);
    std::fs::create_dir_all(&game_dir)?;
    let _ = std::fs::create_dir_all(game_dir.join("mods"));
    let _ = std::fs::create_dir_all(game_dir.join("logs"));

    let player = PlayerInfo {
        name: opts.player_name.clone(),
        uuid: opts.uuid.clone(),
        access_token: opts.access_token.clone(),
        user_type: opts.user_type.clone(),
        xuid: opts.xuid.clone(),
        client_id: opts.client_id.clone(),
    };

    let mut extra_jvm = Vec::new();
    if let Some(l) = &logging_arg {
        extra_jvm.push(l.clone());
    }

    let ctx = ArgContext {
        details: &effective,
        classpath: &classpath,
        natives_directory: &library_path,
        game_dir: &game_dir,
        assets_dir: &dirs.assets,
        assets_index: &assets_index,
        virtual_assets: virtual_assets.as_deref(),
        player: &player,
        launcher_name: "AtomicClient",
        launcher_version: env!("CARGO_PKG_VERSION"),
        memory_mb: opts.memory_mb,
        extra_jvm: &extra_jvm,
        demo: opts.demo,
        width: opts.width,
        height: opts.height,
    };
    let (jvm_args, game_args) = build(&ctx);

    let mut full: Vec<String> = Vec::new();
    full.extend(jvm_args.iter().cloned());
    full.push(main_class.clone());
    full.extend(game_args.iter().cloned());

    let printable = format!("\"{}\" {}", java.display(), full.join(" "));
    if opts.dry_run {
        say("dry run - command below".to_string());
        return Ok(LaunchResult {
            command: printable,
            pid: None,
            version_id: display_version,
            instance_dir: game_dir.to_string_lossy().to_string(),
            main_class,
        });
    }

    say("launching ...".to_string());
    let log_path = game_dir.join("logs").join("atomic-launch.log");
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok();
    if let Some(mut f) = log_file.as_ref() {
        let _ = writeln!(f, "\n=== {} {} ({}) ===", opts.version, opts.loader, printable);
    }

    let mut cmd = Command::new(&java);
    cmd.args(&full);
    cmd.current_dir(&game_dir);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        // Keep the child alive if the launcher closes.
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }

    let mut child = cmd.spawn().with_context(|| {
        format!("spawning java at {}", java.display())
    })?;
    let pid = child.id();

    if let Some(stdout) = child.stdout.take() {
        let log = log.clone();
        let path = log_path.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
                    let _ = writeln!(f, "{line}");
                }
                if let Some(cb) = &log {
                    cb(line);
                }
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let log = log.clone();
        let path = log_path.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
                    let _ = writeln!(f, "{line}");
                }
                if let Some(cb) = &log {
                    cb(line);
                }
            }
        });
    }

    // Reap in the background so we do not leave zombies.
    tokio::spawn(async move {
        let _ = child.wait().await;
    });

    Ok(LaunchResult {
        command: printable,
        pid,
        version_id: display_version,
        instance_dir: game_dir.to_string_lossy().to_string(),
        main_class,
    })
}
