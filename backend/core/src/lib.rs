use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IdParseError {
    #[error("id cannot be empty")]
    Empty,
    #[error("id cannot contain whitespace")]
    ContainsWhitespace,
}

fn validate_id_str(value: &str) -> Result<(), IdParseError> {
    if value.is_empty() {
        return Err(IdParseError::Empty);
    }
    if value.chars().any(char::is_whitespace) {
        return Err(IdParseError::ContainsWhitespace);
    }
    Ok(())
}

macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, IdParseError> {
                let value = value.into();
                validate_id_str(&value)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl FromStr for $name {
            type Err = IdParseError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::new(s)
            }
        }
    };
}

define_id!(WorkspaceId);
define_id!(PaneId);
define_id!(SurfaceId);
define_id!(SessionId);
define_id!(CommandId);
define_id!(EventId);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceMeta {
    pub id: WorkspaceId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneMeta {
    pub id: PaneId,
    pub workspace_id: WorkspaceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceMeta {
    pub id: SurfaceId,
    pub pane_id: PaneId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: SessionId,
    pub workspace_id: WorkspaceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendConfig {
    pub socket_path: String,
    pub request_timeout_ms: u64,
    pub queue_limit: usize,
    pub max_payload_bytes: usize,
    pub max_inflight_per_connection: usize,
    pub session_ttl_ms: u64,
    pub rate_limit_per_sec: u32,
    pub burst_limit: u32,
    pub event_dir: String,
    pub segment_max_bytes: u64,
    pub snapshot_interval_events: u64,
    pub snapshot_retain_count: usize,
    pub browser_runtime: String,
    pub browser_driver: String,
    pub browser_executable_or_channel: String,
    pub browser_launch_args: Vec<String>,
    pub browser_max_contexts: usize,
    pub browser_nav_timeout_ms: u64,
    pub browser_action_timeout_ms: u64,
    pub browser_screenshot_max_bytes: usize,
    pub browser_download_max_bytes: usize,
    pub browser_subscription_limit: usize,
    pub browser_raw_rate_limit_per_sec: u32,
    pub terminal_runtime: String,
    pub terminal_max_sessions: usize,
    pub terminal_max_sessions_per_workspace: usize,
    pub terminal_max_history_events: usize,
    pub terminal_max_history_bytes: usize,
    pub terminal_max_input_bytes: usize,
    pub terminal_max_env_bytes: usize,
    pub terminal_allowed_cwd_roots: Vec<String>,
    pub terminal_allowed_programs: Vec<String>,
    pub shutdown_drain_timeout_ms: u64,
    pub overload_reject_threshold: usize,
    pub breaker_failure_threshold: u32,
    pub breaker_cooldown_ms: u64,
    pub log_level: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("invalid value for {key}: {value}")]
    InvalidValue { key: &'static str, value: String },
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            socket_path: r"\\.\pipe\maxc-rpc".to_string(),
            request_timeout_ms: 5_000,
            queue_limit: 1_024,
            max_payload_bytes: 65_536,
            max_inflight_per_connection: 64,
            session_ttl_ms: 3_600_000,
            rate_limit_per_sec: 100,
            burst_limit: 200,
            event_dir: ".maxc/events".to_string(),
            segment_max_bytes: 1_048_576,
            snapshot_interval_events: 100,
            snapshot_retain_count: 3,
            browser_runtime: "chromium".to_string(),
            browser_driver: "playwright".to_string(),
            browser_executable_or_channel: "chromium".to_string(),
            browser_launch_args: Vec::new(),
            browser_max_contexts: 8,
            browser_nav_timeout_ms: 30_000,
            browser_action_timeout_ms: 10_000,
            browser_screenshot_max_bytes: 5_242_880,
            browser_download_max_bytes: 52_428_800,
            browser_subscription_limit: 32,
            browser_raw_rate_limit_per_sec: 10,
            terminal_runtime: if cfg!(windows) {
                "conpty".to_string()
            } else {
                "process-stdio".to_string()
            },
            terminal_max_sessions: 32,
            terminal_max_sessions_per_workspace: 8,
            terminal_max_history_events: 512,
            terminal_max_history_bytes: 262_144,
            terminal_max_input_bytes: 8_192,
            terminal_max_env_bytes: 8_192,
            terminal_allowed_cwd_roots: Vec::new(),
            terminal_allowed_programs: Vec::new(),
            shutdown_drain_timeout_ms: 3_000,
            overload_reject_threshold: 1_024,
            breaker_failure_threshold: 5,
            breaker_cooldown_ms: 10_000,
            log_level: "info".to_string(),
        }
    }
}

impl BackendConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_env_map(|key| std::env::var(key).ok())
    }

    pub fn from_env_map<F>(get: F) -> Result<Self, ConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let mut cfg = Self::default();

        if let Some(value) = get("MAXC_SOCKET_PATH") {
            cfg.socket_path = value;
        }

        if let Some(value) = get("MAXC_REQUEST_TIMEOUT_MS") {
            cfg.request_timeout_ms =
                value
                    .parse::<u64>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_REQUEST_TIMEOUT_MS",
                        value: value.clone(),
                    })?;
        }

        if let Some(value) = get("MAXC_QUEUE_LIMIT") {
            cfg.queue_limit = value
                .parse::<usize>()
                .map_err(|_| ConfigError::InvalidValue {
                    key: "MAXC_QUEUE_LIMIT",
                    value: value.clone(),
                })?;
        }
        if let Some(value) = get("MAXC_MAX_PAYLOAD_BYTES") {
            cfg.max_payload_bytes =
                value
                    .parse::<usize>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_MAX_PAYLOAD_BYTES",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_MAX_INFLIGHT_PER_CONNECTION") {
            cfg.max_inflight_per_connection =
                value
                    .parse::<usize>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_MAX_INFLIGHT_PER_CONNECTION",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_SESSION_TTL_MS") {
            cfg.session_ttl_ms = value
                .parse::<u64>()
                .map_err(|_| ConfigError::InvalidValue {
                    key: "MAXC_SESSION_TTL_MS",
                    value: value.clone(),
                })?;
        }
        if let Some(value) = get("MAXC_RATE_LIMIT_PER_SEC") {
            cfg.rate_limit_per_sec =
                value
                    .parse::<u32>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_RATE_LIMIT_PER_SEC",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_BURST_LIMIT") {
            cfg.burst_limit = value
                .parse::<u32>()
                .map_err(|_| ConfigError::InvalidValue {
                    key: "MAXC_BURST_LIMIT",
                    value: value.clone(),
                })?;
        }
        if let Some(value) = get("MAXC_EVENT_DIR") {
            cfg.event_dir = value;
        }
        if let Some(value) = get("MAXC_SEGMENT_MAX_BYTES") {
            cfg.segment_max_bytes =
                value
                    .parse::<u64>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_SEGMENT_MAX_BYTES",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_SNAPSHOT_INTERVAL_EVENTS") {
            cfg.snapshot_interval_events =
                value
                    .parse::<u64>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_SNAPSHOT_INTERVAL_EVENTS",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_SNAPSHOT_RETAIN_COUNT") {
            cfg.snapshot_retain_count =
                value
                    .parse::<usize>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_SNAPSHOT_RETAIN_COUNT",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_BROWSER_RUNTIME") {
            cfg.browser_runtime = value;
        }
        if let Some(value) = get("MAXC_BROWSER_DRIVER") {
            cfg.browser_driver = value;
        }
        if let Some(value) = get("MAXC_BROWSER_EXECUTABLE_OR_CHANNEL") {
            cfg.browser_executable_or_channel = value;
        }
        if let Some(value) = get("MAXC_BROWSER_LAUNCH_ARGS") {
            cfg.browser_launch_args = value
                .split(';')
                .filter(|v| !v.trim().is_empty())
                .map(|v| v.trim().to_string())
                .collect();
        }
        if let Some(value) = get("MAXC_BROWSER_MAX_CONTEXTS") {
            cfg.browser_max_contexts =
                value
                    .parse::<usize>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_BROWSER_MAX_CONTEXTS",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_BROWSER_NAV_TIMEOUT_MS") {
            cfg.browser_nav_timeout_ms =
                value
                    .parse::<u64>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_BROWSER_NAV_TIMEOUT_MS",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_BROWSER_ACTION_TIMEOUT_MS") {
            cfg.browser_action_timeout_ms =
                value
                    .parse::<u64>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_BROWSER_ACTION_TIMEOUT_MS",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_BROWSER_SCREENSHOT_MAX_BYTES") {
            cfg.browser_screenshot_max_bytes =
                value
                    .parse::<usize>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_BROWSER_SCREENSHOT_MAX_BYTES",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_BROWSER_DOWNLOAD_MAX_BYTES") {
            cfg.browser_download_max_bytes =
                value
                    .parse::<usize>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_BROWSER_DOWNLOAD_MAX_BYTES",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_BROWSER_SUBSCRIPTION_LIMIT") {
            cfg.browser_subscription_limit =
                value
                    .parse::<usize>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_BROWSER_SUBSCRIPTION_LIMIT",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_BROWSER_RAW_RATE_LIMIT_PER_SEC") {
            cfg.browser_raw_rate_limit_per_sec =
                value
                    .parse::<u32>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_BROWSER_RAW_RATE_LIMIT_PER_SEC",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_TERMINAL_RUNTIME") {
            cfg.terminal_runtime = value;
        }
        if let Some(value) = get("MAXC_TERMINAL_MAX_SESSIONS") {
            cfg.terminal_max_sessions =
                value
                    .parse::<usize>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_TERMINAL_MAX_SESSIONS",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_TERMINAL_MAX_SESSIONS_PER_WORKSPACE") {
            cfg.terminal_max_sessions_per_workspace =
                value
                    .parse::<usize>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_TERMINAL_MAX_SESSIONS_PER_WORKSPACE",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_TERMINAL_MAX_HISTORY_EVENTS") {
            cfg.terminal_max_history_events =
                value
                    .parse::<usize>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_TERMINAL_MAX_HISTORY_EVENTS",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_TERMINAL_MAX_HISTORY_BYTES") {
            cfg.terminal_max_history_bytes =
                value
                    .parse::<usize>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_TERMINAL_MAX_HISTORY_BYTES",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_TERMINAL_MAX_INPUT_BYTES") {
            cfg.terminal_max_input_bytes =
                value
                    .parse::<usize>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_TERMINAL_MAX_INPUT_BYTES",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_TERMINAL_MAX_ENV_BYTES") {
            cfg.terminal_max_env_bytes =
                value
                    .parse::<usize>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_TERMINAL_MAX_ENV_BYTES",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_TERMINAL_ALLOWED_CWD_ROOTS") {
            cfg.terminal_allowed_cwd_roots = value
                .split(';')
                .filter(|v| !v.trim().is_empty())
                .map(|v| v.trim().to_string())
                .collect();
        }
        if let Some(value) = get("MAXC_TERMINAL_ALLOWED_PROGRAMS") {
            cfg.terminal_allowed_programs = value
                .split(';')
                .filter(|v| !v.trim().is_empty())
                .map(|v| v.trim().to_string())
                .collect();
        }
        if let Some(value) = get("MAXC_SHUTDOWN_DRAIN_TIMEOUT_MS") {
            cfg.shutdown_drain_timeout_ms =
                value
                    .parse::<u64>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_SHUTDOWN_DRAIN_TIMEOUT_MS",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_OVERLOAD_REJECT_THRESHOLD") {
            cfg.overload_reject_threshold =
                value
                    .parse::<usize>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_OVERLOAD_REJECT_THRESHOLD",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_BREAKER_FAILURE_THRESHOLD") {
            cfg.breaker_failure_threshold =
                value
                    .parse::<u32>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_BREAKER_FAILURE_THRESHOLD",
                        value: value.clone(),
                    })?;
        }
        if let Some(value) = get("MAXC_BREAKER_COOLDOWN_MS") {
            cfg.breaker_cooldown_ms =
                value
                    .parse::<u64>()
                    .map_err(|_| ConfigError::InvalidValue {
                        key: "MAXC_BREAKER_COOLDOWN_MS",
                        value: value.clone(),
                    })?;
        }

        if let Some(value) = get("MAXC_LOG_LEVEL") {
            cfg.log_level = value;
        }

        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.socket_path.is_empty() {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_SOCKET_PATH",
                value: self.socket_path.clone(),
            });
        }
        if self.request_timeout_ms == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_REQUEST_TIMEOUT_MS",
                value: self.request_timeout_ms.to_string(),
            });
        }
        if self.queue_limit == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_QUEUE_LIMIT",
                value: self.queue_limit.to_string(),
            });
        }
        if self.max_payload_bytes == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_MAX_PAYLOAD_BYTES",
                value: self.max_payload_bytes.to_string(),
            });
        }
        if self.max_inflight_per_connection == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_MAX_INFLIGHT_PER_CONNECTION",
                value: self.max_inflight_per_connection.to_string(),
            });
        }
        if self.session_ttl_ms == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_SESSION_TTL_MS",
                value: self.session_ttl_ms.to_string(),
            });
        }
        if self.rate_limit_per_sec == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_RATE_LIMIT_PER_SEC",
                value: self.rate_limit_per_sec.to_string(),
            });
        }
        if self.burst_limit == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_BURST_LIMIT",
                value: self.burst_limit.to_string(),
            });
        }
        if self.event_dir.is_empty() {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_EVENT_DIR",
                value: self.event_dir.clone(),
            });
        }
        if self.segment_max_bytes == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_SEGMENT_MAX_BYTES",
                value: self.segment_max_bytes.to_string(),
            });
        }
        if self.snapshot_interval_events == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_SNAPSHOT_INTERVAL_EVENTS",
                value: self.snapshot_interval_events.to_string(),
            });
        }
        if self.snapshot_retain_count == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_SNAPSHOT_RETAIN_COUNT",
                value: self.snapshot_retain_count.to_string(),
            });
        }
        if self.browser_runtime.is_empty() {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_BROWSER_RUNTIME",
                value: self.browser_runtime.clone(),
            });
        }
        if self.browser_driver.is_empty() {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_BROWSER_DRIVER",
                value: self.browser_driver.clone(),
            });
        }
        if self.browser_executable_or_channel.is_empty() {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_BROWSER_EXECUTABLE_OR_CHANNEL",
                value: self.browser_executable_or_channel.clone(),
            });
        }
        if self.browser_max_contexts == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_BROWSER_MAX_CONTEXTS",
                value: self.browser_max_contexts.to_string(),
            });
        }
        if self.browser_nav_timeout_ms == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_BROWSER_NAV_TIMEOUT_MS",
                value: self.browser_nav_timeout_ms.to_string(),
            });
        }
        if self.browser_action_timeout_ms == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_BROWSER_ACTION_TIMEOUT_MS",
                value: self.browser_action_timeout_ms.to_string(),
            });
        }
        if self.browser_screenshot_max_bytes == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_BROWSER_SCREENSHOT_MAX_BYTES",
                value: self.browser_screenshot_max_bytes.to_string(),
            });
        }
        if self.browser_download_max_bytes == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_BROWSER_DOWNLOAD_MAX_BYTES",
                value: self.browser_download_max_bytes.to_string(),
            });
        }
        if self.browser_subscription_limit == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_BROWSER_SUBSCRIPTION_LIMIT",
                value: self.browser_subscription_limit.to_string(),
            });
        }
        if self.browser_raw_rate_limit_per_sec == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_BROWSER_RAW_RATE_LIMIT_PER_SEC",
                value: self.browser_raw_rate_limit_per_sec.to_string(),
            });
        }
        if !["conpty", "process-stdio"].contains(&self.terminal_runtime.as_str()) {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_TERMINAL_RUNTIME",
                value: self.terminal_runtime.clone(),
            });
        }
        if self.terminal_max_sessions == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_TERMINAL_MAX_SESSIONS",
                value: self.terminal_max_sessions.to_string(),
            });
        }
        if self.terminal_max_sessions_per_workspace == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_TERMINAL_MAX_SESSIONS_PER_WORKSPACE",
                value: self.terminal_max_sessions_per_workspace.to_string(),
            });
        }
        if self.terminal_max_history_events == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_TERMINAL_MAX_HISTORY_EVENTS",
                value: self.terminal_max_history_events.to_string(),
            });
        }
        if self.terminal_max_history_bytes == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_TERMINAL_MAX_HISTORY_BYTES",
                value: self.terminal_max_history_bytes.to_string(),
            });
        }
        if self.terminal_max_input_bytes == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_TERMINAL_MAX_INPUT_BYTES",
                value: self.terminal_max_input_bytes.to_string(),
            });
        }
        if self.terminal_max_env_bytes == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_TERMINAL_MAX_ENV_BYTES",
                value: self.terminal_max_env_bytes.to_string(),
            });
        }
        if self.shutdown_drain_timeout_ms == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_SHUTDOWN_DRAIN_TIMEOUT_MS",
                value: self.shutdown_drain_timeout_ms.to_string(),
            });
        }
        if self.overload_reject_threshold == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_OVERLOAD_REJECT_THRESHOLD",
                value: self.overload_reject_threshold.to_string(),
            });
        }
        if self.breaker_failure_threshold == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_BREAKER_FAILURE_THRESHOLD",
                value: self.breaker_failure_threshold.to_string(),
            });
        }
        if self.breaker_cooldown_ms == 0 {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_BREAKER_COOLDOWN_MS",
                value: self.breaker_cooldown_ms.to_string(),
            });
        }

        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.log_level.as_str()) {
            return Err(ConfigError::InvalidValue {
                key: "MAXC_LOG_LEVEL",
                value: self.log_level.clone(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_id_validates_input() {
        assert!(WorkspaceId::new("ws_1").is_ok());
        assert_eq!(WorkspaceId::new(""), Err(IdParseError::Empty));
        assert_eq!(
            WorkspaceId::new("bad id"),
            Err(IdParseError::ContainsWhitespace)
        );
    }

    #[test]
    fn id_roundtrip_display_and_parse() {
        let id = WorkspaceId::new("workspace-123").expect("valid");
        let text = id.to_string();
        let parsed = WorkspaceId::from_str(&text).expect("parse");
        assert_eq!(id, parsed);
    }

    #[test]
    fn serde_roundtrip_for_id() {
        let id = SessionId::new("sess_1").expect("valid");
        let json = serde_json::to_string(&id).expect("serialize");
        let out: SessionId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(id, out);
    }

    #[test]
    fn config_defaults_are_valid() {
        let cfg = BackendConfig::default();
        assert!(cfg.validate().is_ok());
        assert_eq!(cfg.request_timeout_ms, 5_000);
    }

    #[test]
    fn config_reads_env_map_overrides() {
        let cfg = BackendConfig::from_env_map(|key| match key {
            "MAXC_SOCKET_PATH" => Some("custom.sock".to_string()),
            "MAXC_REQUEST_TIMEOUT_MS" => Some("2500".to_string()),
            "MAXC_QUEUE_LIMIT" => Some("99".to_string()),
            "MAXC_LOG_LEVEL" => Some("debug".to_string()),
            _ => None,
        })
        .expect("valid config");

        assert_eq!(cfg.socket_path, "custom.sock");
        assert_eq!(cfg.request_timeout_ms, 2_500);
        assert_eq!(cfg.queue_limit, 99);
        assert_eq!(cfg.max_payload_bytes, 65_536);
        assert_eq!(cfg.max_inflight_per_connection, 64);
        assert_eq!(cfg.session_ttl_ms, 3_600_000);
        assert_eq!(cfg.rate_limit_per_sec, 100);
        assert_eq!(cfg.burst_limit, 200);
        assert_eq!(cfg.event_dir, ".maxc/events");
        assert_eq!(cfg.segment_max_bytes, 1_048_576);
        assert_eq!(cfg.snapshot_interval_events, 100);
        assert_eq!(cfg.snapshot_retain_count, 3);
        assert_eq!(cfg.browser_runtime, "chromium");
        assert_eq!(cfg.browser_driver, "playwright");
        assert_eq!(cfg.browser_executable_or_channel, "chromium");
        assert_eq!(cfg.browser_max_contexts, 8);
        assert_eq!(cfg.browser_nav_timeout_ms, 30_000);
        assert_eq!(cfg.browser_action_timeout_ms, 10_000);
        assert_eq!(cfg.browser_screenshot_max_bytes, 5_242_880);
        assert_eq!(cfg.browser_download_max_bytes, 52_428_800);
        assert_eq!(cfg.browser_subscription_limit, 32);
        assert_eq!(cfg.browser_raw_rate_limit_per_sec, 10);
        assert_eq!(
            cfg.terminal_runtime,
            if cfg!(windows) {
                "conpty".to_string()
            } else {
                "process-stdio".to_string()
            }
        );
        assert_eq!(cfg.terminal_max_sessions, 32);
        assert_eq!(cfg.terminal_max_sessions_per_workspace, 8);
        assert_eq!(cfg.terminal_max_history_events, 512);
        assert_eq!(cfg.terminal_max_history_bytes, 262_144);
        assert_eq!(cfg.terminal_max_input_bytes, 8_192);
        assert_eq!(cfg.terminal_max_env_bytes, 8_192);
        assert!(cfg.terminal_allowed_cwd_roots.is_empty());
        assert!(cfg.terminal_allowed_programs.is_empty());
        assert_eq!(cfg.shutdown_drain_timeout_ms, 3_000);
        assert_eq!(cfg.overload_reject_threshold, 1_024);
        assert_eq!(cfg.breaker_failure_threshold, 5);
        assert_eq!(cfg.breaker_cooldown_ms, 10_000);
        assert_eq!(cfg.log_level, "debug");
    }

    #[test]
    fn config_rejects_invalid_values() {
        let err = BackendConfig::from_env_map(|key| match key {
            "MAXC_REQUEST_TIMEOUT_MS" => Some("nope".to_string()),
            _ => None,
        })
        .expect_err("must fail");

        assert_eq!(
            err,
            ConfigError::InvalidValue {
                key: "MAXC_REQUEST_TIMEOUT_MS",
                value: "nope".to_string()
            }
        );
    }

    #[test]
    fn config_reads_all_phase_two_overrides() {
        let cfg = BackendConfig::from_env_map(|key| match key {
            "MAXC_SOCKET_PATH" => Some("pipe.custom".to_string()),
            "MAXC_REQUEST_TIMEOUT_MS" => Some("1111".to_string()),
            "MAXC_QUEUE_LIMIT" => Some("222".to_string()),
            "MAXC_MAX_PAYLOAD_BYTES" => Some("333".to_string()),
            "MAXC_MAX_INFLIGHT_PER_CONNECTION" => Some("4".to_string()),
            "MAXC_SESSION_TTL_MS" => Some("5000".to_string()),
            "MAXC_RATE_LIMIT_PER_SEC" => Some("6".to_string()),
            "MAXC_BURST_LIMIT" => Some("7".to_string()),
            "MAXC_EVENT_DIR" => Some("data/events".to_string()),
            "MAXC_SEGMENT_MAX_BYTES" => Some("8192".to_string()),
            "MAXC_SNAPSHOT_INTERVAL_EVENTS" => Some("10".to_string()),
            "MAXC_SNAPSHOT_RETAIN_COUNT" => Some("2".to_string()),
            "MAXC_BROWSER_RUNTIME" => Some("chromium".to_string()),
            "MAXC_BROWSER_DRIVER" => Some("playwright".to_string()),
            "MAXC_BROWSER_EXECUTABLE_OR_CHANNEL" => Some("chrome-beta".to_string()),
            "MAXC_BROWSER_LAUNCH_ARGS" => Some("--headless=new;--disable-gpu".to_string()),
            "MAXC_BROWSER_MAX_CONTEXTS" => Some("12".to_string()),
            "MAXC_BROWSER_NAV_TIMEOUT_MS" => Some("45000".to_string()),
            "MAXC_BROWSER_ACTION_TIMEOUT_MS" => Some("12000".to_string()),
            "MAXC_BROWSER_SCREENSHOT_MAX_BYTES" => Some("1024".to_string()),
            "MAXC_BROWSER_DOWNLOAD_MAX_BYTES" => Some("2048".to_string()),
            "MAXC_BROWSER_SUBSCRIPTION_LIMIT" => Some("9".to_string()),
            "MAXC_BROWSER_RAW_RATE_LIMIT_PER_SEC" => Some("3".to_string()),
            "MAXC_TERMINAL_RUNTIME" => Some("process-stdio".to_string()),
            "MAXC_TERMINAL_MAX_SESSIONS" => Some("13".to_string()),
            "MAXC_TERMINAL_MAX_SESSIONS_PER_WORKSPACE" => Some("5".to_string()),
            "MAXC_TERMINAL_MAX_HISTORY_EVENTS" => Some("123".to_string()),
            "MAXC_TERMINAL_MAX_HISTORY_BYTES" => Some("4567".to_string()),
            "MAXC_TERMINAL_MAX_INPUT_BYTES" => Some("111".to_string()),
            "MAXC_TERMINAL_MAX_ENV_BYTES" => Some("222".to_string()),
            "MAXC_TERMINAL_ALLOWED_CWD_ROOTS" => Some("C:\\work;D:\\repos".to_string()),
            "MAXC_TERMINAL_ALLOWED_PROGRAMS" => Some("powershell.exe;cmd.exe".to_string()),
            "MAXC_SHUTDOWN_DRAIN_TIMEOUT_MS" => Some("4000".to_string()),
            "MAXC_OVERLOAD_REJECT_THRESHOLD" => Some("77".to_string()),
            "MAXC_BREAKER_FAILURE_THRESHOLD" => Some("8".to_string()),
            "MAXC_BREAKER_COOLDOWN_MS" => Some("9000".to_string()),
            "MAXC_LOG_LEVEL" => Some("warn".to_string()),
            _ => None,
        })
        .expect("valid");

        assert_eq!(cfg.socket_path, "pipe.custom");
        assert_eq!(cfg.request_timeout_ms, 1111);
        assert_eq!(cfg.queue_limit, 222);
        assert_eq!(cfg.max_payload_bytes, 333);
        assert_eq!(cfg.max_inflight_per_connection, 4);
        assert_eq!(cfg.session_ttl_ms, 5000);
        assert_eq!(cfg.rate_limit_per_sec, 6);
        assert_eq!(cfg.burst_limit, 7);
        assert_eq!(cfg.event_dir, "data/events");
        assert_eq!(cfg.segment_max_bytes, 8192);
        assert_eq!(cfg.snapshot_interval_events, 10);
        assert_eq!(cfg.snapshot_retain_count, 2);
        assert_eq!(cfg.browser_runtime, "chromium");
        assert_eq!(cfg.browser_driver, "playwright");
        assert_eq!(cfg.browser_executable_or_channel, "chrome-beta");
        assert_eq!(
            cfg.browser_launch_args,
            vec!["--headless=new".to_string(), "--disable-gpu".to_string()]
        );
        assert_eq!(cfg.browser_max_contexts, 12);
        assert_eq!(cfg.browser_nav_timeout_ms, 45_000);
        assert_eq!(cfg.browser_action_timeout_ms, 12_000);
        assert_eq!(cfg.browser_screenshot_max_bytes, 1024);
        assert_eq!(cfg.browser_download_max_bytes, 2048);
        assert_eq!(cfg.browser_subscription_limit, 9);
        assert_eq!(cfg.browser_raw_rate_limit_per_sec, 3);
        assert_eq!(cfg.terminal_runtime, "process-stdio");
        assert_eq!(cfg.terminal_max_sessions, 13);
        assert_eq!(cfg.terminal_max_sessions_per_workspace, 5);
        assert_eq!(cfg.terminal_max_history_events, 123);
        assert_eq!(cfg.terminal_max_history_bytes, 4567);
        assert_eq!(cfg.terminal_max_input_bytes, 111);
        assert_eq!(cfg.terminal_max_env_bytes, 222);
        assert_eq!(
            cfg.terminal_allowed_cwd_roots,
            vec!["C:\\work".to_string(), "D:\\repos".to_string()]
        );
        assert_eq!(
            cfg.terminal_allowed_programs,
            vec!["powershell.exe".to_string(), "cmd.exe".to_string()]
        );
        assert_eq!(cfg.shutdown_drain_timeout_ms, 4000);
        assert_eq!(cfg.overload_reject_threshold, 77);
        assert_eq!(cfg.breaker_failure_threshold, 8);
        assert_eq!(cfg.breaker_cooldown_ms, 9000);
        assert_eq!(cfg.log_level, "warn");
    }

    #[test]
    fn config_rejects_new_invalid_env_values() {
        let bad_payload = BackendConfig::from_env_map(|key| match key {
            "MAXC_MAX_PAYLOAD_BYTES" => Some("bad".to_string()),
            _ => None,
        })
        .expect_err("must fail");
        assert_eq!(
            bad_payload,
            ConfigError::InvalidValue {
                key: "MAXC_MAX_PAYLOAD_BYTES",
                value: "bad".to_string(),
            }
        );

        let bad_inflight = BackendConfig::from_env_map(|key| match key {
            "MAXC_MAX_INFLIGHT_PER_CONNECTION" => Some("bad".to_string()),
            _ => None,
        })
        .expect_err("must fail");
        assert_eq!(
            bad_inflight,
            ConfigError::InvalidValue {
                key: "MAXC_MAX_INFLIGHT_PER_CONNECTION",
                value: "bad".to_string(),
            }
        );

        let bad_ttl = BackendConfig::from_env_map(|key| match key {
            "MAXC_SESSION_TTL_MS" => Some("bad".to_string()),
            _ => None,
        })
        .expect_err("must fail");
        assert_eq!(
            bad_ttl,
            ConfigError::InvalidValue {
                key: "MAXC_SESSION_TTL_MS",
                value: "bad".to_string(),
            }
        );

        let bad_rate = BackendConfig::from_env_map(|key| match key {
            "MAXC_RATE_LIMIT_PER_SEC" => Some("bad".to_string()),
            _ => None,
        })
        .expect_err("must fail");
        assert_eq!(
            bad_rate,
            ConfigError::InvalidValue {
                key: "MAXC_RATE_LIMIT_PER_SEC",
                value: "bad".to_string(),
            }
        );

        let bad_burst = BackendConfig::from_env_map(|key| match key {
            "MAXC_BURST_LIMIT" => Some("bad".to_string()),
            _ => None,
        })
        .expect_err("must fail");
        assert_eq!(
            bad_burst,
            ConfigError::InvalidValue {
                key: "MAXC_BURST_LIMIT",
                value: "bad".to_string(),
            }
        );

        let bad_segment = BackendConfig::from_env_map(|key| match key {
            "MAXC_SEGMENT_MAX_BYTES" => Some("bad".to_string()),
            _ => None,
        })
        .expect_err("must fail");
        assert_eq!(
            bad_segment,
            ConfigError::InvalidValue {
                key: "MAXC_SEGMENT_MAX_BYTES",
                value: "bad".to_string(),
            }
        );

        let bad_snapshot_interval = BackendConfig::from_env_map(|key| match key {
            "MAXC_SNAPSHOT_INTERVAL_EVENTS" => Some("bad".to_string()),
            _ => None,
        })
        .expect_err("must fail");
        assert_eq!(
            bad_snapshot_interval,
            ConfigError::InvalidValue {
                key: "MAXC_SNAPSHOT_INTERVAL_EVENTS",
                value: "bad".to_string(),
            }
        );

        let bad_snapshot_retain = BackendConfig::from_env_map(|key| match key {
            "MAXC_SNAPSHOT_RETAIN_COUNT" => Some("bad".to_string()),
            _ => None,
        })
        .expect_err("must fail");
        assert_eq!(
            bad_snapshot_retain,
            ConfigError::InvalidValue {
                key: "MAXC_SNAPSHOT_RETAIN_COUNT",
                value: "bad".to_string(),
            }
        );

        let bad_browser_contexts = BackendConfig::from_env_map(|key| match key {
            "MAXC_BROWSER_MAX_CONTEXTS" => Some("bad".to_string()),
            _ => None,
        })
        .expect_err("must fail");
        assert_eq!(
            bad_browser_contexts,
            ConfigError::InvalidValue {
                key: "MAXC_BROWSER_MAX_CONTEXTS",
                value: "bad".to_string(),
            }
        );

        let bad_browser_nav_timeout = BackendConfig::from_env_map(|key| match key {
            "MAXC_BROWSER_NAV_TIMEOUT_MS" => Some("bad".to_string()),
            _ => None,
        })
        .expect_err("must fail");
        assert_eq!(
            bad_browser_nav_timeout,
            ConfigError::InvalidValue {
                key: "MAXC_BROWSER_NAV_TIMEOUT_MS",
                value: "bad".to_string(),
            }
        );

        let bad_browser_raw_rate = BackendConfig::from_env_map(|key| match key {
            "MAXC_BROWSER_RAW_RATE_LIMIT_PER_SEC" => Some("bad".to_string()),
            _ => None,
        })
        .expect_err("must fail");
        assert_eq!(
            bad_browser_raw_rate,
            ConfigError::InvalidValue {
                key: "MAXC_BROWSER_RAW_RATE_LIMIT_PER_SEC",
                value: "bad".to_string(),
            }
        );

        let bad_terminal_sessions = BackendConfig::from_env_map(|key| match key {
            "MAXC_TERMINAL_MAX_SESSIONS" => Some("bad".to_string()),
            _ => None,
        })
        .expect_err("must fail");
        assert_eq!(
            bad_terminal_sessions,
            ConfigError::InvalidValue {
                key: "MAXC_TERMINAL_MAX_SESSIONS",
                value: "bad".to_string(),
            }
        );

        let bad_terminal_runtime = BackendConfig::from_env_map(|key| match key {
            "MAXC_TERMINAL_RUNTIME" => Some("bogus".to_string()),
            _ => None,
        })
        .expect_err("must fail");
        assert_eq!(
            bad_terminal_runtime,
            ConfigError::InvalidValue {
                key: "MAXC_TERMINAL_RUNTIME",
                value: "bogus".to_string(),
            }
        );

        let bad_shutdown_timeout = BackendConfig::from_env_map(|key| match key {
            "MAXC_SHUTDOWN_DRAIN_TIMEOUT_MS" => Some("bad".to_string()),
            _ => None,
        })
        .expect_err("must fail");
        assert_eq!(
            bad_shutdown_timeout,
            ConfigError::InvalidValue {
                key: "MAXC_SHUTDOWN_DRAIN_TIMEOUT_MS",
                value: "bad".to_string(),
            }
        );

        let bad_overload = BackendConfig::from_env_map(|key| match key {
            "MAXC_OVERLOAD_REJECT_THRESHOLD" => Some("bad".to_string()),
            _ => None,
        })
        .expect_err("must fail");
        assert_eq!(
            bad_overload,
            ConfigError::InvalidValue {
                key: "MAXC_OVERLOAD_REJECT_THRESHOLD",
                value: "bad".to_string(),
            }
        );

        let bad_breaker_threshold = BackendConfig::from_env_map(|key| match key {
            "MAXC_BREAKER_FAILURE_THRESHOLD" => Some("bad".to_string()),
            _ => None,
        })
        .expect_err("must fail");
        assert_eq!(
            bad_breaker_threshold,
            ConfigError::InvalidValue {
                key: "MAXC_BREAKER_FAILURE_THRESHOLD",
                value: "bad".to_string(),
            }
        );

        let bad_breaker_cooldown = BackendConfig::from_env_map(|key| match key {
            "MAXC_BREAKER_COOLDOWN_MS" => Some("bad".to_string()),
            _ => None,
        })
        .expect_err("must fail");
        assert_eq!(
            bad_breaker_cooldown,
            ConfigError::InvalidValue {
                key: "MAXC_BREAKER_COOLDOWN_MS",
                value: "bad".to_string(),
            }
        );
    }

    #[test]
    fn config_validate_rejects_zero_and_invalid_log_level() {
        let cfg = BackendConfig {
            max_payload_bytes: 0,
            ..BackendConfig::default()
        };
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::InvalidValue {
                key: "MAXC_MAX_PAYLOAD_BYTES",
                value: "0".to_string(),
            })
        );

        let cfg = BackendConfig {
            max_inflight_per_connection: 0,
            ..BackendConfig::default()
        };
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::InvalidValue {
                key: "MAXC_MAX_INFLIGHT_PER_CONNECTION",
                value: "0".to_string(),
            })
        );

        let cfg = BackendConfig {
            session_ttl_ms: 0,
            ..BackendConfig::default()
        };
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::InvalidValue {
                key: "MAXC_SESSION_TTL_MS",
                value: "0".to_string(),
            })
        );

        let cfg = BackendConfig {
            rate_limit_per_sec: 0,
            ..BackendConfig::default()
        };
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::InvalidValue {
                key: "MAXC_RATE_LIMIT_PER_SEC",
                value: "0".to_string(),
            })
        );

        let cfg = BackendConfig {
            burst_limit: 0,
            ..BackendConfig::default()
        };
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::InvalidValue {
                key: "MAXC_BURST_LIMIT",
                value: "0".to_string(),
            })
        );

        let cfg = BackendConfig {
            event_dir: "".to_string(),
            ..BackendConfig::default()
        };
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::InvalidValue {
                key: "MAXC_EVENT_DIR",
                value: "".to_string(),
            })
        );

        let cfg = BackendConfig {
            segment_max_bytes: 0,
            ..BackendConfig::default()
        };
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::InvalidValue {
                key: "MAXC_SEGMENT_MAX_BYTES",
                value: "0".to_string(),
            })
        );

        let cfg = BackendConfig {
            snapshot_interval_events: 0,
            ..BackendConfig::default()
        };
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::InvalidValue {
                key: "MAXC_SNAPSHOT_INTERVAL_EVENTS",
                value: "0".to_string(),
            })
        );

        let cfg = BackendConfig {
            snapshot_retain_count: 0,
            ..BackendConfig::default()
        };
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::InvalidValue {
                key: "MAXC_SNAPSHOT_RETAIN_COUNT",
                value: "0".to_string(),
            })
        );

        let cfg = BackendConfig {
            browser_runtime: "".to_string(),
            ..BackendConfig::default()
        };
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::InvalidValue {
                key: "MAXC_BROWSER_RUNTIME",
                value: "".to_string(),
            })
        );

        let cfg = BackendConfig {
            browser_driver: "".to_string(),
            ..BackendConfig::default()
        };
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::InvalidValue {
                key: "MAXC_BROWSER_DRIVER",
                value: "".to_string(),
            })
        );

        let cfg = BackendConfig {
            browser_subscription_limit: 0,
            ..BackendConfig::default()
        };
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::InvalidValue {
                key: "MAXC_BROWSER_SUBSCRIPTION_LIMIT",
                value: "0".to_string(),
            })
        );

        let cfg = BackendConfig {
            terminal_max_sessions: 0,
            ..BackendConfig::default()
        };
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::InvalidValue {
                key: "MAXC_TERMINAL_MAX_SESSIONS",
                value: "0".to_string(),
            })
        );

        let cfg = BackendConfig {
            terminal_runtime: "bad".to_string(),
            ..BackendConfig::default()
        };
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::InvalidValue {
                key: "MAXC_TERMINAL_RUNTIME",
                value: "bad".to_string(),
            })
        );

        let cfg = BackendConfig {
            shutdown_drain_timeout_ms: 0,
            ..BackendConfig::default()
        };
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::InvalidValue {
                key: "MAXC_SHUTDOWN_DRAIN_TIMEOUT_MS",
                value: "0".to_string(),
            })
        );

        let cfg = BackendConfig {
            overload_reject_threshold: 0,
            ..BackendConfig::default()
        };
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::InvalidValue {
                key: "MAXC_OVERLOAD_REJECT_THRESHOLD",
                value: "0".to_string(),
            })
        );

        let cfg = BackendConfig {
            breaker_failure_threshold: 0,
            ..BackendConfig::default()
        };
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::InvalidValue {
                key: "MAXC_BREAKER_FAILURE_THRESHOLD",
                value: "0".to_string(),
            })
        );

        let cfg = BackendConfig {
            breaker_cooldown_ms: 0,
            ..BackendConfig::default()
        };
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::InvalidValue {
                key: "MAXC_BREAKER_COOLDOWN_MS",
                value: "0".to_string(),
            })
        );

        let cfg = BackendConfig {
            log_level: "verbose".to_string(),
            ..BackendConfig::default()
        };
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::InvalidValue {
                key: "MAXC_LOG_LEVEL",
                value: "verbose".to_string(),
            })
        );
    }
}
