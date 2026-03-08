use std::path::Path;
use std::sync::{Arc, OnceLock};

use maxc_automation::RpcServer;
use maxc_core::BackendConfig;
use tauri::State;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = RPC_STATE.get_or_init(init_server).clone();

    tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![rpc_call, get_git_branch])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
