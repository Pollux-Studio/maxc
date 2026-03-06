use maxc_automation::{RpcRequest, RpcServer};
use maxc_core::BackendConfig;
use serde_json::{json, Value};

#[tokio::test]
async fn health_check_succeeds_without_auth() {
    let server = RpcServer::new(BackendConfig::default());
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
    let server = RpcServer::new(BackendConfig::default());
    let request = RpcRequest {
        id: None,
        method: "session.refresh".to_string(),
        params: Some(json!({
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
    let server = RpcServer::new(BackendConfig::default());
    let output = server.handle_json_line("conn-3", "{bad-json").await;
    let parsed: Value = serde_json::from_str(&output).expect("valid json");
    assert_eq!(parsed["error"]["code"], "INVALID_REQUEST");
}
