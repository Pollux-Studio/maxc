use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use maxc_automation::RpcServer;
use maxc_core::BackendConfig;
use tauri::State;
use tauri_plugin_updater::UpdaterExt;
use url::Url;

#[derive(Clone)]
struct RpcState {
    server: Arc<RpcServer>,
}

static RPC_STATE: OnceLock<RpcState> = OnceLock::new();

fn init_server() -> RpcState {
    let config = BackendConfig::from_env().unwrap_or_else(|_| BackendConfig::default());
    let server = RpcServer::new(config).expect("failed to start backend automation server");
    RpcState {
        server: Arc::new(server),
    }
}

#[tauri::command]
async fn rpc_call(request: String, state: State<'_, RpcState>) -> Result<String, String> {
    let response = state.server.handle_json_line("tauri-ui", &request).await;
    Ok(response)
}

#[tauri::command]
async fn get_git_branch(folder: String) -> Result<String, String> {
    let folder_path = Path::new(&folder);
    if !folder_path.is_dir() {
        return Ok(String::new());
    }
    let head_path = folder_path.join(".git").join("HEAD");
    if !head_path.is_file() {
        // try walking up parent directories
        let mut current = folder_path.to_path_buf();
        loop {
            let git_head = current.join(".git").join("HEAD");
            if git_head.is_file() {
                return parse_git_head(&git_head);
            }
            if !current.pop() {
                break;
            }
        }
        return Ok(String::new());
    }
    parse_git_head(&head_path)
}

fn parse_git_head(head_path: &Path) -> Result<String, String> {
    let content = std::fs::read_to_string(head_path).map_err(|e| e.to_string())?;
    let trimmed = content.trim();
    if let Some(branch) = trimmed.strip_prefix("ref: refs/heads/") {
        Ok(branch.to_string())
    } else if trimmed.len() >= 7 {
        // detached HEAD, return short hash
        Ok(trimmed[..7].to_string())
    } else {
        Ok(String::new())
    }
}

#[tauri::command]
async fn create_window(app: tauri::AppHandle) -> Result<String, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis();
    let label = format!("window-{}", stamp);

    tauri::WebviewWindowBuilder::new(
        &app,
        label.clone(),
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("maxc_desktop")
    .inner_size(1280.0, 832.0)
    .decorations(false)
    .center()
    .build()
    .map_err(|e: tauri::Error| e.to_string())?;

    Ok(label)
}

fn updater_endpoint(channel: &str) -> Result<String, String> {
    match channel {
        "beta" => Ok(
            "https://github.com/Pollux-Studio/maxc/releases/download/beta/latest.json".to_string(),
        ),
        _ => Ok(
            "https://github.com/Pollux-Studio/maxc/releases/download/stable/latest.json"
                .to_string(),
        ),
    }
}

#[tauri::command]
async fn update_check(app: tauri::AppHandle, channel: String) -> Result<serde_json::Value, String> {
    let endpoint = updater_endpoint(channel.as_str())?;
    let url = Url::parse(&endpoint).map_err(|e| e.to_string())?;
    let updater = app
        .updater_builder()
        .endpoints(vec![url])
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;

    match updater.check().await.map_err(|e| e.to_string())? {
        Some(update) => Ok(serde_json::json!({
            "available": true,
            "version": update.version,
            "current_version": update.current_version,
            "date_ms": update.date.map(|d| d.unix_timestamp() * 1000),
            "body": update.body
        })),
        None => Ok(serde_json::json!({ "available": false })),
    }
}

#[tauri::command]
async fn update_download_and_install(app: tauri::AppHandle, channel: String) -> Result<(), String> {
    let endpoint = updater_endpoint(channel.as_str())?;
    let url = Url::parse(&endpoint).map_err(|e| e.to_string())?;
    let updater = app
        .updater_builder()
        .endpoints(vec![url])
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;

    if let Some(update) = updater.check().await.map_err(|e| e.to_string())? {
        update
            .download_and_install(|_, _| {}, || {})
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = RPC_STATE.get_or_init(init_server).clone();

    tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            rpc_call,
            get_git_branch,
            create_window,
            update_check,
            update_download_and_install
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
