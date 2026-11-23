#[cfg(feature = "tauri")]
pub mod commands;

#[cfg(feature = "tauri")]
mod engine;

#[cfg(feature = "tauri")]
use std::sync::{Arc, Mutex};

#[cfg(feature = "tauri")]
use serde::Serialize;

#[cfg(feature = "tauri")]
use tauri::{AppHandle, Emitter, Manager, State};

#[cfg(feature = "tauri")]
use crate::engine::auth::{self, Account, DeviceCode, PollOutcome};

#[cfg(feature = "tauri")]
use crate::engine::download::Progress;

#[cfg(feature = "tauri")]
use crate::engine::launch::{install_and_launch, LaunchOptions, LaunchResult};

#[cfg(feature = "tauri")]
use crate::engine::modrinth::{self, SearchHit};

#[cfg(feature = "tauri")]
use crate::engine::paths::Dirs;

#[cfg(feature = "tauri")]
use crate::engine::settings::Settings;

#[cfg(feature = "tauri")]
pub struct AppState {
    pub client: reqwest::Client,
    pub dirs: Dirs,
    pub settings: Mutex<Settings>,
}

#[cfg(feature = "tauri")]
#[derive(Serialize, Clone)]
struct ProgressPayload {
    done: u64,
    total: u64,
}

#[cfg(feature = "tauri")]
#[derive(Serialize)]
pub struct AuthPoll {
    pub pending: bool,
    pub account: Option<Account>,
}

#[cfg(feature = "tauri")]
fn make_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!(
            "AtomicClient/",
            env!("CARGO_PKG_VERSION"),
            " (Minecraft launcher; +https://github.com/atomic-client)"
        ))
        .build()
        .expect("reqwest client")
}

#[cfg(feature = "tauri")]
#[tauri::command]
fn begin_login(state: State<'_, AppState>) -> Result<DeviceCode, String> {
    let s = state.settings.lock().unwrap().clone();
    auth::begin_device_code(&state.client, &s.client_id)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(feature = "tauri")]
#[tauri::command]
async fn poll_login(
    state: State<'_, AppState>,
    device_code: String,
) -> Result<AuthPoll, String> {
    let s = state.settings.lock().unwrap().clone();
    match auth::poll_device_code(&state.client, &s.client_id, &device_code)
        .await
        .map_err(|e| e.to_string())?
    {
        PollOutcome::Pending => Ok(AuthPoll {
            pending: true,
            account: None,
        }),
        PollOutcome::Done(acc) => {
            auth::save_account(&state.dirs, &acc).map_err(|e| e.to_string())?;
            Ok(AuthPoll {
                pending: false,
                account: Some(acc),
            })
        }
    }
}

#[cfg(feature = "tauri")]
#[tauri::command]
fn current_account(state: State<'_, AppState>) -> Option<Account> {
    auth::load_account(&state.dirs)
}

#[cfg(feature = "tauri")]
#[tauri::command]
fn logout(state: State<'_, AppState>) -> Result<(), String> {
    let p = state.dirs.root.join("auth.json");
    if p.exists() {
        std::fs::remove_file(p).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(feature = "tauri")]
#[tauri::command]
async fn search_mods(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<SearchHit>, String> {
    modrinth::search(&state.client, &query, 24)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(feature = "tauri")]
#[tauri::command]
async fn install_mod(
    state: State<'_, AppState>,
    slug: String,
    mc_version: String,
    loader: String,
    instance: String,
) -> Result<String, String> {
    modrinth::install(
        &state.client,
        &state.dirs,
        &slug,
        &mc_version,
        &loader,
        &instance,
        None,
    )
    .await
    .map(|p| p.to_string_lossy().to_string())
    .map_err(|e| e.to_string())
}

#[cfg(feature = "tauri")]
pub fn run_tauri() {
    let dirs = Dirs::discover();
    let settings = Settings::load(&dirs);
    let state = AppState {
        client: make_client(),
        dirs,
        settings: Mutex::new(settings),
    };

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::minecraft::list_versions,
            commands::minecraft::launch_game,
            commands::minecraft::data_dir,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::data_dir_settings,
            commands::performance::list_performance_profiles,
            commands::performance::apply_performance_profile,
            commands::performance::restore_settings,
            begin_login,
            poll_login,
            current_account,
            logout,
            search_mods,
            install_mod,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Atomic Client");
}
