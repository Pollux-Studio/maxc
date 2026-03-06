use maxc_automation::{RpcContractError, RpcErrorCode, RpcErrorObject, RpcId, RpcRequest};
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
