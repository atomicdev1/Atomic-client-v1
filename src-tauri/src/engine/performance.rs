use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Performance profile selection. DEFAULT preserves vanilla visuals exactly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerformanceProfile {
    #[default]
    Default,
    Balanced,
    Performance,
}

impl PerformanceProfile {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "default" => Some(Self::Default),
            "balanced" => Some(Self::Balanced),
            "performance" => Some(Self::Performance),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Balanced => "balanced",
            Self::Performance => "performance",
        }
    }
}

/// Documented Minecraft 1.20.1 options.txt keys used by the profiles.
/// Every key below is verified against 1.20.1. Keys from newer versions are NOT used.
const OPTION_RENDER_DISTANCE: &str = "renderDistance";
const OPTION_SIMULATION_DISTANCE: &str = "simulationDistance";
const OPTION_ENTITY_DISTANCE_SCALING: &str = "entityDistanceScaling";
const OPTION_AO: &str = "ao";
const OPTION_MIPMAP_LEVELS: &str = "mipmapLevels";
const OPTION_PARTICLES: &str = "particles";
const OPTION_MAX_FPS: &str = "maxFps";
const OPTION_GRAPHICS_MODE: &str = "graphicsMode";
const OPTION_ENABLE_VSYNC: &str = "enableVsync";
const OPTION_ENTITY_SHADOWS: &str = "entityShadows";
const OPTION_RENDER_CLOUDS: &str = "renderClouds";
const OPTION_BIOME_BLEND_RADIUS: &str = "biomeBlendRadius";

/// Returns the setting changes for each profile as (key, value, visual_trade_off).
/// DEFAULT returns an empty map — vanilla visuals are preserved exactly.
pub fn profile_changes(profile: PerformanceProfile) -> BTreeMap<String, String> {
    let mut changes: BTreeMap<String, String> = BTreeMap::new();

    match profile {
        PerformanceProfile::Default => {
            // Intentionally empty — no changes, vanilla visuals preserved.
        }
        PerformanceProfile::Balanced => {
            // Modest reductions. Visual impact: slightly shorter view distance,
            // fewer particles, mipmaps still full quality.
            changes.insert(OPTION_RENDER_DISTANCE.to_string(), "10".to_string());
            changes.insert(OPTION_PARTICLES.to_string(), "1".to_string());
            changes.insert(OPTION_MAX_FPS.to_string(), "144".to_string());
        }
        PerformanceProfile::Performance => {
            // Stronger reductions. Visual impact: noticeably shorter view distance,
            // minimal particles, lower mipmap quality, no clouds/shadows, Fast graphics.
            changes.insert(OPTION_RENDER_DISTANCE.to_string(), "6".to_string());
            changes.insert(OPTION_SIMULATION_DISTANCE.to_string(), "6".to_string());
            changes.insert(OPTION_PARTICLES.to_string(), "2".to_string());
            changes.insert(OPTION_MIPMAP_LEVELS.to_string(), "2".to_string());
            changes.insert(OPTION_MAX_FPS.to_string(), "120".to_string());
            changes.insert(OPTION_GRAPHICS_MODE.to_string(), "0".to_string()); // Fast
            changes.insert(OPTION_ENABLE_VSYNC.to_string(), "false".to_string());
            changes.insert(OPTION_ENTITY_SHADOWS.to_string(), "false".to_string());
            changes.insert(OPTION_RENDER_CLOUDS.to_string(), "false".to_string());
            changes.insert(OPTION_BIOME_BLEND_RADIUS.to_string(), "1".to_string());
        }
    }

    changes
}

/// Returns a human-readable explanation of what each profile changes.
pub fn profile_description(profile: PerformanceProfile) -> String {
    match profile {
        PerformanceProfile::Default => {
            "No changes. Minecraft runs with its default options.txt exactly as-is."
                .to_string()
        }
        PerformanceProfile::Balanced => {
            "renderDistance=10, particles=decreased, maxFps=144.\
             Trade-off: shorter view distance and fewer particle effects.\
             Graphics remain Fancy."
                .to_string()
        }
        PerformanceProfile::Performance => {
            "renderDistance=6, simulationDistance=6, particles=minimal,\
             mipmapLevels=2, maxFps=120, graphics=Fast, VSync off,\
             entity shadows off, clouds off, biomeBlendRadius=1.\
             Trade-off: significantly shorter view distance, minimal particles,\
             lower mipmap quality, no clouds or entity shadows."
                .to_string()
        }
    }
}

