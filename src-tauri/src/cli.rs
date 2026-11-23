use std::sync::Arc;

use anyhow::Result;
use serde_json::json;

use crate::engine::auth::offline_account;
use crate::engine::download::Progress;
use crate::engine::launch::{install_and_launch, LaunchOptions};
use crate::engine::manifest::fetch_manifest;
use crate::engine::paths::Dirs;
use crate::engine::settings::Settings;

struct CliFlags {
    loader: String,
    instance: String,
    demo: bool,
    dry_run: bool,
    memory: u32,
    name: String,
    version: String,
}

fn parse(args: &[String]) -> CliFlags {
    let mut f = CliFlags {
        loader: "vanilla".to_string(),
        instance: "default".to_string(),
        demo: false,
        dry_run: false,
        memory: 4096,
        name: "Player".to_string(),
        version: String::new(),
    };
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        let next = args.get(i + 1).cloned().unwrap_or_default();
        if let Some(v) = a.strip_prefix("--loader=") {
            f.loader = v.to_string();
        } else if let Some(v) = a.strip_prefix("--instance=") {
            f.instance = v.to_string();
        } else if let Some(v) = a.strip_prefix("--memory=") {
            f.memory = v.parse().unwrap_or(4096);
        } else if let Some(v) = a.strip_prefix("--name=") {
            f.name = v.to_string();
        } else if a == "--loader" {
            f.loader = next;
            i += 1;
        } else if a == "--instance" {
            f.instance = next;
            i += 1;
        } else if a == "--memory" {
            f.memory = next.parse().unwrap_or(4096);
            i += 1;
        } else if a == "--name" {
            f.name = next;
            i += 1;
        } else if a == "--demo" {
            f.demo = true;
        } else if a == "--dry-run" {
            f.dry_run = true;
        } else if f.version.is_empty() && !a.starts_with("--") {
            f.version = a.clone();
        }
        i += 1;
    }
    f
}

pub fn run(args: &[String]) -> i32 {
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("failed to start async runtime: {e}");
            return 1;
        }
    };
    match rt.block_on(async_main(args)) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: {e:#}");
            1
        }
    }
}

async fn async_main(args: &[String]) -> Result<()> {
    let dirs = Dirs::discover();
    let settings = Settings::load(&dirs);
    let client = reqwest::Client::builder()
        .user_agent(concat!("AtomicClient/", env!("CARGO_PKG_VERSION")))
        .build()?;

    let cmd = args.first().map(|s| s.as_str()).unwrap_or("help");
    match cmd {
        "versions" => {
            let all = args.iter().any(|a| a == "--all");
            let m = fetch_manifest(&client).await?;
            if all {
                for v in &m.versions {
                    println!("{}\t{}\t{}", v.id, v.kind, v.release_time);
                }
            } else {
                for v in m.versions.iter().filter(|v| v.kind == "release") {
                    println!("{}", v.id);
                }
            }
            Ok(())
        }
        "where" => {
            println!("data      {}", dirs.root.display());
            println!("instances {}", dirs.instances.display());
            println!("libraries {}", dirs.libraries.display());
            println!("assets    {}", dirs.assets.display());
            println!("runtime   {}", dirs.runtime.display());
            Ok(())
        }
        "install" | "launch" => {
            let flags = parse(&args[1..]);
            if flags.version.is_empty() {
                anyhow::bail!("usage: atomic-client {cmd} <version> [--loader fabric|quilt|vanilla] [--instance name] [--memory 4096] [--demo]");
            }
            let acc = offline_account(&flags.name);
            let opts = LaunchOptions {
                version: flags.version,
                loader: flags.loader,
                player_name: acc.name,
                uuid: acc.uuid,
                access_token: acc.access_token,
                user_type: acc.user_type,
                xuid: String::new(),
                client_id: String::new(),
                demo: flags.demo,
                memory_mb: flags.memory,
                java_path: if settings.java_path.trim().is_empty() {
                    None
                } else {
                    Some(std::path::PathBuf::from(&settings.java_path))
                },
                instance: flags.instance,
                dry_run: cmd == "install" || flags.dry_run,
                width: 1280,
                height: 720,
            };
            let progress: Progress = Arc::new(|done, total| {
                if total > 0 {
                    let pct = (done as f64 / total as f64 * 100.0).min(100.0);
                    print!("\r[download] {:5.1}% ({:.1}/{:.1} MB)", pct, done as f64 / 1_048_576.0, total as f64 / 1_048_576.0);
                    use std::io::Write;
                    let _ = std::io::stdout().flush();
                }
            });
            let res = install_and_launch(&client, &dirs, &opts, Some(progress), None).await?;
            println!("\n{}", json!({"version": res.version_id, "pid": res.pid, "main": res.main_class, "instance": res.instance_dir}));
            println!("command:\n{}", res.command);
            if res.pid.is_some() {
                // Give the log readers a moment to surface early output.
                tokio::time::sleep(std::time::Duration::from_secs(8)).await;
            }
            Ok(())
        }
        "search" => {
            let q = args[1..].join(" ");
            let hits = crate::engine::modrinth::search(&client, &q, 15).await?;
            for h in hits {
                println!("{:<28} {:<40} {}", h.slug, h.title, h.description);
            }
            Ok(())
        }
        _ => {
            println!("Atomic Client {}", env!("CARGO_PKG_VERSION"));
            println!("commands:");
            println!("  versions [--all]                list Minecraft versions");
            println!("  install <version> [flags]       download everything, print launch command");
            println!("  launch <version> [flags]        install + start the game");
            println!("  search <query>                  search Modrinth mods");
            println!("  where                            show data directories");
            println!("flags: --loader vanilla|fabric|quilt, --instance NAME, --memory MB, --demo, --name PLAYER");
            Ok(())
        }
    }
}
