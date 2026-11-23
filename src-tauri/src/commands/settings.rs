use crate::engine::settings::Settings;
use crate::engine::paths::Dirs;

#[tauri::command]
pub fn get_settings(state: tauri::State<'_, crate::AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn save_settings(state: tauri::State<'_, crate::AppState>, settings: Settings) -> Result<(), String> {
    settings.save(&state.dirs).map_err(|e| e.to_string())?;
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

#[tauri::command]
pub fn data_dir_settings(state: tauri::State<'_, crate::AppState>) -> String {
    state.dirs.root.to_string_lossy().to_string()
}