/// Parses an options.txt file into a BTreeMap. Missing or malformed file returns empty map.
fn read_options(path: &Path) -> BTreeMap<String, String> {
    let Ok(text) = fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    let mut map = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            map.insert(key.to_string(), value.to_string());
        }
    }
    map
}

/// Writes a BTreeMap back to options.txt.
fn write_options(path: &Path, options: &BTreeMap<String, String>) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut text = String::new();
    for (key, value) in options {
        text.push_str(&format!("{key}={value}\n"));
    }
    fs::write(path, text)?;
    Ok(())
}

/// Applies a performance profile to the instance's options.txt.
///
/// - Backs up the original options.txt as options.txt.atomic-backup before any change.
/// - Only changes keys documented for the selected profile.
/// - Never touches keys not in the profile.
/// - Returns the list of keys that were changed.
pub fn apply_profile(
    instance_dir: &Path,
    profile: PerformanceProfile,
) -> Result<Vec<String>> {
    let options_path = instance_dir.join("options.txt");
    let backup_path = instance_dir.join("options.txt.atomic-backup");

    let mut options = read_options(&options_path);
    let changes = profile_changes(profile);

    // Back up the original before modifying (only once).
    if !backup_path.exists() && options_path.exists() {
        fs::copy(&options_path, &backup_path)?;
    }

    let mut applied: Vec<String> = Vec::new();
    for (key, value) in &changes {
        let existing = options.insert(key.clone(), value.clone());
        if existing.as_ref() != Some(value) {
            applied.push(key.clone());
        }
    }

    if !applied.is_empty() {
        write_options(&options_path, &options)?;
    }

    Ok(applied)
}

/// Restores options.txt from the atomic backup, if one exists.
pub fn restore_original_settings(instance_dir: &Path) -> Result<bool> {
    let options_path = instance_dir.join("options.txt");
    let backup_path = instance_dir.join("options.txt.atomic-backup");
    if backup_path.exists() {
        fs::copy(&backup_path, &options_path)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Detects total physical memory on the system in MB.
/// Tries Windows WMIC first; falls back to 2048 MB (conservative default).
pub fn detect_total_memory_mb() -> u32 {
    if cfg!(windows) {
        if let Ok(output) = std::process::Command::new("wmic")
            .args(["computersystem", "get", "totalphysicalmemory", "/value"])
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    let line = line.trim();
                    if let Some((_, val)) = line.split_once('=') {
                        if let Ok(bytes) = val.trim().parse::<u64>() {
                            return (bytes / 1_048_576) as u32;
                        }
                    }
                }
            }
        }
    }
    2048
}

