use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcId {
    String(String),
    Number(i64),
    Null,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcRequest {
    pub id: Option<RpcId>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcSuccess {
    pub id: RpcId,
    pub result: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcErrorResponse {
    pub id: Option<RpcId>,
    pub error: RpcErrorObject,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcErrorObject {
    pub code: RpcErrorCode,
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RpcErrorCode {
    InvalidRequest,
    Unauthorized,
    NotFound,
    Conflict,
    Timeout,
    RateLimited,
    Internal,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RpcContractError {
    #[error("method cannot be empty")]
    EmptyMethod,
    #[error("error message cannot be empty")]
    EmptyErrorMessage,
}

impl RpcRequest {
    pub fn validate(&self) -> Result<(), RpcContractError> {
        if self.method.trim().is_empty() {
            return Err(RpcContractError::EmptyMethod);
        }
        Ok(())
    }
}

impl RpcErrorObject {
    pub fn validate(&self) -> Result<(), RpcContractError> {
        if self.message.trim().is_empty() {
            return Err(RpcContractError::EmptyErrorMessage);
        }
        Ok(())
    }
}

pub mod server;

pub use server::{RpcServer, RpcServerInitError, SessionRecord};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn valid_request_passes_validation() {
        let req = RpcRequest {
            id: Some(RpcId::String("1".to_string())),
            method: "workspace.list".to_string(),
            params: Some(json!({})),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn empty_method_fails_validation() {
        let req = RpcRequest {
            id: None,
            method: "  ".to_string(),
            params: None,
        };
        assert_eq!(req.validate(), Err(RpcContractError::EmptyMethod));
    }

    #[test]
    fn error_object_requires_message() {
        let err = RpcErrorObject {
            code: RpcErrorCode::Internal,
            message: "".to_string(),
            data: None,
        };
        assert_eq!(err.validate(), Err(RpcContractError::EmptyErrorMessage));
    }

    #[test]
    fn request_roundtrip() {
        let input = json!({
            "id": 2,
            "method": "terminal.spawn",
            "params": { "workspace_id": "ws_1" }
        });
        let req: RpcRequest = serde_json::from_value(input.clone()).expect("deserialize");
        let output = serde_json::to_value(req).expect("serialize");
        assert_eq!(output, input);
    }

    #[test]
    fn error_code_serializes_to_expected_name() {
        let code = RpcErrorCode::RateLimited;
        let text = serde_json::to_string(&code).expect("serialize");
        assert_eq!(text, "\"RATE_LIMITED\"");
    }

    #[test]
    fn response_roundtrip() {
        let resp = RpcSuccess {
            id: RpcId::Number(42),
            result: json!({"ok": true}),
        };
        let encoded = serde_json::to_string(&resp).expect("serialize");
        let decoded: RpcSuccess = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, resp);
    }
}
