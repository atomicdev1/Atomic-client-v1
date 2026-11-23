use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::engine::manifest::{ArgEntry, ArgValue, VersionDetails};
use crate::engine::rules::{allowed, Rule};

#[derive(Clone, Debug, Default)]
pub struct PlayerInfo {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    pub user_type: String,
    pub xuid: String,
    pub client_id: String,
}

pub struct ArgContext<'a> {
    pub details: &'a VersionDetails,
    pub classpath: &'a [PathBuf],
    pub natives_directory: &'a str,
    pub game_dir: &'a Path,
    pub assets_dir: &'a Path,
    pub assets_index: &'a str,
    pub virtual_assets: Option<&'a Path>,
    pub player: &'a PlayerInfo,
    pub launcher_name: &'a str,
    pub launcher_version: &'a str,
    pub memory_mb: u32,
    pub extra_jvm: &'a [String],
    pub demo: bool,
    pub width: u32,
    pub height: u32,
}

fn classpath_sep() -> &'static str {
    if cfg!(windows) {
        ";"
    } else {
        ":"
    }
}

fn subst(input: &str, map: &HashMap<&'static str, String>) -> String {
    let mut out = input.to_string();
    for (k, v) in map {
        out = out.replace(&format!("${{{k}}}"), v);
    }
    out
}

fn collect_entries(
    entries: &[ArgEntry],
    features: &HashMap<String, bool>,
    map: &HashMap<&'static str, String>,
    out: &mut Vec<String>,
) {
    for e in entries {
        match e {
            ArgEntry::Plain(s) => out.push(subst(s, map)),
            ArgEntry::Conditional(cond) => {
                let rules: Option<&Vec<Rule>> = Some(&cond.rules);
                if allowed(rules, features) {
                    let mut vals: Vec<String> = Vec::new();
                    match &cond.value {
                        ArgValue::One(s) => vals.push(s.clone()),
                        ArgValue::Many(v) => vals.extend(v.iter().cloned()),
                    }
                    for v in vals {
                        out.push(subst(&v, map));
                    }
                }
            }
        }
    }
}

pub fn build(ctx: &ArgContext<'_>) -> (Vec<String>, Vec<String>) {
    let mut features = HashMap::new();
    features.insert("is_demo_user".to_string(), ctx.demo);
    features.insert("has_custom_resolution".to_string(), true);
    features.insert("has_quick_plays_support".to_string(), false);

    let cp = ctx
        .classpath
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(classpath_sep());

    let game_assets = ctx
        .virtual_assets
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ctx.assets_dir.to_string_lossy().to_string());

    let mut map: HashMap<&'static str, String> = HashMap::new();
    map.insert("auth_player_name", ctx.player.name.clone());
    map.insert("version_name", ctx.details.id.clone());
    map.insert(
        "game_directory",
        ctx.game_dir.to_string_lossy().to_string(),
    );
    map.insert("assets_root", ctx.assets_dir.to_string_lossy().to_string());
    map.insert("assets_index_name", ctx.assets_index.to_string());
    map.insert("game_assets", game_assets);
    map.insert("auth_uuid", ctx.player.uuid.replace('-', ""));
    map.insert("auth_access_token", ctx.player.access_token.clone());
    map.insert("auth_session", format!("token:{}", ctx.player.access_token));
    map.insert("clientid", ctx.player.client_id.clone());
    map.insert("auth_xuid", ctx.player.xuid.clone());
    map.insert("user_type", ctx.player.user_type.clone());
    map.insert(
        "version_type",
        if ctx.details.kind.is_empty() {
            "release".to_string()
        } else {
            ctx.details.kind.clone()
        },
    );
    map.insert("natives_directory", ctx.natives_directory.to_string());
    map.insert("launcher_name", ctx.launcher_name.to_string());
    map.insert("launcher_version", ctx.launcher_version.to_string());
    map.insert("classpath", cp);
    map.insert("user_properties", "{}".to_string());
    map.insert("resolution_width", ctx.width.to_string());
    map.insert("resolution_height", ctx.height.to_string());

    let mut jvm: Vec<String> = Vec::new();
    let mut game: Vec<String> = Vec::new();

    if let Some(args) = &ctx.details.arguments {
        collect_entries(&args.jvm, &features, &map, &mut jvm);
        if ctx.memory_mb == 0 {
            collect_entries(&args.default_user_jvm, &features, &map, &mut jvm);
        }
        collect_entries(&args.game, &features, &map, &mut game);
    } else if let Some(legacy) = &ctx.details.minecraft_arguments {
        for tok in legacy.split_whitespace() {
            game.push(subst(tok, &map));
        }
    }

    if ctx.memory_mb > 0 {
        jvm.push(format!("-Xmx{}M", ctx.memory_mb));
        jvm.push(format!("-Xms{}M", (ctx.memory_mb / 2).max(512)));
    }
    for extra in ctx.extra_jvm {
        jvm.push(extra.clone());
    }

    (jvm, game)
}
