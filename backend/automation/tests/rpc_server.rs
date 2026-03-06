use maxc_automation::{RpcRequest, RpcServer};
use maxc_core::BackendConfig;
use serde_json::{json, Value};

fn test_config(label: &str) -> BackendConfig {
    let mut cfg = BackendConfig::default();
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_millis();
    cfg.event_dir = std::env::temp_dir()
        .join(format!("maxc-rpc-server-{label}-{millis}"))
        .to_string_lossy()
        .to_string();
    cfg
}

#[tokio::test]
async fn health_check_succeeds_without_auth() {
    let server = RpcServer::new(test_config("health")).expect("server");
    let input = json!({
        "id": 1,
        "method": "system.health"
    })
    .to_string();
    let output = server.handle_json_line("conn-1", &input).await;
    let parsed: Value = serde_json::from_str(&output).expect("valid json");
    assert_eq!(parsed["result"]["ok"], true);
}

#[tokio::test]
async fn session_refresh_rejects_invalid_token() {
    let server = RpcServer::new(test_config("invalid-token")).expect("server");
    let request = RpcRequest {
        id: None,
        method: "session.refresh".to_string(),
        params: Some(json!({
            "command_id": "cmd-1",
            "auth": {
                "token": "missing"
            }
        })),
    };

    let raw = serde_json::to_string(&request).expect("serialize");
    let output = server.handle_json_line("conn-2", &raw).await;
    let parsed: Value = serde_json::from_str(&output).expect("valid json");
    assert_eq!(parsed["error"]["code"], "UNAUTHORIZED");
}

#[tokio::test]
async fn malformed_json_maps_to_invalid_request() {
    let server = RpcServer::new(test_config("bad-json")).expect("server");
    let output = server.handle_json_line("conn-3", "{bad-json").await;
    let parsed: Value = serde_json::from_str(&output).expect("valid json");
    assert_eq!(parsed["error"]["code"], "INVALID_REQUEST");
}
