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
    let response = state
        .server
        .handle_json_line("tauri-ui", &request)
        .await;
    Ok(response)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = RPC_STATE.get_or_init(init_server).clone();

    tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![rpc_call])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
