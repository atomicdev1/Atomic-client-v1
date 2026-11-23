use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Os {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Rule {
    pub action: String,
    #[serde(default)]
    pub os: Option<Os>,
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
}

pub fn os_name() -> &'static str {
    match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "osx",
        _ => "linux",
    }
}

pub fn os_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86" => "x86",
        "aarch64" => "arm64",
        _ => "x86_64",
    }
}

fn parse_version(v: &str) -> Vec<u64> {
    v.split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<u64>().unwrap_or(0))
        .collect()
}

fn cmp_versions(a: &[u64], b: &[u64]) -> std::cmp::Ordering {
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x != y {
            return x.cmp(&y);
        }
    }
    std::cmp::Ordering::Equal
}

fn os_version_matches(range: &str) -> bool {
    // Mojang only ever uses min/max bounds for windows version ranges.
    let current = current_os_version();
    let cur = parse_version(&current);
    if let Some(rest) = range.strip_prefix('[') {
        // e.g. [10.0,)  or  [10.0,10.0.17134]
        let inner = rest.trim_end_matches(']').trim_end_matches(')');
        let mut it = inner.split(',');
        let low = it.next().unwrap_or("").trim();
        let high = it.next().unwrap_or("").trim();
        if !low.is_empty() && cmp_versions(&cur, &parse_version(low)) == std::cmp::Ordering::Less {
            return false;
        }
        if !high.is_empty() && cmp_versions(&cur, &parse_version(high)) == std::cmp::Ordering::Greater {
            return false;
        }
        return true;
    }
    // Handle {"min": "x", "max": "y"} style is parsed elsewhere; this fallback
    // treats a bare version as an exact-prefix match.
    current.starts_with(&range.replace('\\', ""))
}

pub fn current_os_version() -> String {
    if cfg!(windows) {
        // Windows 10/11 both report 10.0 for compatibility purposes.
        "10.0.19045".to_string()
    } else {
        "0.0.0".to_string()
    }
}

pub fn rule_matches(rule: &Rule, features: &HashMap<String, bool>) -> bool {
    if let Some(os) = &rule.os {
        if let Some(name) = &os.name {
            if name != os_name() {
                return false;
            }
        }
        if let Some(arch) = &os.arch {
            let want = arch.as_str();
            let have = os_arch();
            let ok = match want {
                "x86" => have == "x86",
                "x86_64" | "amd64" => have == "x86_64",
                "arm64" | "aarch64" => have == "arm64",
                _ => true,
            };
            if !ok {
                return false;
            }
        }
        if let Some(ver) = &os.version {
            if !os_version_matches(ver) {
                return false;
            }
        }
    }
    if let Some(feature_map) = &rule.features {
        for (k, want) in feature_map {
            let have = features.get(k).copied().unwrap_or(false);
            if have != *want {
                return false;
            }
        }
    }
    true
}

/// Standard Mojang rule evaluation: no rules = allowed. Otherwise the last
/// matching rule wins and the default is denied.
pub fn allowed(rules: Option<&Vec<Rule>>, features: &HashMap<String, bool>) -> bool {
    match rules {
        None => true,
        Some(list) if list.is_empty() => true,
        Some(list) => {
            let mut allow = false;
            for r in list {
                if rule_matches(r, features) {
                    allow = r.action == "allow";
                }
            }
            allow
        }
    }
}

pub fn feature_map(demo: bool, custom_resolution: bool) -> HashMap<String, bool> {
    let mut m = HashMap::new();
    m.insert("is_demo_user".to_string(), demo);
    m.insert("has_custom_resolution".to_string(), custom_resolution);
    m.insert("has_quick_plays_support".to_string(), false);
    m.insert("is_quick_play_singleplayer".to_string(), false);
    m.insert("is_quick_play_multiplayer".to_string(), false);
    m.insert("is_quick_play_realms".to_string(), false);
    m
}
