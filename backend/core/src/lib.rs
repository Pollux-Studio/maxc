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
