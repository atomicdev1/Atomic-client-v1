use crate::engine::launch::{install_and_launch, LaunchOptions};
use crate::engine::paths::Dirs;
use crate::engine::settings::Settings;
use crate::engine::version_catalogue::{VersionCatalogue, VersionOrder, VersionTypeFilter};
use crate::engine::auth::offline_account;
use crate::engine::download::Progress;
use std::sync::Arc;
use std::path::PathBuf;

#[derive(serde::Serialize, Clone)]
pub struct ProgressPayload {
    pub done: u64,
    pub total: u64,
}

#[tauri::command]
pub async fn list_versions(state: tauri::State<'_, crate::AppState>) -> Result<serde_json::Value, String> {
    let client = state.client.clone();
    let dirs = state.dirs.clone();
    let catalogue = VersionCatalogue::new(client, dirs);
    let manifest = catalogue
        .fetch_manifest(false)
        .await
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "latest": manifest.latest,
        "versions": manifest.versions.iter().map(|v| serde_json::json!({
            "id": v.id, "type": v.kind, "releaseTime": v.release_time
        })).collect::<Vec<_>>()
    }))
}

#[tauri::command]
pub async fn list_versions_filtered(
    state: tauri::State<'_, crate::AppState>,
    filter: Option<String>,
    order: Option<String>,
) -> Result<serde_json::Value, String> {
    let client = state.client.clone();
    let dirs = state.dirs.clone();
    let catalogue = VersionCatalogue::new(client, dirs);
    let manifest = catalogue
        .fetch_manifest(false)
        .await
        .map_err(|e| e.to_string())?;

    let filter = match filter.as_deref() {
        Some("release") => VersionTypeFilter::Release,
        Some("snapshot") => VersionTypeFilter::Snapshot,
        Some("legacy") => VersionTypeFilter::Legacy,
        Some("exclude_snapshot") => VersionTypeFilter::ExcludeSnapshot,
        _ => VersionTypeFilter::All,
    };

    let order = match order.as_deref() {
        Some("oldest") => VersionOrder::OldestFirst,
        _ => VersionOrder::NewestFirst,
    };

    let versions = catalogue.list_versions(&manifest, &filter, order);
    Ok(serde_json::json!({
        "latest": manifest.latest,
        "versions": versions.iter().map(|v| serde_json::json!({
            "id": v.id, "type": v.kind, "releaseTime": v.release_time
        })).collect::<Vec<_>>(),
        "total": manifest.versions.len(),
    }))
}

#[tauri::command]
pub async fn get_version_info(
    state: tauri::State<'_, crate::AppState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let client = state.client.clone();
    let dirs = state.dirs.clone();
    let catalogue = VersionCatalogue::new(client, dirs);
    let manifest = catalogue
        .fetch_manifest(false)
        .await
        .map_err(|e| e.to_string())?;
    let info = catalogue
        .version_info(&id, &manifest)
        .await
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "id": info.id,
        "type": info.kind,
        "release_time": info.release_time,
        "java_major": info.java_major,
        "installed": info.installed,
        "client_size_mb": info.client_size_mb,
    }))
}

#[tauri::command]
pub async fn launch_game(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
    version: String,
    loader: String,
    offline: bool,
    demo: bool,
    dry_run: bool,
    profile: Option<String>,
) -> Result<crate::engine::launch::LaunchResult, String> {
    let settings = state.settings.lock().unwrap().clone();
    let client = state.client.clone();
    let dirs = state.dirs.clone();

    // Apply performance profile only if explicitly requested (opt-in)
    if let Some(profile_str) = &profile {
        if let Some(p) = crate::engine::performance::PerformanceProfile::from_str(profile_str) {
            let instance_dir = dirs.instance(&settings.instance);
            let _ = crate::engine::performance::apply_profile(&instance_dir, p);
        }
    }

    let mut account = if offline {
        offline_account(&settings.username)
    } else {
        match crate::engine::auth::load_account(&dirs) {
            Some(a) => a,
            None => offline_account(&settings.username),
        }
    };
    if account.name.is_empty() {
        account.name = settings.username.clone();
    }

    let opts = LaunchOptions {
        version,
        loader,
        player_name: account.name.clone(),
        uuid: account.uuid.clone(),
        access_token: account.access_token.clone(),
        user_type: account.user_type.clone(),
        xuid: account.xuid.clone(),
        client_id: account.client_id.clone(),
        demo,
        memory_mb: settings.memory_mb,
        java_path: if settings.java_path.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(&settings.java_path))
        },
        instance: settings.instance.clone(),
        dry_run,
        width: 1280,
        height: 720,
    };

    let app_p = app.clone();
    let progress: Progress = Arc::new(move |done, total| {
        let _ = app_p.emit("progress", ProgressPayload { done, total });
    });
    let app_l = app.clone();
    let log = Arc::new(move |line: String| {
        let _ = app_l.emit("launcher-log", line);
    });

    install_and_launch(&client, &dirs, &opts, Some(progress), Some(log))
        .await
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub fn data_dir(state: tauri::State<'_, crate::AppState>) -> String {
    state.dirs.root.to_string_lossy().to_string()
}