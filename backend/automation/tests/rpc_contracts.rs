use maxc_automation::{RpcContractError, RpcErrorCode, RpcErrorObject, RpcId, RpcRequest};
use maxc_browser::{BrowserMethod, BrowserRpcRequest, BrowserSessionId, BrowserTabId};
use serde_json::json;

#[test]
fn rpc_request_with_method_is_valid() {
    let request = RpcRequest {
        id: Some(RpcId::String("req-1".to_string())),
        method: "workspace.list".to_string(),
        params: Some(json!({})),
    };
    assert!(request.validate().is_ok());
}

#[test]
fn rpc_request_without_method_fails() {
    let request = RpcRequest {
        id: Some(RpcId::Number(1)),
        method: "".to_string(),
        params: None,
    };
    assert_eq!(request.validate(), Err(RpcContractError::EmptyMethod));
}

#[test]
fn rpc_error_code_serialization_is_stable() {
    let err = RpcErrorObject {
        code: RpcErrorCode::Timeout,
        message: "request timed out".to_string(),
        data: None,
    };

    let encoded = serde_json::to_value(err).expect("serialize");
    assert_eq!(encoded["code"], "TIMEOUT");
}

#[test]
fn browser_rpc_contract_roundtrip() {
    let request = BrowserRpcRequest {
        method: BrowserMethod::BrowserRawCommand,
        workspace_id: "ws-1".to_string(),
        surface_id: "sf-1".to_string(),
        browser_session_id: BrowserSessionId::new("bs-1"),
        browser_tab_id: BrowserTabId::new("tab-1"),
        payload: Some(json!({
            "allow_raw": true,
            "raw_command": "Page.captureScreenshot"
        })),
    };

    let encoded = serde_json::to_string(&request).expect("serialize");
    let decoded: BrowserRpcRequest = serde_json::from_str(&encoded).expect("deserialize");
    assert_eq!(decoded.workspace_id, "ws-1");
    assert_eq!(decoded.surface_id, "sf-1");
}
