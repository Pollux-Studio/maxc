use maxc_automation::{RpcErrorCode, RpcId, RpcRequest, RpcSuccess};
use serde_json::json;

#[test]
fn json_rpc_v1_request_tolerates_additive_fields() {
    let raw = json!({
        "id": "req-1",
        "method": "system.health",
        "params": {"extra": true},
        "future_field": "ignored"
    });
    let request: RpcRequest = serde_json::from_value(raw).expect("request");
    assert_eq!(request.method, "system.health");
}

#[test]
fn json_rpc_v1_response_shape_remains_stable() {
    let response = RpcSuccess {
        id: RpcId::Number(1),
        result: json!({
            "ok": true,
            "version": "0.1.0",
            "future_optional": "allowed"
        }),
    };
    let encoded = serde_json::to_value(response).expect("response");
    assert_eq!(encoded["id"], 1);
    assert_eq!(encoded["result"]["ok"], true);
}

#[test]
fn rpc_error_code_names_are_additive_but_stable() {
    assert_eq!(
        serde_json::to_string(&RpcErrorCode::RateLimited).expect("serialize"),
        "\"RATE_LIMITED\""
    );
    assert_eq!(
        serde_json::to_string(&RpcErrorCode::Unauthorized).expect("serialize"),
        "\"UNAUTHORIZED\""
    );
}