/// Computes a conservative default memory allocation: 50% of total system RAM,
/// clamped between 512 MB and 4096 MB.
pub fn default_memory_mb() -> u32 {
    let total = detect_total_memory_mb();
    let half = total / 2;
    half.clamp(512, 4096)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_default_has_no_changes() {
        assert!(profile_changes(PerformanceProfile::Default).is_empty());
    }

    #[test]
    fn profile_balanced_has_changes() {
        let changes = profile_changes(PerformanceProfile::Balanced);
        assert!(!changes.is_empty());
        assert_eq!(changes.get("renderDistance"), Some(&"10".to_string()));
    }

    #[test]
    fn profile_performance_has_changes() {
        let changes = profile_changes(PerformanceProfile::Performance);
        assert!(!changes.is_empty());
        assert_eq!(changes.get("renderDistance"), Some(&"6".to_string()));
        assert_eq!(changes.get("graphicsMode"), Some(&"0".to_string()));
    }

    #[test]
    fn parse_options_handles_missing_file() {
        let map = read_options(Path::new("/nonexistent/options.txt"));
        assert!(map.is_empty());
    }

    #[test]
    fn parse_options_handles_comments_and_blanks() {
        let content = "# comment\nrenderDistance=12\n\nmaxFps=260";
        let path = Path::new("test_options.txt");
        std::fs::write(path, content).unwrap();
        let map = read_options(path);
        assert_eq!(map.get("renderDistance"), Some(&"12".to_string()));
        assert_eq!(map.get("maxFps"), Some(&"260".to_string()));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn default_memory_is_clamped() {
        let m = default_memory_mb();
        assert!(m >= 512);
        assert!(m <= 4096);
    }

    #[test]
    fn test_profile_default_has_no_changes() {
        let changes = profile_changes(PerformanceProfile::Default);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_profile_balanced_has_changes() {
        let changes = profile_changes(PerformanceProfile::Balanced);
        assert!(!changes.is_empty());
        assert_eq!(changes.get("renderDistance"), Some(&"10".to_string()));
        assert_eq!(changes.get("particles"), Some(&"1".to_string()));
        assert_eq!(changes.get("maxFps"), Some(&"144".to_string()));
    }

    #[test]
    fn test_profile_performance_has_changes() {
        let changes = profile_changes(PerformanceProfile::Performance);
        assert!(!changes.is_empty());
        assert_eq!(changes.get("renderDistance"), Some(&"6".to_string()));
        assert_eq!(changes.get("simulationDistance"), Some(&"6".to_string()));
        assert_eq!(changes.get("particles"), Some(&"2".to_string()));
        assert_eq!(changes.get("mipmapLevels"), Some(&"2".to_string()));
        assert_eq!(changes.get("maxFps"), Some(&"120".to_string()));
        assert_eq!(changes.get("graphicsMode"), Some(&"0".to_string()));
        assert_eq!(changes.get("enableVsync"), Some(&"false".to_string()));
        assert_eq!(changes.get("entityShadows"), Some(&"false".to_string()));
        assert_eq!(changes.get("renderClouds"), Some(&"false".to_string()));
        assert_eq!(changes.get("biomeBlendRadius"), Some(&"1".to_string()));
    }

    #[test]
    fn test_profile_descriptions_non_empty() {
        assert!(!profile_description(PerformanceProfile::Default).is_empty());
        assert!(!profile_description(PerformanceProfile::Balanced).is_empty());
        assert!(!profile_description(PerformanceProfile::Performance).is_empty());
    }

    #[test]
    fn test_apply_and_restore_profile() {
        use std::path::PathBuf;

        let dirs = Dirs::discover();
        let instance_dir = dirs.instance("test_instance");
        std::fs::create_dir_all(&instance_dir).expect("create instance dir");

        let options_path = instance_dir.join("options.txt");
        // Write original settings
        std::fs::write(
            &options_path,
            "renderDistance:12\nmaxFps:260\ngraphicsMode:1\nenableVsync:true\n",
        )
        .expect("write original options");

        // Apply performance profile
        let applied = apply_profile(&instance_dir, PerformanceProfile::Performance)
            .expect("apply profile should succeed");
        assert!(!applied.is_empty());

        // Verify changes were applied
        let content = std::fs::read_to_string(&options_path).expect("read options");
        assert!(content.contains("renderDistance=6"));
        assert!(content.contains("maxFps=120"));
        assert!(content.contains("graphicsMode=0"));
        assert!(content.contains("enableVsync=false"));

        // Verify backup was created
        let backup_path = instance_dir.join("options.txt.atomic-backup");
        assert!(backup_path.exists());
        let backup_content = std::fs::read_to_string(&backup_path).expect("read backup");
        assert!(backup_content.contains("renderDistance:12"));
        assert!(backup_content.contains("maxFps:260"));

        // Restore original settings
        let restored = restore_original_settings(&instance_dir).expect("restore should succeed");
        assert!(restored);
        let restored_content = std::fs::read_to_string(&options_path).expect("read restored");
        assert!(restored_content.contains("renderDistance:12"));
        assert!(restored_content.contains("maxFps:260"));
        assert!(restored_content.contains("graphicsMode:1"));
        assert!(restored_content.contains("enableVsync:true"));
    }

    #[test]
    fn test_restore_without_backup_returns_false() {
        let dirs = Dirs::discover();
        let instance_dir = dirs.instance("nonexistent_instance");
        let restored = restore_original_settings(&instance_dir).expect("restore should not panic");
        assert!(!restored);
    }

    #[test]
    fn test_parse_options_handles_comments_and_blanks() {
        let content = "# comment\nrenderDistance=12\n\nmaxFps=260";
        let path = Path::new("test_options.txt");
        std::fs::write(path, content).unwrap();
        let map = read_options(path);
        assert_eq!(map.get("renderDistance"), Some(&"12".to_string()));
        assert_eq!(map.get("maxFps"), Some(&"260".to_string()));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_default_memory_is_clamped() {
        let m = default_memory_mb();
        assert!(m >= 512);
        assert!(m <= 4096);
    }
}
