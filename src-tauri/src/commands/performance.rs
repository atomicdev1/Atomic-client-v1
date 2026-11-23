use crate::engine::performance::{self, PerformanceProfile};
use crate::engine::paths::Dirs;
use std::path::PathBuf;

#[derive(serde::Serialize, Clone)]
pub struct ProfileInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub default: bool,
}

#[tauri::command]
pub fn list_performance_profiles() -> Vec<ProfileInfo> {
    vec![
        ProfileInfo {
            id: "default".to_string(),
            name: "Default".to_string(),
            description: performance::PerformanceProfile::Default.description(),
            default: true,
        },
        ProfileInfo {
            id: "balanced".to_string(),
            name: "Balanced".to_string(),
            description: performance::PerformanceProfile::Balanced.description(),
            default: false,
        },
        ProfileInfo {
            id: "performance".to_string(),
            name: "Performance".to_string(),
            description: performance::PerformanceProfile::Performance.description(),
            default: false,
        },
    ]
}

#[tauri::command]
pub fn apply_performance_profile(
    state: tauri::State<'_, crate::AppState>,
    profile_id: String,
) -> Result<Vec<String>, String> {
    let profile = PerformanceProfile::from_str(&profile_id)
        .ok_or_else(|| format!("Unknown profile: {profile_id}"))?;
    let settings = state.settings.lock().unwrap().clone();
    let instance_dir = state.dirs.instance(&settings.instance);
    performance::apply_profile(&instance_dir, profile)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn restore_settings(
    state: tauri::State<'_, crate::AppState>,
) -> Result<bool, String> {
    let settings = state.settings.lock().unwrap().clone();
    let instance_dir = state.dirs.instance(&settings.instance);
    performance::restore_original_settings(&instance_dir).map_err(|e| e.to_string())
}
