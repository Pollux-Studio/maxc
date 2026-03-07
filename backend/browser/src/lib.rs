use serde::{Deserialize, Serialize};

macro_rules! define_browser_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Option<Self> {
                let value = value.into();
                if value.trim().is_empty() {
                    None
                } else {
                    Some(Self(value))
                }
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

define_browser_id!(BrowserSessionId);
define_browser_id!(BrowserTabId);
define_browser_id!(FrameId);
define_browser_id!(TargetId);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserSessionMeta {
    pub browser_session_id: BrowserSessionId,
    pub workspace_id: String,
    pub surface_id: String,
    pub runtime: BrowserRuntime,
    pub driver: BrowserDriver,
    pub attached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserTabMeta {
    pub browser_tab_id: BrowserTabId,
    pub browser_session_id: BrowserSessionId,
    pub url: String,
    pub focused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowserCapability {
    CreateSurface,
    CloseSurface,
    OpenTab,
    CloseTab,
    Navigate,
    QueryDom,
    Click,
    Type,
    WaitFor,
    Screenshot,
    EvaluateScript,
    InterceptNetwork,
    UploadFile,
    DownloadFile,
    RawCommand,
    SubscribeEvents,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowserRuntime {
    Chromium,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowserDriver {
    Playwright,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowserMethod {
    BrowserCreate,
    BrowserAttach,
    BrowserDetach,
    BrowserClose,
    BrowserTabOpen,
    BrowserTabList,
    BrowserTabFocus,
    BrowserTabClose,
    BrowserGoto,
    BrowserReload,
    BrowserBack,
    BrowserForward,
    BrowserClick,
    BrowserType,
    BrowserWait,
    BrowserScreenshot,
    BrowserEvaluate,
    BrowserCookieGet,
    BrowserCookieSet,
    BrowserUpload,
    BrowserDownload,
    BrowserTraceStart,
    BrowserTraceStop,
    BrowserSubscribe,
    BrowserRawCommand,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrowserRpcRequest {
    pub method: BrowserMethod,
    pub workspace_id: String,
    pub surface_id: String,
    #[serde(default)]
    pub browser_session_id: Option<BrowserSessionId>,
    #[serde(default)]
    pub browser_tab_id: Option<BrowserTabId>,
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
}

pub fn phase_one_runtime() -> (BrowserRuntime, BrowserDriver) {
    (BrowserRuntime::Chromium, BrowserDriver::Playwright)
}

pub fn planned_capabilities() -> Vec<BrowserCapability> {
    vec![
        BrowserCapability::CreateSurface,
        BrowserCapability::CloseSurface,
        BrowserCapability::OpenTab,
        BrowserCapability::CloseTab,
        BrowserCapability::Navigate,
        BrowserCapability::QueryDom,
        BrowserCapability::Click,
        BrowserCapability::Type,
        BrowserCapability::WaitFor,
        BrowserCapability::Screenshot,
        BrowserCapability::EvaluateScript,
        BrowserCapability::InterceptNetwork,
        BrowserCapability::UploadFile,
        BrowserCapability::DownloadFile,
        BrowserCapability::RawCommand,
        BrowserCapability::SubscribeEvents,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_defaults_to_chromium_with_playwright() {
        let runtime = phase_one_runtime();
        assert_eq!(
            runtime,
            (BrowserRuntime::Chromium, BrowserDriver::Playwright)
        );
    }

    #[test]
    fn browser_ids_require_non_empty_values() {
        assert!(BrowserSessionId::new("s1").is_some());
        assert!(BrowserSessionId::new("").is_none());
        assert!(BrowserTabId::new("t1").is_some());
    }

    #[test]
    fn contract_struct_roundtrip() {
        let request = BrowserRpcRequest {
            method: BrowserMethod::BrowserGoto,
            workspace_id: "ws-1".to_string(),
            surface_id: "sf-1".to_string(),
            browser_session_id: BrowserSessionId::new("bs-1"),
            browser_tab_id: BrowserTabId::new("tab-1"),
            payload: Some(serde_json::json!({ "url": "https://example.com" })),
        };
        let encoded = serde_json::to_string(&request).expect("serialize");
        let decoded: BrowserRpcRequest = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded.workspace_id, "ws-1");
    }

    #[test]
    fn all_browser_methods_serialize() {
        let methods = vec![
            BrowserMethod::BrowserCreate,
            BrowserMethod::BrowserAttach,
            BrowserMethod::BrowserDetach,
            BrowserMethod::BrowserClose,
            BrowserMethod::BrowserTabOpen,
            BrowserMethod::BrowserTabList,
            BrowserMethod::BrowserTabFocus,
            BrowserMethod::BrowserTabClose,
            BrowserMethod::BrowserGoto,
            BrowserMethod::BrowserReload,
            BrowserMethod::BrowserBack,
            BrowserMethod::BrowserForward,
            BrowserMethod::BrowserClick,
            BrowserMethod::BrowserType,
            BrowserMethod::BrowserWait,
            BrowserMethod::BrowserScreenshot,
            BrowserMethod::BrowserEvaluate,
            BrowserMethod::BrowserCookieGet,
            BrowserMethod::BrowserCookieSet,
            BrowserMethod::BrowserUpload,
            BrowserMethod::BrowserDownload,
            BrowserMethod::BrowserTraceStart,
            BrowserMethod::BrowserTraceStop,
            BrowserMethod::BrowserSubscribe,
            BrowserMethod::BrowserRawCommand,
        ];
        for method in methods {
            let encoded = serde_json::to_string(&method).expect("serialize");
            let _: BrowserMethod = serde_json::from_str(&encoded).expect("deserialize");
        }
    }

    #[test]
    fn browser_meta_models_are_constructible() {
        let session = BrowserSessionMeta {
            browser_session_id: BrowserSessionId::new("bs-1").expect("id"),
            workspace_id: "ws-1".to_string(),
            surface_id: "sf-1".to_string(),
            runtime: BrowserRuntime::Chromium,
            driver: BrowserDriver::Playwright,
            attached: true,
        };
        let tab = BrowserTabMeta {
            browser_tab_id: BrowserTabId::new("tab-1").expect("id"),
            browser_session_id: BrowserSessionId::new("bs-1").expect("id"),
            url: "https://example.com".to_string(),
            focused: true,
        };
        assert_eq!(session.workspace_id, "ws-1");
        assert!(tab.focused);
        assert_eq!(planned_capabilities().len(), 16);
    }
}
